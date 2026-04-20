extern crate workspace as workspace_crate;

use relay_proto::workspace::CreateWorkspaceRequest;
use relay_proto::workspace::workspace_service_client::WorkspaceServiceClient;
use sea_orm::{ColumnTrait, Database, EntityTrait, QueryFilter};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{core::IntoContainerPort, runners::AsyncRunner},
};
use tonic::{Request, metadata::MetadataValue, transport::Server};
use uuid::Uuid;

use migration::{Migrator, MigratorTrait};
use workspace_crate::{
    entity::{
        outbox_event, user_snapshot, workspace, workspace_channel, workspace_member,
        workspace_member_role, workspace_role,
    },
    grpc::WorkspaceServer,
};

const ACTOR_USER_ID_METADATA: &str = "x-user-id";

#[tokio::test]
async fn create_workspace_persists_workspace_member_channel_roles_and_events()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let actor_user_id = Uuid::new_v4();

    insert_user_snapshot(&env.db, actor_user_id).await?;

    let response = env
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

    assert_eq!(response.name, "Acme");
    assert_eq!(response.owner_user_id, actor_user_id.to_string());
    assert_eq!(response.initial_member_user_id, actor_user_id.to_string());
    assert_eq!(response.first_channel_id.len(), 36);

    let workspace_id = Uuid::parse_str(&response.workspace_id)?;
    let channel_id = Uuid::parse_str(&response.first_channel_id)?;

    let workspace_row: workspace::Model = workspace::Entity::find_by_id(workspace_id)
        .one(&env.db)
        .await?
        .expect("workspace row");
    assert_eq!(workspace_row.owner_user_id, actor_user_id);
    assert_eq!(workspace_row.name, "Acme");

    let channel_row: workspace_channel::Model = workspace_channel::Entity::find_by_id(channel_id)
        .one(&env.db)
        .await?
        .expect("workspace channel row");
    assert_eq!(channel_row.workspace_id, workspace_id);
    assert_eq!(channel_row.name, "general");
    assert_eq!(channel_row.channel_kind, "text");
    assert_eq!(channel_row.position, 1);
    assert_eq!(channel_row.created_by_user_id, actor_user_id);

    let member_row: workspace_member::Model = workspace_member::Entity::find()
        .filter(workspace_member::Column::WorkspaceId.eq(workspace_id))
        .filter(workspace_member::Column::UserId.eq(actor_user_id))
        .one(&env.db)
        .await?
        .expect("workspace member row");
    assert_eq!(member_row.membership_status, "active");
    assert_eq!(member_row.added_by_user_id, Some(actor_user_id));

    let role_rows: Vec<workspace_role::Model> = workspace_role::Entity::find()
        .filter(workspace_role::Column::WorkspaceId.eq(workspace_id))
        .all(&env.db)
        .await?;
    assert_eq!(role_rows.len(), 3);
    assert!(
        role_rows
            .iter()
            .any(|row| row.name == "Owner" && row.permissions == 0b1111_1111_1111)
    );
    assert!(
        role_rows
            .iter()
            .any(|row| row.name == "Admin" && row.permissions == 0b0111_1111_1111)
    );
    assert!(
        role_rows
            .iter()
            .any(|row| row.name == "Member" && row.permissions == 0b0000_0000_0001)
    );

    let member_role_rows: Vec<workspace_member_role::Model> = workspace_member_role::Entity::find()
        .filter(workspace_member_role::Column::WorkspaceId.eq(workspace_id))
        .filter(workspace_member_role::Column::UserId.eq(actor_user_id))
        .all(&env.db)
        .await?;
    assert_eq!(member_role_rows.len(), 1);
    assert_eq!(
        member_role_rows[0].role_id,
        role_rows.iter().find(|row| row.name == "Owner").unwrap().id
    );

    let outbox_rows: Vec<outbox_event::Model> = outbox_event::Entity::find().all(&env.db).await?;
    assert_eq!(outbox_rows.len(), 3);
    assert!(
        outbox_rows
            .iter()
            .any(|row| row.event_type == "WorkspaceCreated")
    );
    assert!(
        outbox_rows
            .iter()
            .any(|row| row.event_type == "WorkspaceMemberAdded")
    );
    assert!(
        outbox_rows
            .iter()
            .any(|row| row.event_type == "WorkspaceChannelCreated")
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
