extern crate workspace as workspace_crate;

use relay_proto::workspace::workspace_service_client::WorkspaceServiceClient;
use relay_proto::workspace::{AddMemberRequest, CreateWorkspaceRequest};
use sea_orm::{
    ActiveValue::Set, ColumnTrait, Database, EntityTrait, IntoActiveModel, QueryFilter,
    QueryOrder,
};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{core::IntoContainerPort, runners::AsyncRunner},
};
use tonic::{metadata::MetadataValue, transport::Server, Request};
use uuid::Uuid;

use migration::{Migrator, MigratorTrait};
use workspace_crate::{
    entity::{outbox_event, user_snapshot, workspace_member, workspace_member_role, workspace_role},
    grpc::WorkspaceServer,
};

const ACTOR_USER_ID_METADATA: &str = "x-user-id";

#[tokio::test]
async fn add_member_reactivates_removed_member_and_recreates_baseline_role()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let actor_user_id = Uuid::new_v4();
    let target_user_id = Uuid::new_v4();

    insert_user_account(&env.db, actor_user_id).await?;
    insert_user_account(&env.db, target_user_id).await?;

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

    env.client
        .clone()
        .add_member(actor_request(
            actor_user_id,
            AddMemberRequest {
                workspace_id: created.workspace_id.clone(),
                target_user_id: target_user_id.to_string(),
            },
        ))
        .await?;

    let workspace_id = Uuid::parse_str(&created.workspace_id)?;

    let target_member = workspace_member::Entity::find()
        .filter(workspace_member::Column::WorkspaceId.eq(workspace_id))
        .filter(workspace_member::Column::UserId.eq(target_user_id))
        .one(&env.db)
        .await?
        .expect("member row");
    let mut target_member_active = target_member.into_active_model();
    target_member_active.membership_status = Set("removed".to_string());
    target_member_active.removed_at = Set(Some(chrono::Utc::now().into()));
    workspace_member::Entity::update(target_member_active)
        .exec(&env.db)
        .await?;

    env.client
        .clone()
        .add_member(actor_request(
            actor_user_id,
            AddMemberRequest {
                workspace_id: created.workspace_id.clone(),
                target_user_id: target_user_id.to_string(),
            },
        ))
        .await?
        .into_inner();

    let target_member = workspace_member::Entity::find()
        .filter(workspace_member::Column::WorkspaceId.eq(workspace_id))
        .filter(workspace_member::Column::UserId.eq(target_user_id))
        .one(&env.db)
        .await?
        .expect("member row after readd");
    assert_eq!(target_member.membership_status, "active");
    assert_eq!(target_member.added_by_user_id, Some(actor_user_id));

    let member_role_id = workspace_role::Entity::find()
        .filter(workspace_role::Column::WorkspaceId.eq(workspace_id))
        .filter(workspace_role::Column::Name.eq("Member".to_string()))
        .one(&env.db)
        .await?
        .expect("member role")
        .id;

    let target_member_role = workspace_member_role::Entity::find()
        .filter(workspace_member_role::Column::WorkspaceId.eq(workspace_id))
        .filter(workspace_member_role::Column::UserId.eq(target_user_id))
        .one(&env.db)
        .await?
        .expect("member role row");
    assert_eq!(target_member_role.role_id, member_role_id);

    let added_event = outbox_event::Entity::find()
        .filter(outbox_event::Column::AggregateId.eq(workspace_id))
        .filter(outbox_event::Column::EventType.eq("WorkspaceMemberAdded".to_string()))
        .order_by_desc(outbox_event::Column::CreatedAt)
        .one(&env.db)
        .await?
        .expect("added event");
    assert_eq!(added_event.payload["user_id"], target_user_id.to_string());
    assert_eq!(added_event.payload["added_by_user_id"], actor_user_id.to_string());
    assert_eq!(added_event.payload["source"], "direct_add");

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
    request
        .metadata_mut()
        .insert(
            ACTOR_USER_ID_METADATA,
            MetadataValue::try_from(user_id.to_string()).expect("metadata"),
        );
    request
}

async fn insert_user_account(
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
