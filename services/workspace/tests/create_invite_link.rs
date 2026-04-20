extern crate workspace as workspace_crate;

use chrono::Duration;
use relay_proto::workspace::workspace_service_client::WorkspaceServiceClient;
use relay_proto::workspace::{CreateInviteLinkRequest, CreateWorkspaceRequest};
use sea_orm::{ColumnTrait, Database, EntityTrait, QueryFilter};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{core::IntoContainerPort, runners::AsyncRunner},
};
use tonic::{metadata::MetadataValue, transport::Server, Request};
use uuid::Uuid;

use migration::{Migrator, MigratorTrait};
use workspace_crate::{
    entity::{outbox_event, user_snapshot, workspace_invite_link},
    grpc::WorkspaceServer,
};
use relay_types::to_timestamp;

const ACTOR_USER_ID_METADATA: &str = "x-user-id";

#[tokio::test]
async fn create_invite_link_uses_future_expiry_and_rejects_past_expiry()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let actor_user_id = Uuid::new_v4();

    insert_user_snapshot(&env.db, actor_user_id).await?;

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

    let future_expires_at = chrono::Utc::now() + Duration::hours(2);
    let response = env
        .client
        .clone()
        .create_invite_link(actor_request(
            actor_user_id,
            CreateInviteLinkRequest {
                workspace_id: created.workspace_id.clone(),
                expires_at: Some(to_timestamp(future_expires_at)),
                max_uses: Some(3),
            },
        ))
        .await?
        .into_inner();

    assert_eq!(response.workspace_id, created.workspace_id);
    assert_eq!(response.status, "active");
    assert_eq!(response.max_uses, Some(3));
    assert!(response.expires_at.is_some());
    assert_eq!(response.use_count, 0);

    let invite_link_id = Uuid::parse_str(&response.workspace_invite_link_id)?;
    let invite_link = workspace_invite_link::Entity::find_by_id(invite_link_id)
        .one(&env.db)
        .await?
        .expect("invite link row");
    assert_eq!(invite_link.status, "active");
    assert_eq!(invite_link.max_uses, Some(3));
    assert_eq!(invite_link.use_count, 0);

    let outbox_row = outbox_event::Entity::find()
        .filter(outbox_event::Column::AggregateType.eq("workspace_invite_link"))
        .filter(outbox_event::Column::AggregateId.eq(invite_link_id))
        .filter(outbox_event::Column::EventType.eq("WorkspaceInviteLinkCreated".to_string()))
        .one(&env.db)
        .await?
        .expect("invite link outbox row");

    assert_eq!(outbox_row.payload["workspace_invite_link_id"], response.workspace_invite_link_id);
    assert_eq!(outbox_row.payload["status"], "active");
    assert_eq!(outbox_row.payload["max_uses"], 3);
    assert_eq!(outbox_row.payload["use_count"], 0);

    let expired = env
        .client
        .clone()
        .create_invite_link(actor_request(
            actor_user_id,
            CreateInviteLinkRequest {
                workspace_id: created.workspace_id,
                expires_at: Some(to_timestamp(chrono::Utc::now() - Duration::hours(1))),
                max_uses: None,
            },
        ))
        .await;

    assert!(expired.is_err());

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
        user_id: sea_orm::Set(user_id),
        email_verified: sea_orm::Set(false),
        username: sea_orm::Set(format!("user-{user_id}")),
        display_name: sea_orm::Set("Test User".to_string()),
        avatar_url: sea_orm::Set(None),
        created_at: sea_orm::Set(now.into()),
        updated_at: sea_orm::Set(now.into()),
    })
    .exec(db)
    .await?;

    Ok(())
}
