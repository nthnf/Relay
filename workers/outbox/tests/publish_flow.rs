use chrono::{Duration as ChronoDuration, Utc};
use lapin::{
    options::{BasicAckOptions, BasicGetOptions, ConfirmSelectOptions, QueueBindOptions, QueueDeclareOptions},
    types::FieldTable,
    Connection, ConnectionProperties,
};
use outbox::{config::Config, entity::outbox_event, worker};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, Database, EntityTrait, QueryFilter, Set,
};
use testcontainers_modules::{
    postgres::Postgres,
    rabbitmq::RabbitMq,
    testcontainers::{core::IntoContainerPort, runners::AsyncRunner},
};
use uuid::Uuid;

#[tokio::test]
async fn publishes_pending_row_to_rabbitmq_and_marks_published() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let event_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    insert_outbox_row(&env.db, pending_row(event_id, user_id, "UserRegistered")).await?;

    worker::declare_exchange(&env.channel, &env.config.exchange).await?;
    declare_test_queue(&env.channel, &env.config.exchange, &env.queue_name, "identity.UserRegistered").await?;
    worker::publish_batch_once(&env.db, &env.channel, &env.config).await?;

    let stored = outbox_event::Entity::find_by_id(event_id)
        .one(&env.db)
        .await?
        .expect("stored outbox row");
    assert_eq!(stored.status, "published");
    assert!(stored.published_at.is_some());

    let delivery = env
        .channel
        .basic_get(env.queue_name.as_str().into(), BasicGetOptions::default())
        .await?
        .expect("published message");
    assert_eq!(String::from_utf8(delivery.data.clone())?, serde_json::json!({
        "event_id": event_id,
        "user_id": user_id,
        "email": "nathan@example.com"
    }).to_string());
    delivery.ack(BasicAckOptions::default()).await?;

    Ok(())
}

#[tokio::test]
async fn reclaims_expired_claim_and_publishes_it() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let event_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    insert_outbox_row(
        &env.db,
        expired_claim_row(event_id, user_id, "VerificationEmailRequested"),
    )
    .await?;

    worker::declare_exchange(&env.channel, &env.config.exchange).await?;
    declare_test_queue(
        &env.channel,
        &env.config.exchange,
        &env.queue_name,
        "identity.VerificationEmailRequested",
    )
    .await?;
    worker::publish_batch_once(&env.db, &env.channel, &env.config).await?;

    let stored = outbox_event::Entity::find()
        .filter(outbox_event::Column::EventId.eq(event_id))
        .one(&env.db)
        .await?
        .expect("stored outbox row");
    assert_eq!(stored.status, "published");
    assert_eq!(stored.claimed_by, None);
    assert_eq!(stored.publish_attempts, 4);

    let delivery = env
        .channel
        .basic_get(env.queue_name.as_str().into(), BasicGetOptions::default())
        .await?
        .expect("reclaimed message");
    delivery.ack(BasicAckOptions::default()).await?;

    Ok(())
}

struct TestEnv {
    _postgres: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    _rabbitmq: testcontainers_modules::testcontainers::ContainerAsync<RabbitMq>,
    db: sea_orm::DatabaseConnection,
    channel: lapin::Channel,
    config: Config,
    queue_name: String,
}

impl TestEnv {
    async fn start() -> Result<Self, Box<dyn std::error::Error>> {
        let postgres = Postgres::default().start().await?;
        let rabbitmq = RabbitMq::default().start().await?;

        let postgres_host = postgres.get_host().await?;
        let postgres_port = postgres.get_host_port_ipv4(5432.tcp()).await?;
        let rabbitmq_host = rabbitmq.get_host().await?;
        let rabbitmq_port = rabbitmq.get_host_port_ipv4(5672.tcp()).await?;

        let database_url = format!(
            "postgres://postgres:postgres@{postgres_host}:{postgres_port}/postgres"
        );
        let amqp_addr = format!("amqp://{rabbitmq_host}:{rabbitmq_port}/%2f");
        let db = Database::connect(&database_url).await?;

        db.execute_unprepared(OUTBOX_EVENT_TABLE_SQL).await?;

        let rabbit_conn = Connection::connect(&amqp_addr, ConnectionProperties::default()).await?;
        let channel = rabbit_conn.create_channel().await?;
        channel
            .confirm_select(ConfirmSelectOptions::default())
            .await?;

        let queue_id = Uuid::new_v4();
        Ok(Self {
            _postgres: postgres,
            _rabbitmq: rabbitmq,
            db,
            channel,
            config: Config {
                database_url,
                amqp_addr,
                exchange: "relay.events".to_string(),
                publisher_service: "identity".to_string(),
                batch_size: 10,
                poll_interval: std::time::Duration::from_millis(50),
                claim_ttl: std::time::Duration::from_secs(30),
                retry_delay: std::time::Duration::from_secs(5),
                max_publish_attempts: 10,
                worker_id: "identity:test-worker".to_string(),
            },
            queue_name: format!("test.queue.{queue_id}"),
        })
    }
}

async fn declare_test_queue(
    channel: &lapin::Channel,
    exchange: &str,
    queue_name: &str,
    routing_key: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    channel
        .queue_declare(
            queue_name.into(),
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;
    channel
        .queue_bind(
            queue_name.into(),
            exchange.into(),
            routing_key.into(),
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await?;
    Ok(())
}

async fn insert_outbox_row(
    db: &sea_orm::DatabaseConnection,
    row: outbox_event::ActiveModel,
) -> Result<(), Box<dyn std::error::Error>> {
    row.insert(db).await?;
    Ok(())
}

fn pending_row(event_id: Uuid, user_id: Uuid, event_type: &str) -> outbox_event::ActiveModel {
    let now = Utc::now();
    outbox_event::ActiveModel {
        event_id: Set(event_id),
        aggregate_type: Set("user_account".to_string()),
        aggregate_id: Set(Uuid::new_v4()),
        event_type: Set(event_type.to_string()),
        payload: Set(serde_json::json!({
            "event_id": event_id,
            "user_id": user_id,
            "email": "nathan@example.com"
        })),
        status: Set("pending".to_string()),
        publish_attempts: Set(0),
        occurred_at: Set(now.into()),
        available_at: Set(now.into()),
        claimed_by: Set(None),
        claimed_at: Set(None),
        published_at: Set(None),
        last_error: Set(None),
        created_at: Set(now.into()),
        ..Default::default()
    }
}

fn expired_claim_row(event_id: Uuid, user_id: Uuid, event_type: &str) -> outbox_event::ActiveModel {
    let now = Utc::now();
    outbox_event::ActiveModel {
        event_id: Set(event_id),
        aggregate_type: Set("user_account".to_string()),
        aggregate_id: Set(Uuid::new_v4()),
        event_type: Set(event_type.to_string()),
        payload: Set(serde_json::json!({
            "event_id": event_id,
            "user_id": user_id,
            "email": "nathan@example.com"
        })),
        status: Set("claimed".to_string()),
        publish_attempts: Set(3),
        occurred_at: Set(now.into()),
        available_at: Set((now - ChronoDuration::minutes(1)).into()),
        claimed_by: Set(Some("identity:stale-worker".to_string())),
        claimed_at: Set(Some((now - ChronoDuration::minutes(1)).into())),
        published_at: Set(None),
        last_error: Set(Some("worker crashed".to_string())),
        created_at: Set((now - ChronoDuration::minutes(1)).into()),
        ..Default::default()
    }
}

const OUTBOX_EVENT_TABLE_SQL: &str = r#"
CREATE TABLE outbox_event (
    event_id uuid PRIMARY KEY,
    aggregate_type text NOT NULL,
    aggregate_id uuid NOT NULL,
    event_type text NOT NULL,
    payload jsonb NOT NULL,
    status text NOT NULL,
    publish_attempts integer NOT NULL,
    occurred_at timestamptz NOT NULL,
    available_at timestamptz NOT NULL,
    claimed_by text NULL,
    claimed_at timestamptz NULL,
    published_at timestamptz NULL,
    last_error text NULL,
    created_at timestamptz NOT NULL
)
"#;
