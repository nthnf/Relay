extern crate workspace as workspace_crate;

use relay_proto::workspace::workspace_service_client::WorkspaceServiceClient;
use relay_proto::workspace::{CreateWorkspaceRequest, IssueInvitationRequest};
use sea_orm::{ActiveValue::Set, ColumnTrait, Database, EntityTrait, QueryFilter};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{core::IntoContainerPort, runners::AsyncRunner},
};
use tonic::{Request, metadata::MetadataValue, transport::Server};
use uuid::Uuid;

use migration::{Migrator, MigratorTrait};
use workspace_crate::{
    entity::{outbox_event, user_snapshot, workspace_invitation},
    grpc::WorkspaceServer,
};

const ACTOR_USER_ID_METADATA: &str = "x-user-id";

#[tokio::test]
async fn issue_invitation_persists_invitation_and_outbox_event()
-> Result<(), Box<dyn std::error::Error>> {
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

    let response = env
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

    assert_eq!(response.workspace_id, created.workspace_id);
    assert_eq!(response.issued_to_user_id, target_user_id.to_string());
    assert_eq!(response.issued_by_user_id, actor_user_id.to_string());
    assert_eq!(response.status, "pending");

    let workspace_invitation_id = Uuid::parse_str(&response.workspace_invitation_id)?;
    let workspace_id = Uuid::parse_str(&created.workspace_id)?;

    let invitation_row: workspace_invitation::Model =
        workspace_invitation::Entity::find_by_id(workspace_invitation_id)
            .one(&env.db)
            .await?
            .expect("workspace invitation row");
    assert_eq!(invitation_row.workspace_id, workspace_id);
    assert_eq!(invitation_row.issued_to_user_id, target_user_id);
    assert_eq!(invitation_row.issued_by_user_id, actor_user_id);
    assert_eq!(invitation_row.status, "pending");

    let outbox_row = outbox_event::Entity::find()
        .filter(outbox_event::Column::AggregateType.eq("workspace_invitation"))
        .filter(outbox_event::Column::AggregateId.eq(workspace_invitation_id))
        .filter(outbox_event::Column::EventType.eq("WorkspaceInvitationIssued".to_string()))
        .one(&env.db)
        .await?
        .expect("workspace invitation outbox row");

    assert_eq!(
        outbox_row.payload["workspace_invitation_id"],
        response.workspace_invitation_id
    );
    assert_eq!(outbox_row.payload["workspace_id"], created.workspace_id);
    assert_eq!(
        outbox_row.payload["issued_to_user_id"],
        target_user_id.to_string()
    );
    assert_eq!(
        outbox_row.payload["issued_by_user_id"],
        actor_user_id.to_string()
    );
    assert_eq!(outbox_row.payload["workspace_name_snapshot"], "Acme");
    assert_eq!(
        outbox_row.payload["inviter_display_name_snapshot"],
        "Test User"
    );

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
