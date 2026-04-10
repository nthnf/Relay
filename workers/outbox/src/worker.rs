use crate::{config::Config, entity::outbox_event};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use lapin::{
    options::{BasicPublishOptions, ConfirmSelectOptions, ExchangeDeclareOptions},
    Confirmation,
    types::{AMQPValue, FieldTable, LongString, ShortString},
    BasicProperties, Channel, Connection, ConnectionProperties, ExchangeKind,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, Database, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use sea_orm::sea_query::{LockBehavior, LockType};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

pub async fn run(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::connect(&config.database_url).await?;
    let conn = Connection::connect(&config.amqp_addr, ConnectionProperties::default()).await?;
    let channel = conn.create_channel().await?;

    channel
        .confirm_select(ConfirmSelectOptions::default())
        .await?;
    declare_exchange(&channel, &config.exchange).await?;

    info!(
        exchange = %config.exchange,
        publisher_service = %config.publisher_service,
        worker_id = %config.worker_id,
        "outbox worker started"
    );

    loop {
        if let Err(error) = publish_batch(&db, &channel, &config).await {
            error!(error = %error, "outbox publish batch failed");
        }

        sleep(config.poll_interval).await;
    }
}

pub async fn declare_exchange(channel: &Channel, exchange: &str) -> lapin::Result<()> {
    channel
        .exchange_declare(
            exchange.into(),
            ExchangeKind::Topic,
            ExchangeDeclareOptions {
                durable: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;

    Ok(())
}

pub async fn publish_batch_once(
    db: &DatabaseConnection,
    channel: &Channel,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = Utc::now();
    let expired_claim_before = now - duration_to_chrono(config.claim_ttl);

    let rows = claim_batch(db, config, now, expired_claim_before).await?;

    for row in rows {
        match publish_claimed_row(channel, &row, config).await {
            Ok(()) => {
                mark_published(db, &row).await?;
            }
            Err(error) => {
                warn!(event_id = %row.event_id, error = %error, "publish attempt failed");
                release_claim_for_retry(db, &row, config, &error.to_string()).await?;
            }
        }
    }

    Ok(())
}

async fn publish_batch(
    db: &DatabaseConnection,
    channel: &Channel,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    publish_batch_once(db, channel, config).await
}

async fn claim_batch(
    db: &DatabaseConnection,
    config: &Config,
    now: DateTime<Utc>,
    expired_claim_before: DateTime<Utc>,
) -> Result<Vec<outbox_event::Model>, sea_orm::DbErr> {
    let txn = db.begin().await?;

    let rows = outbox_event::Entity::find()
        .filter(eligible_rows_condition(now, expired_claim_before))
        .order_by_asc(outbox_event::Column::AvailableAt)
        .order_by_asc(outbox_event::Column::CreatedAt)
        .limit(config.batch_size)
        .lock_with_behavior(LockType::Update, LockBehavior::SkipLocked)
        .all(&txn)
        .await?;

    let mut claimed_rows = Vec::with_capacity(rows.len());

    for row in rows {
        let publish_attempts = row.publish_attempts + 1;

        outbox_event::ActiveModel {
            event_id: Set(row.event_id),
            status: Set("claimed".to_string()),
            publish_attempts: Set(publish_attempts),
            claimed_by: Set(Some(config.worker_id.clone())),
            claimed_at: Set(Some(now.into())),
            last_error: Set(None),
            ..Default::default()
        }
        .update(&txn)
        .await?;

        let mut claimed_row = row;
        claimed_row.status = "claimed".to_string();
        claimed_row.publish_attempts = publish_attempts;
        claimed_row.claimed_by = Some(config.worker_id.clone());
        claimed_row.claimed_at = Some(now.into());
        claimed_row.last_error = None;
        claimed_rows.push(claimed_row);
    }

    txn.commit().await?;
    Ok(claimed_rows)
}

fn eligible_rows_condition(
    now: DateTime<Utc>,
    expired_claim_before: DateTime<Utc>,
) -> Condition {
    Condition::any()
        .add(
            Condition::all()
                .add(outbox_event::Column::Status.eq("pending"))
                .add(outbox_event::Column::AvailableAt.lte(now)),
        )
        .add(
            Condition::all()
                .add(outbox_event::Column::Status.eq("claimed"))
                .add(outbox_event::Column::ClaimedAt.lte(expired_claim_before)),
        )
}

async fn publish_claimed_row(
    channel: &Channel,
    row: &outbox_event::Model,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let payload = row.payload.to_string();
    let routing_key = routing_key(&config.publisher_service, &row.event_type);
    let headers = event_headers(row, &config.publisher_service);

    let confirm = channel
        .basic_publish(
            config.exchange.as_str().into(),
            routing_key.as_str().into(),
            BasicPublishOptions::default(),
            payload.as_bytes(),
            BasicProperties::default()
                .with_content_type(ShortString::from("application/json"))
                .with_message_id(ShortString::from(row.event_id.to_string()))
                .with_headers(headers),
        )
        .await?;

    match confirm.await? {
        Confirmation::Ack(_) => Ok(()),
        Confirmation::Nack(_) => Err("broker returned nack for outbox publish".into()),
        Confirmation::NotRequested => Err("publisher confirms were not enabled on the channel".into()),
    }
}

async fn release_claim_for_retry(
    db: &DatabaseConnection,
    row: &outbox_event::Model,
    config: &Config,
    error: &str,
) -> Result<(), sea_orm::DbErr> {
    let now = Utc::now();
    let next_available = now + duration_to_chrono(config.retry_delay);
    let exhausted = row.publish_attempts >= config.max_publish_attempts;

    let mut active = outbox_event::ActiveModel {
        event_id: Set(row.event_id),
        status: Set(if exhausted {
            "failed".to_string()
        } else {
            "pending".to_string()
        }),
        available_at: Set(next_available.into()),
        claimed_by: Set(None),
        claimed_at: Set(None),
        last_error: Set(Some(error.to_string())),
        ..Default::default()
    };

    if exhausted {
        active.published_at = Set(None);
    }

    active.update(db).await?;
    Ok(())
}

pub async fn mark_published(
    db: &DatabaseConnection,
    row: &outbox_event::Model,
) -> Result<(), sea_orm::DbErr> {
    outbox_event::ActiveModel {
        event_id: Set(row.event_id),
        status: Set("published".to_string()),
        published_at: Set(Some(Utc::now().into())),
        claimed_by: Set(None),
        claimed_at: Set(None),
        last_error: Set(None),
        ..Default::default()
    }
    .update(db)
    .await?;

    Ok(())
}

fn routing_key(publisher_service: &str, event_type: &str) -> String {
    format!("{publisher_service}.{event_type}")
}

fn event_headers(row: &outbox_event::Model, publisher_service: &str) -> FieldTable {
    let mut headers = FieldTable::default();
    headers.insert(
        ShortString::from("event_id"),
        AMQPValue::LongString(LongString::from(row.event_id.to_string())),
    );
    headers.insert(
        ShortString::from("event_type"),
        AMQPValue::LongString(LongString::from(row.event_type.clone())),
    );
    headers.insert(
        ShortString::from("aggregate_type"),
        AMQPValue::LongString(LongString::from(row.aggregate_type.clone())),
    );
    headers.insert(
        ShortString::from("aggregate_id"),
        AMQPValue::LongString(LongString::from(row.aggregate_id.to_string())),
    );
    headers.insert(
        ShortString::from("publisher_service"),
        AMQPValue::LongString(LongString::from(publisher_service.to_string())),
    );
    headers.insert(
        ShortString::from("occurred_at"),
        AMQPValue::LongString(LongString::from(row.occurred_at.to_rfc3339())),
    );
    headers
}

fn duration_to_chrono(duration: Duration) -> ChronoDuration {
    ChronoDuration::from_std(duration).expect("duration fits into chrono::Duration")
}

#[cfg(test)]
mod tests {
    use super::{event_headers, routing_key};
    use crate::entity::outbox_event;
    use chrono::Utc;
    use uuid::Uuid;

    fn sample_row() -> outbox_event::Model {
        let now = Utc::now();
        outbox_event::Model {
            event_id: Uuid::new_v4(),
            aggregate_type: "user_account".to_string(),
            aggregate_id: Uuid::new_v4(),
            event_type: "UserRegistered".to_string(),
            payload: serde_json::json!({ "user_id": Uuid::new_v4() }),
            status: "pending".to_string(),
            publish_attempts: 0,
            occurred_at: now.into(),
            available_at: now.into(),
            claimed_by: None,
            claimed_at: None,
            published_at: None,
            last_error: None,
            created_at: now.into(),
        }
    }

    #[test]
    fn routing_key_uses_publisher_service_and_event_type() {
        assert_eq!(routing_key("identity", "UserRegistered"), "identity.UserRegistered");
    }

    #[test]
    fn event_headers_include_dedupe_and_origin_fields() {
        let row = sample_row();
        let headers = event_headers(&row, "identity");

        assert_eq!(headers.inner().len(), 6);
    }
}
