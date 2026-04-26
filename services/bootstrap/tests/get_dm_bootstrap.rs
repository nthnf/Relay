extern crate bootstrap as bootstrap_crate;

use bootstrap_crate::{entity::dm_projection, grpc::BootstrapServer};
use migration::{Migrator, MigratorTrait};
use relay_proto::bootstrap::{
    GetDmBootstrapRequest, bootstrap_service_client::BootstrapServiceClient,
};
use sea_orm::{ActiveModelTrait, Database, Set};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{core::IntoContainerPort, runners::AsyncRunner},
};
use tonic::{Request, metadata::MetadataValue, transport::Server};
use uuid::Uuid;

const ACTOR_USER_ID_METADATA: &str = "x-user-id";

#[tokio::test]
async fn get_dm_bootstrap_returns_threads_by_activity() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let actor_user_id = Uuid::new_v4();
    let older_conversation = Uuid::new_v4();
    let newer_conversation = Uuid::new_v4();
    let older_at = chrono::Utc::now() - chrono::Duration::minutes(5);
    let newer_at = chrono::Utc::now();

    insert_dm(
        &env.db,
        actor_user_id,
        older_conversation,
        Uuid::new_v4(),
        "older-peer",
        "Older Peer",
        older_at,
        0,
    )
    .await?;
    insert_dm(
        &env.db,
        actor_user_id,
        newer_conversation,
        Uuid::new_v4(),
        "newer-peer",
        "Newer Peer",
        newer_at,
        4,
    )
    .await?;

    let response = env
        .client
        .clone()
        .get_dm_bootstrap(actor_request(actor_user_id, GetDmBootstrapRequest {}))
        .await?
        .into_inner();

    assert_eq!(response.items.len(), 2);
    assert_eq!(
        response.items[0].conversation_id,
        newer_conversation.to_string()
    );
    assert_eq!(response.items[0].peer_username, "newer-peer");
    assert_eq!(response.items[0].peer_display_name, "Newer Peer");
    assert_eq!(response.items[0].unread_count, 4);
    assert_eq!(response.items[0].last_message_preview, "last message");
    assert!(response.items[0].last_activity_at.is_some());
    assert_eq!(
        response.items[1].conversation_id,
        older_conversation.to_string()
    );

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn get_dm_bootstrap_returns_empty_list_when_projection_lags()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;

    let response = env
        .client
        .clone()
        .get_dm_bootstrap(actor_request(Uuid::new_v4(), GetDmBootstrapRequest {}))
        .await?
        .into_inner();

    assert!(response.items.is_empty());

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

#[allow(clippy::too_many_arguments)]
async fn insert_dm(
    db: &sea_orm::DatabaseConnection,
    user_id: Uuid,
    conversation_id: Uuid,
    peer_user_id: Uuid,
    peer_username: &str,
    peer_display_name: &str,
    last_activity_at: chrono::DateTime<chrono::Utc>,
    unread_count: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    dm_projection::ActiveModel {
        user_id: Set(user_id),
        conversation_id: Set(Some(conversation_id)),
        dm_pair_id: Set(Uuid::new_v4()),
        peer_user_id: Set(peer_user_id),
        peer_username: Set(peer_username.to_string()),
        peer_display_name: Set(peer_display_name.to_string()),
        peer_avatar_url: Set(None),
        last_message_seq: Set(Some(1)),
        last_read_conversation_message_seq: Set(None),
        last_message_preview: Set(Some("last message".to_string())),
        last_activity_at: Set(Some(last_activity_at.into())),
        unread_count: Set(unread_count),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(db)
    .await?;

    Ok(())
}
