use friendship::{amqp::AmqpHandler, db, entity::user_snapshot};
use lapin::message::Delivery;
use migration::{Migrator, MigratorTrait};
use sea_orm::{ActiveModelTrait, EntityTrait};
use serde_json::json;
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{core::IntoContainerPort, runners::AsyncRunner},
};

#[tokio::test]
async fn handles_user_registered_and_seeds_user_account()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let handler = AmqpHandler::new(env.db.clone());

    let delivery = Delivery::mock(
        1,
        "relay.events".into(),
        "identity.UserRegistered".into(),
        false,
        json!({
            "user_id": env.user_id.to_string(),
            "email": "user1@example.com",
            "email_verified": false,
            "username": "user1",
            "display_name": "User One",
            "avatar_url": null,
            "registered_at": chrono::Utc::now().to_rfc3339(),
        })
        .to_string()
        .into_bytes(),
    );

    handler.handle_delivery(&delivery).await?;

    let row = user_snapshot::Entity::find_by_id(env.user_id)
        .one(&env.db)
        .await?
        .expect("user_account row");

    assert_eq!(row.user_id, env.user_id);
    assert!(!row.email_verified);
    assert_eq!(row.username, "user1");
    assert_eq!(row.display_name, "User One");
    assert_eq!(row.avatar_url, None);
    Ok(())
}

#[tokio::test]
async fn handles_user_profile_updated_and_refreshes_user_snapshot()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let handler = AmqpHandler::new(env.db.clone());

    let delivery = Delivery::mock(
        2,
        "relay.events".into(),
        "identity.UserProfileUpdated".into(),
        false,
        json!({
            "user_id": env.user_id.to_string(),
            "username": "user2",
            "display_name": "User Two",
            "avatar_url": "https://cdn.example/avatar.png",
            "updated_at": chrono::Utc::now().to_rfc3339(),
        })
        .to_string()
        .into_bytes(),
    );

    handler.handle_delivery(&delivery).await?;

    let row = user_snapshot::Entity::find_by_id(env.user_id)
        .one(&env.db)
        .await?
        .expect("user_account row");

    assert_eq!(row.username, "user2");
    assert_eq!(row.display_name, "User Two");
    assert_eq!(row.avatar_url.as_deref(), Some("https://cdn.example/avatar.png"));
    Ok(())
}

#[tokio::test]
async fn handles_user_email_verified_and_marks_verified()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let handler = AmqpHandler::new(env.db.clone());

    let delivery = Delivery::mock(
        2,
        "relay.events".into(),
        "identity.UserEmailVerified".into(),
        false,
        json!({
            "user_id": env.user_id.to_string(),
            "email": "user1@example.com",
            "email_verified_at": chrono::Utc::now().to_rfc3339(),
        })
        .to_string()
        .into_bytes(),
    );

    handler.handle_delivery(&delivery).await?;

    let row = user_snapshot::Entity::find_by_id(env.user_id)
        .one(&env.db)
        .await?
        .expect("user_account row");

    assert_eq!(row.user_id, env.user_id);
    assert!(row.email_verified);
    assert_eq!(row.username, "");
    assert_eq!(row.display_name, "");
    assert_eq!(row.avatar_url, None);
    Ok(())
}

#[tokio::test]
async fn duplicate_user_registered_keeps_existing_row()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let handler = AmqpHandler::new(env.db.clone());

    user_snapshot::ActiveModel {
        user_id: sea_orm::Set(env.user_id),
        email_verified: sea_orm::Set(true),
        username: sea_orm::Set("user1".to_string()),
        display_name: sea_orm::Set("User One".to_string()),
        avatar_url: sea_orm::Set(None),
        created_at: sea_orm::Set(chrono::Utc::now().into()),
        updated_at: sea_orm::Set(chrono::Utc::now().into()),
    }
    .insert(&env.db)
    .await?;

    let delivery = Delivery::mock(
        3,
        "relay.events".into(),
        "identity.UserRegistered".into(),
        false,
        json!({
            "user_id": env.user_id.to_string(),
            "email": "user1@example.com",
            "email_verified": false,
            "username": "user1",
            "display_name": "User One",
            "avatar_url": null,
            "registered_at": chrono::Utc::now().to_rfc3339(),
        })
        .to_string()
        .into_bytes(),
    );

    handler.handle_delivery(&delivery).await?;

    let row = user_snapshot::Entity::find_by_id(env.user_id)
        .one(&env.db)
        .await?
        .expect("user_account row");

    assert!(row.email_verified);
    Ok(())
}

struct TestEnv {
    _postgres: testcontainers_modules::testcontainers::ContainerAsync<
        testcontainers_modules::postgres::Postgres,
    >,
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
