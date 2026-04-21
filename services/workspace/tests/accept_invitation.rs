extern crate workspace as workspace_crate;

use relay_proto::workspace::workspace_service_client::WorkspaceServiceClient;
use relay_proto::workspace::{
    AcceptInvitationRequest, CreateWorkspaceRequest, IssueInvitationRequest,
};
use sea_orm::{ActiveValue::Set, ColumnTrait, Database, EntityTrait, QueryFilter};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{core::IntoContainerPort, runners::AsyncRunner},
};
use tonic::{Request, metadata::MetadataValue, transport::Server};
use uuid::Uuid;

use migration::{Migrator, MigratorTrait};
use workspace_crate::{
    entity::{user_snapshot, workspace_member, workspace_member_role, workspace_role},
    grpc::WorkspaceServer,
};

const ACTOR_USER_ID_METADATA: &str = "x-user-id";

#[tokio::test]
async fn accept_invitation_replaces_stale_member_role() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let actor_user_id = Uuid::new_v4();
    let target_user_id = Uuid::new_v4();

    insert_user_snapshot(&env.db, actor_user_id).await?;
    insert_user_snapshot(&env.db, target_user_id).await?;

    let created = env
        .client
        .clone()
        .create_workspace(actor_request(
            actor_user_id,
            CreateWorkspaceRequest {
                name: "Acme".to_string(),
                first_channel_name: "general".to_string(),
            },
        ))
        .await?
        .into_inner();

    let invitation = env
        .client
        .clone()
        .issue_invitation(actor_request(
            actor_user_id,
            IssueInvitationRequest {
                workspace_id: created.workspace_id.clone(),
                target_user_id: target_user_id.to_string(),
                expires_at: None,
            },
        ))
        .await?
        .into_inner();

    let workspace_id = Uuid::parse_str(&created.workspace_id)?;
    let workspace_invitation_id = Uuid::parse_str(&invitation.workspace_invitation_id)?;

    let member_role_id = workspace_role::Entity::find()
        .filter(workspace_role::Column::WorkspaceId.eq(workspace_id))
        .filter(workspace_role::Column::Name.eq("Member".to_string()))
        .one(&env.db)
        .await?
        .expect("member role")
        .id;

    workspace_member::Entity::insert(workspace_member::ActiveModel {
        workspace_id: Set(workspace_id),
        user_id: Set(target_user_id),
        membership_status: Set("removed".to_string()),
        joined_at: Set(chrono::Utc::now().into()),
        removed_at: Set(Some(chrono::Utc::now().into())),
        added_by_user_id: Set(Some(actor_user_id)),
    })
    .exec(&env.db)
    .await?;

    workspace_member_role::Entity::insert(workspace_member_role::ActiveModel {
        workspace_id: Set(workspace_id),
        user_id: Set(target_user_id),
        role_id: Set(member_role_id),
        assigned_at: Set(chrono::Utc::now().into()),
        assigned_by_user_id: Set(Some(actor_user_id)),
    })
    .exec(&env.db)
    .await?;

    let response = env
        .client
        .clone()
        .accept_invitation(actor_request(
            target_user_id,
            AcceptInvitationRequest {
                workspace_invitation_id: workspace_invitation_id.to_string(),
            },
        ))
        .await?
        .into_inner();

    assert_eq!(
        response.workspace_invitation_id,
        workspace_invitation_id.to_string()
    );
    assert_eq!(response.user_id, target_user_id.to_string());
    assert_eq!(response.added_by_user_id, actor_user_id.to_string());

    let target_member = workspace_member::Entity::find()
        .filter(workspace_member::Column::WorkspaceId.eq(workspace_id))
        .filter(workspace_member::Column::UserId.eq(target_user_id))
        .one(&env.db)
        .await?
        .expect("member row");
    assert_eq!(target_member.membership_status, "active");
    assert_eq!(target_member.added_by_user_id, Some(actor_user_id));

    let target_roles = workspace_member_role::Entity::find()
        .filter(workspace_member_role::Column::WorkspaceId.eq(workspace_id))
        .filter(workspace_member_role::Column::UserId.eq(target_user_id))
        .all(&env.db)
        .await?;
    assert_eq!(target_roles.len(), 1);
    assert_eq!(target_roles[0].role_id, member_role_id);

    env.shutdown().await;
    Ok(())
}

struct TestEnv {
    _postgres: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    db: sea_orm::DatabaseConnection,
    client: WorkspaceServiceClient<tonic::transport::Channel>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    server_task: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
}

impl TestEnv {
    async fn start() -> Result<Self, Box<dyn std::error::Error>> {
        let postgres = Postgres::default().start().await?;
        let postgres_host = postgres.get_host().await?;
        let postgres_port = postgres.get_host_port_ipv4(5432.tcp()).await?;
        let database_url =
            format!("postgres://postgres:postgres@{postgres_host}:{postgres_port}/postgres");

        let db = Database::connect(&database_url).await?;
        Migrator::up(&db, None).await?;

        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        drop(listener);

        let handler = WorkspaceServer::new(db.clone());
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        let server_task = tokio::spawn(async move {
            Server::builder()
                .add_service(handler.into_server())
                .serve_with_shutdown(addr, async {
                    let _ = shutdown_rx.await;
                })
                .await
        });

        let client = connect_client(addr).await?;

        Ok(Self {
            _postgres: postgres,
            db,
            client,
            shutdown: Some(shutdown_tx),
            server_task,
        })
    }

    async fn shutdown(mut self) {
        if let Some(sender) = self.shutdown.take() {
            let _ = sender.send(());
        }

        let _ = self.server_task.await;
    }
}

async fn connect_client(
    addr: std::net::SocketAddr,
) -> Result<WorkspaceServiceClient<tonic::transport::Channel>, Box<dyn std::error::Error>> {
    let endpoint = format!("http://{addr}");

    for _ in 0..20 {
        match WorkspaceServiceClient::connect(endpoint.clone()).await {
            Ok(client) => return Ok(client),
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(50)).await,
        }
    }

    Ok(WorkspaceServiceClient::connect(endpoint).await?)
}

fn actor_request<T>(user_id: Uuid, request: T) -> Request<T> {
    let mut request = Request::new(request);
    request.metadata_mut().insert(
        ACTOR_USER_ID_METADATA,
        MetadataValue::try_from(user_id.to_string()).expect("metadata"),
    );
    request
}

async fn insert_user_snapshot(
    db: &sea_orm::DatabaseConnection,
    user_id: Uuid,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = chrono::Utc::now();
    workspace_crate::entity::user_snapshot::Entity::insert(user_snapshot::ActiveModel {
        user_id: Set(user_id),
        email_verified: Set(false),
        username: Set(format!("user-{user_id}")),
        display_name: Set("Test User".to_string()),
        avatar_url: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    })
    .exec(db)
    .await?;

    Ok(())
}
