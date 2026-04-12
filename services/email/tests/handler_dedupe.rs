use chrono::Utc;
use email::{amqp::events::VerificationEmailRequested, amqp::handler::Handler, entity::{email_delivery_attempt, outbound_email}, smtp::SmtpClient};
use migration::{Migrator, MigratorTrait};
use sea_orm::{ActiveModelTrait, ColumnTrait, Database, EntityTrait, QueryFilter, Set};
use testcontainers_modules::{postgres::Postgres, testcontainers::{core::IntoContainerPort, runners::AsyncRunner}};
use uuid::Uuid;

#[tokio::test]
async fn duplicate_verification_email_is_ignored() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let user_id = Uuid::new_v4();
    let verification_token_id = Uuid::new_v4().to_string();
    let dedupe_key = format!("verification_email:{verification_token_id}:signup");

    seed_outbound_email(&env.db, &dedupe_key).await?;

    let handler = Handler::new(
        env.db.clone(),
        "https://relay.example.com".to_string(),
        "smtp".to_string(),
        SmtpClient::new(
            "smtp://localhost".to_string(),
            "relay@example.com".to_string(),
            "Relay".to_string(),
        ),
    );

    handler
        .handle_email_event(email::amqp::events::EmailEvent::VerificationEmailRequested(
            VerificationEmailRequested {
                user_id: user_id.to_string(),
                email: "nathan@example.com".to_string(),
                verification_token: "token-123".to_string(),
                verification_token_id,
                verification_token_expires_at: Utc::now().to_rfc3339(),
                reason: "signup".to_string(),
                requested_at: Utc::now().to_rfc3339(),
            },
        ))
        .await?;

    let rows = outbound_email::Entity::find()
        .filter(outbound_email::Column::DedupeKey.eq(&dedupe_key))
        .all(&env.db)
        .await?;
    assert_eq!(rows.len(), 1);

    let attempts = email_delivery_attempt::Entity::find()
        .all(&env.db)
        .await?;
    assert!(attempts.is_empty());

    env.shutdown().await;
    Ok(())
}

struct TestEnv {
    _postgres: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    db: sea_orm::DatabaseConnection,
}

impl TestEnv {
    async fn start() -> Result<Self, Box<dyn std::error::Error>> {
        let postgres = Postgres::default().start().await?;
        let postgres_host = postgres.get_host().await?;
        let postgres_port = postgres.get_host_port_ipv4(5432.tcp()).await?;
        let database_url = format!("postgres://postgres:postgres@{postgres_host}:{postgres_port}/postgres");

        let db = Database::connect(&database_url).await?;
        Migrator::up(&db, None).await?;

        Ok(Self {
            _postgres: postgres,
            db,
        })
    }

    async fn shutdown(self) {}
}

async fn seed_outbound_email(
    db: &sea_orm::DatabaseConnection,
    dedupe_key: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = Utc::now();
    outbound_email::ActiveModel {
        id: Set(Uuid::new_v4()),
        dedupe_key: Set(dedupe_key.to_string()),
        email_kind: Set("registration_verification".to_string()),
        recipient_user_id: Set(Some(Uuid::new_v4())),
        recipient_email: Set("nathan@example.com".to_string()),
        provider_message_id: Set(None),
        provider_name: Set(Some("smtp".to_string())),
        template_key: Set("verify-email-v1".to_string()),
        template_version: Set(1),
        subject: Set("Verify your Relay account".to_string()),
        body_text: Set("plain".to_string()),
        body_html: Set(Some("<p>html</p>".to_string())),
        source_event_type: Set("VerificationEmailRequested".to_string()),
        source_event_id: Set(Uuid::new_v4().to_string()),
        source_occurred_at: Set(now.into()),
        send_status: Set("pending".to_string()),
        last_error_code: Set(None),
        last_error_message: Set(None),
        next_attempt_after: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    Ok(())
}
