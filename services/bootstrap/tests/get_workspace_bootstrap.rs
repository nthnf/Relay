extern crate bootstrap as bootstrap_crate;

use bootstrap_crate::{
    entity::{workspace_channel_projection, workspace_projection},
    grpc::BootstrapServer,
};
use migration::{Migrator, MigratorTrait};
use relay_proto::bootstrap::{
    GetWorkspaceBootstrapRequest, bootstrap_service_client::BootstrapServiceClient,
};
use sea_orm::{ActiveModelTrait, Database, Set};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{core::IntoContainerPort, runners::AsyncRunner},
};
use tonic::{Code, Request, metadata::MetadataValue, transport::Server};
use uuid::Uuid;

const ACTOR_USER_ID_METADATA: &str = "x-user-id";

#[tokio::test]
async fn get_workspace_bootstrap_returns_header_and_ordered_channels()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let actor_user_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    let channel_a = Uuid::new_v4();
    let channel_b = Uuid::new_v4();
    let channel_without_conversation = Uuid::new_v4();
    let conversation_a = Uuid::new_v4();
    let conversation_b = Uuid::new_v4();

    insert_workspace(&env.db, actor_user_id, workspace_id, "Relay HQ", 3).await?;
    insert_channel(
        &env.db,
        actor_user_id,
        workspace_id,
        channel_b,
        conversation_b,
        "random",
        20,
        1,
    )
    .await?;
    insert_channel_without_conversation(
        &env.db,
        actor_user_id,
        workspace_id,
        channel_without_conversation,
        "pending",
        5,
    )
    .await?;
    insert_channel(
        &env.db,
        actor_user_id,
        workspace_id,
        channel_a,
        conversation_a,
        "general",
        10,
        2,
    )
    .await?;

    let response = env
        .client
        .clone()
        .get_workspace_bootstrap(actor_request(
            actor_user_id,
            GetWorkspaceBootstrapRequest {
                workspace_id: workspace_id.to_string(),
            },
        ))
        .await?
        .into_inner();

    let workspace = response.workspace.expect("workspace");
    assert_eq!(workspace.workspace_id, workspace_id.to_string());
    assert_eq!(workspace.name, "Relay HQ");
    assert_eq!(workspace.member_count, 4);
    assert_eq!(workspace.unread_count, 3);

    assert_eq!(response.channels.len(), 2);
    assert_eq!(response.channels[0].channel_id, channel_a.to_string());
    assert_eq!(
        response.channels[0].conversation_id,
        conversation_a.to_string()
    );
    assert_eq!(response.channels[0].name, "general");
    assert_eq!(response.channels[0].position, 10);
    assert_eq!(response.channels[0].unread_count, 2);
    assert_eq!(response.channels[1].channel_id, channel_b.to_string());
    assert_eq!(response.channels[1].name, "random");

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn get_workspace_bootstrap_returns_not_found_without_actor_projection()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let error = env
        .client
        .clone()
        .get_workspace_bootstrap(actor_request(
            Uuid::new_v4(),
            GetWorkspaceBootstrapRequest {
                workspace_id: Uuid::new_v4().to_string(),
            },
        ))
        .await
        .expect_err("missing actor workspace projection should fail");

    assert_eq!(error.code(), Code::NotFound);

    env.shutdown().await;
    Ok(())
}

struct TestEnv {
    _postgres: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    db: sea_orm::DatabaseConnection,
    client: BootstrapServiceClient<tonic::transport::Channel>,
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

        let handler = BootstrapServer::new(db.clone());
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
) -> Result<BootstrapServiceClient<tonic::transport::Channel>, Box<dyn std::error::Error>> {
    let endpoint = format!("http://{addr}");

    for _ in 0..20 {
        match BootstrapServiceClient::connect(endpoint.clone()).await {
            Ok(client) => return Ok(client),
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(50)).await,
        }
    }

    Ok(BootstrapServiceClient::connect(endpoint).await?)
}

fn actor_request<T>(user_id: Uuid, request: T) -> Request<T> {
    let mut request = Request::new(request);
    request.metadata_mut().insert(
        ACTOR_USER_ID_METADATA,
        MetadataValue::try_from(user_id.to_string()).expect("metadata"),
    );
    request
}

async fn insert_workspace(
    db: &sea_orm::DatabaseConnection,
    user_id: Uuid,
    workspace_id: Uuid,
    name: &str,
    unread_count: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    workspace_projection::ActiveModel {
        user_id: Set(user_id),
        workspace_id: Set(workspace_id),
        workspace_name: Set(name.to_string()),
        workspace_icon_url: Set(None),
        member_count: Set(4),
        unread_count: Set(unread_count),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(db)
    .await?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_channel(
    db: &sea_orm::DatabaseConnection,
    user_id: Uuid,
    workspace_id: Uuid,
    channel_id: Uuid,
    conversation_id: Uuid,
    name: &str,
    position: i32,
    unread_count: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    workspace_channel_projection::ActiveModel {
        user_id: Set(user_id),
        workspace_id: Set(workspace_id),
        channel_id: Set(channel_id),
        conversation_id: Set(Some(conversation_id)),
        channel_name: Set(name.to_string()),
        channel_kind: Set("text".to_string()),
        position: Set(position),
        last_message_seq: Set(Some(1)),
        last_read_conversation_message_seq: Set(None),
        unread_count: Set(unread_count),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(db)
    .await?;

    Ok(())
}

async fn insert_channel_without_conversation(
    db: &sea_orm::DatabaseConnection,
    user_id: Uuid,
    workspace_id: Uuid,
    channel_id: Uuid,
    name: &str,
    position: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    workspace_channel_projection::ActiveModel {
        user_id: Set(user_id),
        workspace_id: Set(workspace_id),
        channel_id: Set(channel_id),
        conversation_id: Set(None),
        channel_name: Set(name.to_string()),
        channel_kind: Set("text".to_string()),
        position: Set(position),
        last_message_seq: Set(None),
        last_read_conversation_message_seq: Set(None),
        unread_count: Set(0),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(db)
    .await?;

    Ok(())
}
