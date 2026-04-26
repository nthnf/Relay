use chrono::{DateTime, Utc};
use envoy_types::pb::envoy::service::auth::v3::authorization_client::AuthorizationClient;
use identity::{
    auth::{AuthKeys, hash_password, hash_token},
    entity::{
        email_verification_token, outbox_event, user_account, user_credential_password,
        user_profile, user_session,
    },
    grpc::IdentityServer,
};
use migration::{Migrator, MigratorTrait};
use relay_proto::identity::identity_service_client::IdentityServiceClient;
use sea_orm::{ColumnTrait, Database, DatabaseConnection, EntityTrait, QueryFilter, Set};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{core::IntoContainerPort, runners::AsyncRunner},
};
use tonic::transport::Server;
use uuid::Uuid;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

pub struct TestEnv {
    _postgres: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    pub db: DatabaseConnection,
    pub client: IdentityServiceClient<tonic::transport::Channel>,
    pub auth_client: AuthorizationClient<tonic::transport::Channel>,
    pub listen_addr: SocketAddr,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    server_task: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
}

impl TestEnv {
    pub async fn start() -> Result<Self, Box<dyn std::error::Error>> {
        Self::start_with_bind_ip(IpAddr::V4(Ipv4Addr::LOCALHOST)).await
    }

    pub async fn start_with_bind_ip(bind_ip: IpAddr) -> Result<Self, Box<dyn std::error::Error>> {
        let postgres = Postgres::default().start().await?;
        let postgres_host = postgres.get_host().await?;
        let postgres_port = postgres.get_host_port_ipv4(5432.tcp()).await?;
        let database_url =
            format!("postgres://postgres:postgres@{postgres_host}:{postgres_port}/postgres");

        let db = Database::connect(&database_url).await?;
        Migrator::up(&db, None).await?;

        let listener = std::net::TcpListener::bind(SocketAddr::new(bind_ip, 0))?;
        let addr = listener.local_addr()?;
        drop(listener);

        let service = IdentityServer::new(db.clone(), auth_keys());
        let auth_service = service.clone();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        let server_task = tokio::spawn(async move {
            Server::builder()
                .add_service(service.into_server())
                .add_service(auth_service.into_auth_server())
                .serve_with_shutdown(addr, async {
                    let _ = shutdown_rx.await;
                })
                .await
        });

        let client_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), addr.port());
        let client = connect_identity_client(client_addr).await?;
        let auth_client = connect_auth_client(client_addr).await?;

        Ok(Self {
            _postgres: postgres,
            db,
            client,
            auth_client,
            listen_addr: addr,
            shutdown: Some(shutdown_tx),
            server_task,
        })
    }

    pub async fn shutdown(mut self) {
        if let Some(sender) = self.shutdown.take() {
            let _ = sender.send(());
        }

        let _ = self.server_task.await;
    }
}

pub fn auth_keys() -> AuthKeys {
    AuthKeys::from_shared_secret(b"test-secret-key")
}

pub fn access_token(user_id: Uuid, session_id: Uuid) -> String {
    auth_keys()
        .sign_access_token(identity::auth::AccessClaims {
            user_id,
            session_id,
        })
        .expect("token signing should succeed")
}

pub fn hash_password_for_test(password: &str) -> String {
    hash_password(password).expect("hashing should succeed")
}

pub fn hash_token_for_test(token: &str) -> String {
    hash_token(token)
}

pub async fn insert_user_account(
    db: &DatabaseConnection,
    user_id: Uuid,
    email: &str,
    email_verified_at: Option<DateTime<Utc>>,
    account_status: &str,
    now: DateTime<Utc>,
) -> Result<(), sea_orm::DbErr> {
    user_account::Entity::insert(user_account::ActiveModel {
        user_id: Set(user_id),
        email: Set(email.to_string()),
        email_normalized: Set(email.to_lowercase()),
        email_verified_at: Set(email_verified_at.map(Into::into)),
        account_status: Set(account_status.to_string()),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    })
    .exec(db)
    .await
    .map(|_| ())
}

pub async fn insert_user_profile(
    db: &DatabaseConnection,
    user_id: Uuid,
    username: &str,
    display_name: &str,
    avatar_url: Option<&str>,
    now: DateTime<Utc>,
) -> Result<(), sea_orm::DbErr> {
    user_profile::Entity::insert(user_profile::ActiveModel {
        user_id: Set(user_id),
        username: Set(username.to_string()),
        display_name: Set(display_name.to_string()),
        avatar_url: Set(avatar_url.map(str::to_string)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    })
    .exec(db)
    .await
    .map(|_| ())
}

pub async fn insert_password_credential(
    db: &DatabaseConnection,
    user_id: Uuid,
    password_hash: String,
    now: DateTime<Utc>,
) -> Result<(), sea_orm::DbErr> {
    user_credential_password::Entity::insert(user_credential_password::ActiveModel {
        user_id: Set(user_id),
        password_hash: Set(password_hash),
        password_updated_at: Set(now.into()),
        failed_attempt_count: Set(0),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    })
    .exec(db)
    .await
    .map(|_| ())
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_user_session(
    db: &DatabaseConnection,
    session_id: Uuid,
    user_id: Uuid,
    refresh_token_hash: String,
    issued_at: DateTime<Utc>,
    refresh_expires_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
    revoke_reason: Option<&str>,
    replaced_by_session_id: Option<Uuid>,
    client_instance_id: Option<Uuid>,
) -> Result<(), sea_orm::DbErr> {
    user_session::Entity::insert(user_session::ActiveModel {
        session_id: Set(session_id),
        user_id: Set(user_id),
        refresh_token_hash: Set(refresh_token_hash),
        issued_at: Set(issued_at.into()),
        refresh_expires_at: Set(refresh_expires_at.into()),
        revoked_at: Set(revoked_at.map(Into::into)),
        revoke_reason: Set(revoke_reason.map(str::to_string)),
        replaced_by_session_id: Set(replaced_by_session_id),
        client_instance_id: Set(client_instance_id),
        created_at: Set(issued_at.into()),
    })
    .exec(db)
    .await
    .map(|_| ())
}

pub async fn insert_email_verification_token(
    db: &DatabaseConnection,
    token_id: Uuid,
    user_id: Uuid,
    token_hash: String,
    expires_at: DateTime<Utc>,
    consumed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
) -> Result<(), sea_orm::DbErr> {
    email_verification_token::Entity::insert(email_verification_token::ActiveModel {
        token_id: Set(token_id),
        user_id: Set(user_id),
        token_hash: Set(token_hash),
        expires_at: Set(expires_at.into()),
        consumed_at: Set(consumed_at.map(Into::into)),
        created_at: Set(created_at.into()),
    })
    .exec(db)
    .await
    .map(|_| ())
}

pub async fn count_outbox_events(
    db: &DatabaseConnection,
    aggregate_id: Uuid,
) -> Result<Vec<outbox_event::Model>, sea_orm::DbErr> {
    outbox_event::Entity::find()
        .filter(outbox_event::Column::AggregateId.eq(aggregate_id))
        .all(db)
        .await
}

async fn connect_identity_client(
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

async fn connect_auth_client(
    addr: std::net::SocketAddr,
) -> Result<AuthorizationClient<tonic::transport::Channel>, Box<dyn std::error::Error>> {
    let endpoint = format!("http://{addr}");

    for _ in 0..20 {
        match AuthorizationClient::connect(endpoint.clone()).await {
            Ok(client) => return Ok(client),
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(50)).await,
        }
    }

    Ok(AuthorizationClient::connect(endpoint).await?)
}
