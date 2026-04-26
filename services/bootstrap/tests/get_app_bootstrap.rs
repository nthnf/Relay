extern crate bootstrap as bootstrap_crate;

use bootstrap_crate::{
    entity::{user_app_projection, workspace_projection},
    grpc::BootstrapServer,
};
use migration::{Migrator, MigratorTrait};
use relay_proto::bootstrap::{
    GetAppBootstrapRequest, bootstrap_service_client::BootstrapServiceClient,
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
async fn get_app_bootstrap_returns_viewer_and_ordered_workspaces()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let actor_user_id = Uuid::new_v4();
    let workspace_a = Uuid::new_v4();
    let workspace_b = Uuid::new_v4();
    let now = chrono::Utc::now();

    user_app_projection::ActiveModel {
        user_id: Set(actor_user_id),
        username: Set("user1".to_string()),
        display_name: Set("User One".to_string()),
        avatar_url: Set(Some("https://cdn.example/avatar.png".to_string())),
        pending_friend_request_count: Set(3),
        updated_at: Set(now.into()),
    }
    .insert(&env.db)
    .await?;

    insert_workspace(&env.db, actor_user_id, workspace_b, "Zulu", 5).await?;
    insert_workspace(&env.db, actor_user_id, workspace_a, "Alpha", 2).await?;

    let response = env
        .client
        .clone()
        .get_app_bootstrap(actor_request(actor_user_id, GetAppBootstrapRequest {}))
        .await?
        .into_inner();

    let viewer = response.viewer.expect("viewer");
    assert_eq!(viewer.user_id, actor_user_id.to_string());
    assert_eq!(viewer.username, "user1");
    assert_eq!(viewer.display_name, "User One");
    assert_eq!(
        viewer.avatar_url.as_deref(),
        Some("https://cdn.example/avatar.png")
    );
    assert_eq!(response.pending_friend_request_count, 3);

    assert_eq!(response.workspaces.len(), 2);
    assert_eq!(response.workspaces[0].workspace_id, workspace_a.to_string());
    assert_eq!(response.workspaces[0].name, "Alpha");
    assert_eq!(response.workspaces[0].unread_count, 2);
    assert_eq!(response.workspaces[1].workspace_id, workspace_b.to_string());
    assert_eq!(response.workspaces[1].name, "Zulu");
    assert_eq!(response.workspaces[1].unread_count, 5);

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn get_app_bootstrap_returns_defaults_when_projection_lags()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let actor_user_id = Uuid::new_v4();

    let response = env
        .client
        .clone()
        .get_app_bootstrap(actor_request(actor_user_id, GetAppBootstrapRequest {}))
        .await?
        .into_inner();

    let viewer = response.viewer.expect("viewer");
    assert_eq!(viewer.user_id, actor_user_id.to_string());
    assert_eq!(viewer.username, "");
    assert_eq!(viewer.display_name, "");
    assert!(viewer.avatar_url.is_none());
    assert!(response.workspaces.is_empty());
    assert_eq!(response.pending_friend_request_count, 0);

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
        member_count: Set(1),
        unread_count: Set(unread_count),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(db)
    .await?;

    Ok(())
}
