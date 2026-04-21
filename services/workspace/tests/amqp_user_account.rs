use migration::{Migrator, MigratorTrait};
use relay_amqp::DeliveryContext;
use sea_orm::EntityTrait;
use std::sync::Arc;
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{core::IntoContainerPort, runners::AsyncRunner},
};

use workspace::{
    amqp::AmqpHandler,
    db,
    entity::user_snapshot,
    events::{UserEmailVerifiedPayload, UserProfileUpdatedPayload, UserRegisteredPayload},
};

#[tokio::test]
async fn handles_user_registered_user_profile_updated_and_verified()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let handler = Arc::new(AmqpHandler::new(env.db.clone()));

    handler
        .clone()
        .handle_user_registered(
            DeliveryContext::default(),
            UserRegisteredPayload {
                user_id: env.user_id.to_string(),
                email: "user1@example.com".to_string(),
                email_verified: false,
                username: "user1".to_string(),
                display_name: "User One".to_string(),
                avatar_url: None,
                registered_at: chrono::Utc::now().to_rfc3339(),
            },
        )
        .await?;

    handler
        .clone()
        .handle_user_profile_updated(
            DeliveryContext::default(),
            UserProfileUpdatedPayload {
                user_id: env.user_id.to_string(),
                username: "user2".to_string(),
                display_name: "User Two".to_string(),
                avatar_url: Some("https://cdn.example/avatar.png".to_string()),
                updated_at: chrono::Utc::now().to_rfc3339(),
            },
        )
        .await?;

    handler
        .clone()
        .handle_user_email_verified(
            DeliveryContext::default(),
            UserEmailVerifiedPayload {
                user_id: env.user_id.to_string(),
                email: "user1@example.com".to_string(),
                email_verified_at: chrono::Utc::now().to_rfc3339(),
            },
        )
        .await?;

    let row = user_snapshot::Entity::find_by_id(env.user_id)
        .one(&env.db)
        .await?
        .expect("user snapshot row");

    assert!(row.email_verified);
    assert_eq!(row.username, "user2");
    assert_eq!(row.display_name, "User Two");
    assert_eq!(
        row.avatar_url.as_deref(),
        Some("https://cdn.example/avatar.png")
    );

    Ok(())
}

struct TestEnv {
    _postgres: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    db: sea_orm::DatabaseConnection,
    user_id: uuid::Uuid,
}

impl TestEnv {
    async fn start() -> Result<Self, Box<dyn std::error::Error>> {
        let postgres: testcontainers_modules::testcontainers::ContainerAsync<Postgres> =
            Postgres::default().start().await?;
        let postgres_host = postgres.get_host().await?;
        let postgres_port = postgres.get_host_port_ipv4(5432_u16.tcp()).await?;
        let database_url =
            format!("postgres://postgres:postgres@{postgres_host}:{postgres_port}/postgres");

        let db = db::connect(&database_url).await?;
        Migrator::up(&db, None).await?;

        Ok(Self {
            _postgres: postgres,
            db,
            user_id: uuid::Uuid::new_v4(),
        })
    }
}
