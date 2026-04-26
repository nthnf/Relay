use chrono::{DateTime, FixedOffset, Utc};
use sea_orm::{
    ActiveModelTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, IntoActiveModel, Set,
    TransactionError,
};
use std::sync::Arc;
use uuid::Uuid;

use relay_amqp::{DeliveryContext, EventHandleError, EventHandleResult};

use crate::entity::{compose_queue, processed_event};

#[derive(Clone)]
pub struct Handler {
    pub(crate) db: DatabaseConnection,
}

impl Handler {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub(crate) fn parse_uuid(field: &str, value: &str) -> Result<Uuid, EventHandleError> {
        Uuid::parse_str(value)
            .map_err(|error| EventHandleError::Permanent(format!("invalid {field}: {error}")))
    }

    pub(crate) fn parse_timestamp(
        field: &str,
        value: &str,
    ) -> Result<DateTime<FixedOffset>, EventHandleError> {
        DateTime::parse_from_rfc3339(value)
            .map_err(|error| EventHandleError::Permanent(format!("invalid {field}: {error}")))
    }

    pub(crate) fn db_error(operation: &str, error: sea_orm::DbErr) -> EventHandleError {
        EventHandleError::Transient(format!("{operation} failed: {error}"))
    }

    pub(crate) fn tx_error(
        operation: &str,
        error: TransactionError<EventHandleError>,
    ) -> EventHandleError {
        match error {
            TransactionError::Connection(error) => Self::db_error(operation, error),
            TransactionError::Transaction(error) => error,
        }
    }

    pub(crate) async fn mark_event_processed(
        txn: &DatabaseTransaction,
        delivery: &DeliveryContext,
        source_id: &str,
    ) -> Result<bool, EventHandleError> {
        let event_id = processed_event_id(delivery, source_id);

        if processed_event::Entity::find_by_id(&event_id)
            .one(txn)
            .await
            .map_err(|error| Self::db_error("processed event lookup", error))?
            .is_some()
        {
            return Ok(false);
        }

        processed_event::ActiveModel {
            event_id: Set(event_id),
            routing_key: Set(delivery.routing_key.clone()),
            processed_at: Set(Utc::now().into()),
        }
        .insert(txn)
        .await
        .map_err(|error| Self::db_error("processed event insert", error))?;

        Ok(true)
    }

    pub(crate) async fn enqueue_compose(
        txn: &DatabaseTransaction,
        work: ComposeWork,
    ) -> Result<(), EventHandleError> {
        let now = Utc::now().into();

        match compose_queue::Entity::find_by_id(&work.compose_key)
            .one(txn)
            .await
            .map_err(|error| Self::db_error("compose queue lookup", error))?
        {
            Some(existing) => {
                let mut active = existing.into_active_model();
                active.compose_kind = Set(work.compose_kind);
                active.user_id = Set(work.user_id);
                active.workspace_id = Set(work.workspace_id);
                active.channel_id = Set(work.channel_id);
                active.conversation_id = Set(work.conversation_id);
                active.dm_pair_id = Set(work.dm_pair_id);
                active.status = Set("claimed".to_string());
                active.available_at = Set(now);
                active.claimed_at = Set(Some(now));
                active.last_error = Set(None);
                active.updated_at = Set(now);
                active
                    .update(txn)
                    .await
                    .map_err(|error| Self::db_error("compose queue update", error))?;
            }
            None => {
                compose_queue::ActiveModel {
                    compose_key: Set(work.compose_key),
                    compose_kind: Set(work.compose_kind),
                    user_id: Set(work.user_id),
                    workspace_id: Set(work.workspace_id),
                    channel_id: Set(work.channel_id),
                    conversation_id: Set(work.conversation_id),
                    dm_pair_id: Set(work.dm_pair_id),
                    status: Set("claimed".to_string()),
                    attempts: Set(0),
                    available_at: Set(now),
                    claimed_at: Set(Some(now)),
                    last_error: Set(None),
                    updated_at: Set(now),
                }
                .insert(txn)
                .await
                .map_err(|error| Self::db_error("compose queue insert", error))?;
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn ignore_event<T>(
        self: Arc<Self>,
        _delivery: DeliveryContext,
        _payload: T,
    ) -> EventHandleResult {
        Ok(())
    }
}

pub(crate) struct ComposeWork {
    pub(crate) compose_key: String,
    pub(crate) compose_kind: String,
    pub(crate) user_id: Option<Uuid>,
    pub(crate) workspace_id: Option<Uuid>,
    pub(crate) channel_id: Option<Uuid>,
    pub(crate) conversation_id: Option<Uuid>,
    pub(crate) dm_pair_id: Option<Uuid>,
}

impl ComposeWork {
    pub(crate) fn user_app(user_id: Uuid) -> Self {
        Self {
            compose_key: format!("user_app:{user_id}"),
            compose_kind: "user_app".to_string(),
            user_id: Some(user_id),
            workspace_id: None,
            channel_id: None,
            conversation_id: None,
            dm_pair_id: None,
        }
    }

    pub(crate) fn workspace(user_id: Option<Uuid>, workspace_id: Uuid) -> Self {
        Self {
            compose_key: user_id.map_or_else(
                || format!("workspace:{workspace_id}"),
                |user_id| format!("workspace:{user_id}:{workspace_id}"),
            ),
            compose_kind: "workspace".to_string(),
            user_id,
            workspace_id: Some(workspace_id),
            channel_id: None,
            conversation_id: None,
            dm_pair_id: None,
        }
    }

    pub(crate) fn workspace_channel(
        user_id: Option<Uuid>,
        workspace_id: Option<Uuid>,
        channel_id: Option<Uuid>,
        conversation_id: Option<Uuid>,
    ) -> Self {
        Self {
            compose_key: format!(
                "workspace_channel:{}:{}:{}:{}",
                user_id.map(|id| id.to_string()).unwrap_or_default(),
                workspace_id.map(|id| id.to_string()).unwrap_or_default(),
                channel_id.map(|id| id.to_string()).unwrap_or_default(),
                conversation_id.map(|id| id.to_string()).unwrap_or_default()
            ),
            compose_kind: "workspace_channel".to_string(),
            user_id,
            workspace_id,
            channel_id,
            conversation_id,
            dm_pair_id: None,
        }
    }

    pub(crate) fn dm(
        user_id: Option<Uuid>,
        dm_pair_id: Option<Uuid>,
        conversation_id: Option<Uuid>,
    ) -> Self {
        Self {
            compose_key: format!(
                "dm:{}:{}:{}",
                user_id.map(|id| id.to_string()).unwrap_or_default(),
                dm_pair_id.map(|id| id.to_string()).unwrap_or_default(),
                conversation_id.map(|id| id.to_string()).unwrap_or_default()
            ),
            compose_kind: "dm".to_string(),
            user_id,
            workspace_id: None,
            channel_id: None,
            conversation_id,
            dm_pair_id,
        }
    }

    pub(crate) fn workspace_unread(
        user_id: Option<Uuid>,
        workspace_id: Option<Uuid>,
        conversation_id: Option<Uuid>,
    ) -> Self {
        Self {
            compose_key: format!(
                "workspace_unread:{}:{}:{}",
                user_id.map(|id| id.to_string()).unwrap_or_default(),
                workspace_id.map(|id| id.to_string()).unwrap_or_default(),
                conversation_id.map(|id| id.to_string()).unwrap_or_default()
            ),
            compose_kind: "workspace_unread".to_string(),
            user_id,
            workspace_id,
            channel_id: None,
            conversation_id,
            dm_pair_id: None,
        }
    }
}

fn processed_event_id(delivery: &DeliveryContext, source_id: &str) -> String {
    delivery
        .message_id
        .clone()
        .unwrap_or_else(|| format!("{}:{}", delivery.routing_key, source_id))
}
