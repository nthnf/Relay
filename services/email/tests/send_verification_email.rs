use chrono::{Duration as ChronoDuration, Utc};
use email::{amqp::Handler, entity::outbound_email, smtp::SmtpClient};
use futures_util::StreamExt;
use lapin::{
    BasicProperties, Connection, ConnectionProperties, ExchangeKind,
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicPublishOptions, ConfirmSelectOptions,
        ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions,
    },
    types::FieldTable,
};
use migration::{Migrator, MigratorTrait};
use relay_amqp::{AmqpSubscriber, DeliveryContext};
use reqwest::Client;
use sea_orm::{ColumnTrait, Database, EntityTrait, QueryFilter};
use serde_json::Value;
use std::{collections::HashMap, time::Duration};
use testcontainers_modules::{
    postgres::Postgres,
    rabbitmq::RabbitMq,
    testcontainers::{
        GenericImage,
        core::{IntoContainerPort, WaitFor},
        runners::AsyncRunner,
    },
};
use uuid::Uuid;

#[tokio::test]
async fn publishes_verification_email_to_mailpit() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let channel = env.amqp_channel().await?;
    let queue_name = format!("email.test.{}", Uuid::new_v4());
    let route_key = "identity.VerificationEmailRequested";
    let event = verification_event();
    let body = serde_json::json!({
        "user_id": event.user_id,
        "email": event.email,
        "verification_token": event.verification_token,
        "verification_token_id": event.verification_token_id,
        "verification_token_expires_at": event.verification_token_expires_at,
        "reason": event.reason,
        "requested_at": event.requested_at,
    })
    .to_string();

    let handler = Handler::new(
        env.db.clone(),
        "https://relay.example.com".to_string(),
        "mailpit".to_string(),
        SmtpClient::new(
            env.smtp_url.clone(),
            "relay@example.com".to_string(),
            "Relay".to_string(),
        ),
    );

    let subscriber = AmqpSubscriber::topic(
        "email",
        queue_name.clone(),
        "email-test-consumer",
        env.exchange.clone(),
        route_key,
    )
    .handle(handler);

    channel
        .exchange_declare(
            env.exchange.clone().into(),
            ExchangeKind::Topic,
            ExchangeDeclareOptions {
                durable: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;
    channel
        .queue_declare(
            queue_name.clone().into(),
            QueueDeclareOptions {
                durable: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;
    channel
        .queue_bind(
            queue_name.clone().into(),
            env.exchange.clone().into(),
            route_key.into(),
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await?;
    channel
        .confirm_select(ConfirmSelectOptions::default())
        .await?;

    channel
        .basic_publish(
            env.exchange.as_str().into(),
            route_key.into(),
            BasicPublishOptions::default(),
            body.as_bytes(),
            BasicProperties::default(),
        )
        .await?
        .await?;

    let mut consumer = channel
        .basic_consume(
            queue_name.as_str().into(),
            "email-test-consumer".into(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    let delivery = tokio::time::timeout(Duration::from_secs(15), consumer.next())
        .await?
        .ok_or("consumer ended early")??;

    let context = DeliveryContext {
        routing_key: delivery.routing_key.to_string(),
        message_id: None,
        correlation_id: None,
        headers: HashMap::new(),
    };

    subscriber
        .dispatch_for_test(context, delivery.data.clone())
        .await?;
    delivery.ack(BasicAckOptions::default()).await?;

    let captured = wait_for_mailpit_message(&env.http_client, &env.mailpit_api_url).await?;

    assert!(captured.contains("Verify your Relay account"), "{captured}");
    assert!(captured.contains(&event.email), "{captured}");
    assert!(captured.contains(&event.verification_token), "{captured}");
    assert!(
        captured.contains(&event.verification_token_expires_at),
        "{captured}"
    );
    assert!(
        captured.contains("Verify your email for Relay"),
        "{captured}"
    );

    let stored = outbound_email::Entity::find()
        .filter(outbound_email::Column::DedupeKey.eq(format!(
            "verification_email:{}:{}",
            event.verification_token_id, event.reason
        )))
        .one(&env.db)
        .await?
        .ok_or("missing outbound email row")?;
    assert_eq!(stored.send_status, "submitted");
    assert_eq!(stored.subject, "Verify your Relay account");

    Ok(())
}

struct TestEnv {
    _postgres: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    _rabbitmq: testcontainers_modules::testcontainers::ContainerAsync<RabbitMq>,
    _mailpit: testcontainers_modules::testcontainers::ContainerAsync<GenericImage>,
    db: sea_orm::DatabaseConnection,
    amqp_addr: String,
    smtp_url: String,
    mailpit_api_url: String,
    exchange: String,
    http_client: Client,
}

impl TestEnv {
    async fn start() -> Result<Self, Box<dyn std::error::Error>> {
        let postgres = Postgres::default().start().await?;
        let rabbitmq = RabbitMq::default().start().await?;
        let mailpit = GenericImage::new("axllent/mailpit", "latest")
            .with_exposed_port(1025.tcp())
            .with_exposed_port(8025.tcp())
            .with_wait_for(WaitFor::seconds(2))
            .start()
            .await?;

        let postgres_host = postgres.get_host().await?;
        let postgres_port = postgres.get_host_port_ipv4(5432.tcp()).await?;
        let rabbitmq_host = rabbitmq.get_host().await?;
        let rabbitmq_port = rabbitmq.get_host_port_ipv4(5672.tcp()).await?;
        let mailpit_host = mailpit.get_host().await?;
        let smtp_port = mailpit.get_host_port_ipv4(1025.tcp()).await?;
        let http_port = mailpit.get_host_port_ipv4(8025.tcp()).await?;

        let database_url =
            format!("postgres://postgres:postgres@{postgres_host}:{postgres_port}/postgres");
        let amqp_addr = format!("amqp://{rabbitmq_host}:{rabbitmq_port}/%2f");
        let smtp_url = format!("smtp://{mailpit_host}:{smtp_port}");
        let mailpit_api_url = format!("http://{mailpit_host}:{http_port}");

        let db = Database::connect(&database_url).await?;
        Migrator::up(&db, None).await?;

        Ok(Self {
            _postgres: postgres,
            _rabbitmq: rabbitmq,
            _mailpit: mailpit,
            db,
            amqp_addr,
            smtp_url,
            mailpit_api_url,
            exchange: "relay.events".to_string(),
            http_client: Client::new(),
        })
    }

    async fn amqp_channel(&self) -> Result<lapin::Channel, Box<dyn std::error::Error>> {
        let connection =
            Connection::connect(&self.amqp_addr, ConnectionProperties::default()).await?;
        let channel = connection.create_channel().await?;
        Ok(channel)
    }
}

struct VerificationFixture {
    user_id: String,
    email: String,
    verification_token: String,
    verification_token_id: String,
    verification_token_expires_at: String,
    reason: String,
    requested_at: String,
}

fn verification_event() -> VerificationFixture {
    let requested_at = Utc::now();
    let expires_at = requested_at + ChronoDuration::minutes(15);

    VerificationFixture {
        user_id: Uuid::new_v4().to_string(),
        email: "user1@example.com".to_string(),
        verification_token: "token-123".to_string(),
        verification_token_id: Uuid::new_v4().to_string(),
        verification_token_expires_at: expires_at.to_rfc3339(),
        reason: "signup".to_string(),
        requested_at: requested_at.to_rfc3339(),
    }
}

async fn wait_for_mailpit_message(
    client: &Client,
    base_url: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);

    loop {
        let list_url = format!("{base_url}/api/v1/messages");
        if let Ok(response) = client.get(&list_url).send().await
            && let Ok(response) = response.error_for_status()
        {
            let body = response.text().await?;
            if let Some(message_id) = extract_message_id(&body) {
                for detail_url in [
                    format!("{base_url}/api/v1/message/{message_id}"),
                    format!("{base_url}/api/v1/messages/{message_id}"),
                ] {
                    if let Ok(detail_response) = client.get(&detail_url).send().await
                        && let Ok(detail_response) = detail_response.error_for_status()
                    {
                        let detail = detail_response.text().await?;
                        if detail.contains("Verify your Relay account")
                            || detail.contains("token-123")
                            || detail.contains("Verify your email for Relay")
                        {
                            return Ok(detail);
                        }
                    }
                }
            }

            if body.contains("Verify your Relay account") || body.contains("token-123") {
                return Ok(body);
            }
        }

        if tokio::time::Instant::now() >= deadline {
            return Err("mailpit did not receive the message".into());
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

fn extract_message_id(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    extract_message_id_value(&value)
}

fn extract_message_id_value(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in ["id", "ID", "message_id", "MessageID"] {
                if let Some(Value::String(id)) = map.get(key) {
                    return Some(id.clone());
                }
            }

            map.values().find_map(extract_message_id_value)
        }
        Value::Array(values) => values.iter().find_map(extract_message_id_value),
        _ => None,
    }
}
