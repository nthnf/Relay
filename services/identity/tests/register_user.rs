use identity::{
    auth::AuthKeys,
    entity::{outbox_event, user_account, user_profile},
    grpc::identity::IdentityServer,
};
use migration::{Migrator, MigratorTrait};
use relay_proto::identity::{
    identity_service_client::IdentityServiceClient, RegisterUserRequest,
};
use sea_orm::{ColumnTrait, Database, EntityTrait, QueryFilter};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{core::IntoContainerPort, runners::AsyncRunner},
};
use tonic::transport::Server;

#[tokio::test]
async fn register_user_persists_identity_state_and_outbox_events(
) -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;

    let response = env
        .client
        .clone()
        .register_user(RegisterUserRequest {
            email: "nathan@example.com".to_string(),
            password: "correct horse battery staple".to_string(),
            username: "nathan".to_string(),
            display_name: "Nathan".to_string(),
            avatar_url: Some("https://example.com/avatar.png".to_string()),
        })
        .await?
        .into_inner();

    assert_eq!(response.email, "nathan@example.com");
    assert!(!response.user_id.is_empty());
    assert!(response.verification_email_requested_at.is_some());

    let account = user_account::Entity::find()
        .filter(user_account::Column::EmailNormalized.eq("nathan@example.com"))
        .one(&env.db)
        .await?
        .expect("user account row");
    assert_eq!(account.email, "nathan@example.com");
    assert_eq!(account.user_id.to_string(), response.user_id);

    let profile = user_profile::Entity::find_by_id(account.user_id)
        .one(&env.db)
        .await?
        .expect("user profile row");
    assert_eq!(profile.username, "nathan");
    assert_eq!(profile.display_name, "Nathan");

    let outbox_rows = outbox_event::Entity::find()
        .filter(outbox_event::Column::AggregateId.eq(account.user_id))
        .all(&env.db)
        .await?;
    assert_eq!(outbox_rows.len(), 2);
    assert!(outbox_rows.iter().any(|row| row.event_type == "UserRegistered"));
    assert!(outbox_rows
        .iter()
        .any(|row| row.event_type == "VerificationEmailRequested"));

    env.shutdown().await;
    Ok(())
}

struct TestEnv {
    _postgres: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    db: sea_orm::DatabaseConnection,
    client: IdentityServiceClient<tonic::transport::Channel>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    server_task: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
}

impl TestEnv {
    async fn start() -> Result<Self, Box<dyn std::error::Error>> {
        let postgres = Postgres::default().start().await?;
        let postgres_host = postgres.get_host().await?;
        let postgres_port = postgres.get_host_port_ipv4(5432.tcp()).await?;
        let database_url = format!(
            "postgres://postgres:postgres@{postgres_host}:{postgres_port}/postgres"
        );

        let db = Database::connect(&database_url).await?;
        Migrator::up(&db, None).await?;

        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        drop(listener);

        let service = IdentityServer::new(
            db.clone(),
            AuthKeys::from_shared_secret(b"test-secret-key"),
        );
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        let server_task = tokio::spawn(async move {
            Server::builder()
                .add_service(service.into_server())
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
) -> Result<IdentityServiceClient<tonic::transport::Channel>, Box<dyn std::error::Error>> {
    let endpoint = format!("http://{addr}");

    for _ in 0..20 {
        match IdentityServiceClient::connect(endpoint.clone()).await {
            Ok(client) => return Ok(client),
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(50)).await,
        }
    }

    Ok(IdentityServiceClient::connect(endpoint).await?)
}
