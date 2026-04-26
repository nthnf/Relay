use std::sync::Arc;

use crate::{
    entity::{
        conversation_message_state, conversation_read_state, conversation_snapshot,
        dm_pair_snapshot,
    },
    events::{
        ConversationCreatedPayload, ConversationReadCursorUpdatedPayload, ConversationTargetType,
        DmPairCreatedPayload, MessageCreatedPayload, MessageDeletedPayload, MessageEditedPayload,
    },
};
use relay_amqp::{DeliveryContext, EventHandleResult, RegisteredSubscriber, route};
use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, Set, TransactionTrait};

use super::handler::{ComposeWork, Handler};

impl Handler {
    pub async fn handle_conversation_created(
        self: Arc<Self>,
        delivery: DeliveryContext,
        payload: ConversationCreatedPayload,
    ) -> EventHandleResult {
        let ConversationCreatedPayload {
            conversation_id,
            target_type,
            dm_pair_id,
            workspace_channel_id,
            created_at,
        } = payload;

        let parsed_conversation_id = Self::parse_uuid("conversation_id", &conversation_id)?;

        let parsed_dm_pair_id = dm_pair_id
            .as_deref()
            .map(|value| Self::parse_uuid("dm_pair_id", value))
            .transpose()?;

        let parsed_workspace_channel_id = workspace_channel_id
            .as_deref()
            .map(|value| Self::parse_uuid("workspace_channel_id", value))
            .transpose()?;

        let created_at = Self::parse_timestamp("created_at", &created_at)?;
        let target_type_name = target_type_name(target_type).to_string();

        match target_type {
            ConversationTargetType::WorkspaceChannel if parsed_workspace_channel_id.is_none() => {
                return Err(relay_amqp::EventHandleError::Permanent(
                    "workspace channel conversation missing workspace_channel_id".to_string(),
                ));
            }
            ConversationTargetType::Dm if parsed_dm_pair_id.is_none() => {
                return Err(relay_amqp::EventHandleError::Permanent(
                    "dm conversation missing dm_pair_id".to_string(),
                ));
            }
            _ => {}
        }

        self.db
            .transaction::<_, (), relay_amqp::EventHandleError>(|txn| {
                Box::pin(async move {
                    if !Self::mark_event_processed(txn, &delivery, &conversation_id).await? {
                        return Ok(());
                    }

                    match conversation_snapshot::Entity::find_by_id(parsed_conversation_id)
                        .one(txn)
                        .await
                        .map_err(|error| Self::db_error("conversation snapshot lookup", error))?
                    {
                        Some(existing) => {
                            let mut active = existing.into_active_model();
                            active.target_type = Set(target_type_name);
                            active.dm_pair_id = Set(parsed_dm_pair_id);
                            active.workspace_channel_id = Set(parsed_workspace_channel_id);
                            active.created_at = Set(created_at);
                            active.updated_at = Set(created_at);
                            active.update(txn).await.map_err(|error| {
                                Self::db_error("conversation snapshot update", error)
                            })?;
                        }
                        None => {
                            conversation_snapshot::ActiveModel {
                                conversation_id: Set(parsed_conversation_id),
                                target_type: Set(target_type_name),
                                dm_pair_id: Set(parsed_dm_pair_id),
                                workspace_channel_id: Set(parsed_workspace_channel_id),
                                created_at: Set(created_at),
                                updated_at: Set(created_at),
                            }
                            .insert(txn)
                            .await
                            .map_err(|error| {
                                Self::db_error("conversation snapshot insert", error)
                            })?;
                        }
                    }

                    match target_type {
                        ConversationTargetType::WorkspaceChannel => {
                            Self::enqueue_compose(
                                txn,
                                ComposeWork::workspace_channel(
                                    None,
                                    None,
                                    parsed_workspace_channel_id,
                                    Some(parsed_conversation_id),
                                ),
                            )
                            .await?;
                            Self::enqueue_compose(
                                txn,
                                ComposeWork::workspace_unread(
                                    None,
                                    None,
                                    Some(parsed_conversation_id),
                                ),
                            )
                            .await?;
                        }
                        ConversationTargetType::Dm => {
                            Self::enqueue_compose(
                                txn,
                                ComposeWork::dm(
                                    None,
                                    parsed_dm_pair_id,
                                    Some(parsed_conversation_id),
                                ),
                            )
                            .await?;
                        }
                    }

                    Ok(())
                })
            })
            .await
            .map_err(|error| Self::tx_error("conversation created transaction", error))?;

        Ok(())
    }

    pub async fn handle_dm_pair_created(
        self: Arc<Self>,
        delivery: DeliveryContext,
        payload: DmPairCreatedPayload,
    ) -> EventHandleResult {
        let DmPairCreatedPayload {
            dm_pair_id,
            low_user_id,
            high_user_id,
            created_at,
        } = payload;

        let parsed_dm_pair_id = Self::parse_uuid("dm_pair_id", &dm_pair_id)?;
        let low_user_id = Self::parse_uuid("low_user_id", &low_user_id)?;
        let high_user_id = Self::parse_uuid("high_user_id", &high_user_id)?;
        let created_at = Self::parse_timestamp("created_at", &created_at)?;

        self.db
            .transaction::<_, (), relay_amqp::EventHandleError>(|txn| {
                Box::pin(async move {
                    if !Self::mark_event_processed(txn, &delivery, &dm_pair_id).await? {
                        return Ok(());
                    }

                    match dm_pair_snapshot::Entity::find_by_id(parsed_dm_pair_id)
                        .one(txn)
                        .await
                        .map_err(|error| Self::db_error("dm pair snapshot lookup", error))?
                    {
                        Some(existing) => {
                            let mut active = existing.into_active_model();
                            active.low_user_id = Set(low_user_id);
                            active.high_user_id = Set(high_user_id);
                            active.created_at = Set(created_at);
                            active.updated_at = Set(created_at);
                            active.update(txn).await.map_err(|error| {
                                Self::db_error("dm pair snapshot update", error)
                            })?;
                        }
                        None => {
                            dm_pair_snapshot::ActiveModel {
                                dm_pair_id: Set(parsed_dm_pair_id),
                                low_user_id: Set(low_user_id),
                                high_user_id: Set(high_user_id),
                                created_at: Set(created_at),
                                updated_at: Set(created_at),
                            }
                            .insert(txn)
                            .await
                            .map_err(|error| Self::db_error("dm pair snapshot insert", error))?;
                        }
                    }

                    Self::enqueue_compose(
                        txn,
                        ComposeWork::dm(Some(low_user_id), Some(parsed_dm_pair_id), None),
                    )
                    .await?;
                    Self::enqueue_compose(
                        txn,
                        ComposeWork::dm(Some(high_user_id), Some(parsed_dm_pair_id), None),
                    )
                    .await?;

                    Ok(())
                })
            })
            .await
            .map_err(|error| Self::tx_error("dm pair created transaction", error))?;

        Ok(())
    }

    pub async fn handle_message_created(
        self: Arc<Self>,
        delivery: DeliveryContext,
        payload: MessageCreatedPayload,
    ) -> EventHandleResult {
        let MessageCreatedPayload {
            delivery_id: _,
            message_id,
            conversation_id,
            target_type,
            workspace_id,
            workspace_channel_id,
            author_user_id,
            conversation_message_seq,
            body,
            created_at,
        } = payload;

        let parsed_message_id = Self::parse_uuid("message_id", &message_id)?;
        let parsed_conversation_id = Self::parse_uuid("conversation_id", &conversation_id)?;
        let author_user_id = Self::parse_uuid("author_user_id", &author_user_id)?;
        let parsed_workspace_id = workspace_id
            .as_deref()
            .map(|value| Self::parse_uuid("workspace_id", value))
            .transpose()?;
        let parsed_workspace_channel_id = workspace_channel_id
            .as_deref()
            .map(|value| Self::parse_uuid("workspace_channel_id", value))
            .transpose()?;
        let created_at = Self::parse_timestamp("created_at", &created_at)?;

        self.db
            .transaction::<_, (), relay_amqp::EventHandleError>(|txn| {
                Box::pin(async move {
                    if !Self::mark_event_processed(txn, &delivery, &message_id).await? {
                        return Ok(());
                    }

                    match conversation_message_state::Entity::find_by_id(parsed_conversation_id)
                        .one(txn)
                        .await
                        .map_err(|error| {
                            Self::db_error("conversation message state lookup", error)
                        })? {
                        Some(existing)
                            if existing
                                .last_message_seq
                                .is_some_and(|seq| seq > conversation_message_seq) => {}
                        Some(existing) => {
                            let mut active = existing.into_active_model();
                            active.last_message_id = Set(Some(parsed_message_id));
                            active.last_message_author_user_id = Set(Some(author_user_id));
                            active.last_message_seq = Set(Some(conversation_message_seq));
                            active.last_message_preview = Set(Some(body));
                            active.last_activity_at = Set(Some(created_at));
                            active.updated_at = Set(created_at);
                            active.update(txn).await.map_err(|error| {
                                Self::db_error("conversation message state update", error)
                            })?;
                        }
                        None => {
                            conversation_message_state::ActiveModel {
                                conversation_id: Set(parsed_conversation_id),
                                last_message_id: Set(Some(parsed_message_id)),
                                last_message_author_user_id: Set(Some(author_user_id)),
                                last_message_seq: Set(Some(conversation_message_seq)),
                                last_message_preview: Set(Some(body)),
                                last_activity_at: Set(Some(created_at)),
                                updated_at: Set(created_at),
                            }
                            .insert(txn)
                            .await
                            .map_err(|error| {
                                Self::db_error("conversation message state insert", error)
                            })?;
                        }
                    }

                    enqueue_for_target(
                        txn,
                        target_type,
                        parsed_workspace_id,
                        parsed_workspace_channel_id,
                        None,
                        Some(parsed_conversation_id),
                    )
                    .await?;

                    Ok(())
                })
            })
            .await
            .map_err(|error| Self::tx_error("message created transaction", error))?;

        Ok(())
    }

    pub async fn handle_message_edited(
        self: Arc<Self>,
        delivery: DeliveryContext,
        payload: MessageEditedPayload,
    ) -> EventHandleResult {
        let MessageEditedPayload {
            delivery_id: _,
            message_id,
            conversation_id,
            target_type,
            workspace_id,
            workspace_channel_id,
            editor_user_id: _,
            body,
            edited_at,
        } = payload;

        let parsed_message_id = Self::parse_uuid("message_id", &message_id)?;
        let parsed_conversation_id = Self::parse_uuid("conversation_id", &conversation_id)?;
        let parsed_workspace_id = workspace_id
            .as_deref()
            .map(|value| Self::parse_uuid("workspace_id", value))
            .transpose()?;
        let parsed_workspace_channel_id = workspace_channel_id
            .as_deref()
            .map(|value| Self::parse_uuid("workspace_channel_id", value))
            .transpose()?;
        let edited_at = Self::parse_timestamp("edited_at", &edited_at)?;

        self.db
            .transaction::<_, (), relay_amqp::EventHandleError>(|txn| {
                Box::pin(async move {
                    if !Self::mark_event_processed(txn, &delivery, &message_id).await? {
                        return Ok(());
                    }

                    if let Some(existing) =
                        conversation_message_state::Entity::find_by_id(parsed_conversation_id)
                            .one(txn)
                            .await
                            .map_err(|error| {
                                Self::db_error("conversation message state lookup", error)
                            })?
                            .filter(|state| state.last_message_id == Some(parsed_message_id))
                    {
                        let mut active = existing.into_active_model();
                        active.last_message_preview = Set(Some(body));
                        active.updated_at = Set(edited_at);
                        active.update(txn).await.map_err(|error| {
                            Self::db_error("conversation message state update", error)
                        })?;

                        enqueue_for_target(
                            txn,
                            target_type,
                            parsed_workspace_id,
                            parsed_workspace_channel_id,
                            None,
                            Some(parsed_conversation_id),
                        )
                        .await?;
                    }

                    Ok(())
                })
            })
            .await
            .map_err(|error| Self::tx_error("message edited transaction", error))?;

        Ok(())
    }

    pub async fn handle_message_deleted(
        self: Arc<Self>,
        delivery: DeliveryContext,
        payload: MessageDeletedPayload,
    ) -> EventHandleResult {
        let MessageDeletedPayload {
            delivery_id: _,
            message_id,
            conversation_id,
            target_type,
            workspace_id,
            workspace_channel_id,
            deleted_by_user_id: _,
            deleted_at,
        } = payload;
        let parsed_message_id = Self::parse_uuid("message_id", &message_id)?;
        let parsed_conversation_id = Self::parse_uuid("conversation_id", &conversation_id)?;
        let parsed_workspace_id = workspace_id
            .as_deref()
            .map(|value| Self::parse_uuid("workspace_id", value))
            .transpose()?;
        let parsed_workspace_channel_id = workspace_channel_id
            .as_deref()
            .map(|value| Self::parse_uuid("workspace_channel_id", value))
            .transpose()?;
        let deleted_at = Self::parse_timestamp("deleted_at", &deleted_at)?;

        self.db
            .transaction::<_, (), relay_amqp::EventHandleError>(|txn| {
                Box::pin(async move {
                    if !Self::mark_event_processed(txn, &delivery, &message_id).await? {
                        return Ok(());
                    }

                    if let Some(existing) =
                        conversation_message_state::Entity::find_by_id(parsed_conversation_id)
                            .one(txn)
                            .await
                            .map_err(|error| {
                                Self::db_error("conversation message state lookup", error)
                            })?
                            .filter(|state| state.last_message_id == Some(parsed_message_id))
                    {
                        let mut active = existing.into_active_model();
                        active.last_message_preview = Set(Some("Message deleted".to_string()));
                        active.updated_at = Set(deleted_at);
                        active.update(txn).await.map_err(|error| {
                            Self::db_error("conversation message state update", error)
                        })?;

                        enqueue_for_target(
                            txn,
                            target_type,
                            parsed_workspace_id,
                            parsed_workspace_channel_id,
                            None,
                            Some(parsed_conversation_id),
                        )
                        .await?;
                    }

                    Ok(())
                })
            })
            .await
            .map_err(|error| Self::tx_error("message deleted transaction", error))?;

        Ok(())
    }

    pub async fn handle_conversation_read_cursor_updated(
        self: Arc<Self>,
        delivery: DeliveryContext,
        payload: ConversationReadCursorUpdatedPayload,
    ) -> EventHandleResult {
        let ConversationReadCursorUpdatedPayload {
            conversation_id,
            target_type,
            workspace_channel_id,
            user_id,
            last_read_conversation_message_seq,
            read_at,
        } = payload;

        let parsed_conversation_id = Self::parse_uuid("conversation_id", &conversation_id)?;
        let parsed_user_id = Self::parse_uuid("user_id", &user_id)?;
        let parsed_workspace_channel_id = workspace_channel_id
            .as_deref()
            .map(|value| Self::parse_uuid("workspace_channel_id", value))
            .transpose()?;
        let read_at = Self::parse_timestamp("read_at", &read_at)?;

        self.db
            .transaction::<_, (), relay_amqp::EventHandleError>(|txn| {
                Box::pin(async move {
                    let source_id = format!("{}:{}", conversation_id, user_id);
                    if !Self::mark_event_processed(txn, &delivery, &source_id).await? {
                        return Ok(());
                    }

                    match conversation_read_state::Entity::find_by_id((
                        parsed_conversation_id,
                        parsed_user_id,
                    ))
                    .one(txn)
                    .await
                    .map_err(|error| Self::db_error("conversation read state lookup", error))?
                    {
                        Some(existing) => {
                            let mut active = existing.into_active_model();
                            active.last_read_conversation_message_seq =
                                Set(last_read_conversation_message_seq);
                            active.read_at = Set(read_at);
                            active.updated_at = Set(read_at);
                            active.update(txn).await.map_err(|error| {
                                Self::db_error("conversation read state update", error)
                            })?;
                        }
                        None => {
                            conversation_read_state::ActiveModel {
                                conversation_id: Set(parsed_conversation_id),
                                user_id: Set(parsed_user_id),
                                last_read_conversation_message_seq: Set(
                                    last_read_conversation_message_seq,
                                ),
                                read_at: Set(read_at),
                                updated_at: Set(read_at),
                            }
                            .insert(txn)
                            .await
                            .map_err(|error| {
                                Self::db_error("conversation read state insert", error)
                            })?;
                        }
                    }

                    enqueue_for_target(
                        txn,
                        target_type,
                        None,
                        parsed_workspace_channel_id,
                        Some(parsed_user_id),
                        Some(parsed_conversation_id),
                    )
                    .await?;

                    Ok(())
                })
            })
            .await
            .map_err(|error| Self::tx_error("read cursor transaction", error))?;

        Ok(())
    }
}

async fn enqueue_for_target(
    txn: &sea_orm::DatabaseTransaction,
    target_type: ConversationTargetType,
    workspace_id: Option<uuid::Uuid>,
    workspace_channel_id: Option<uuid::Uuid>,
    user_id: Option<uuid::Uuid>,
    conversation_id: Option<uuid::Uuid>,
) -> EventHandleResult {
    match target_type {
        ConversationTargetType::WorkspaceChannel => {
            Handler::enqueue_compose(
                txn,
                ComposeWork::workspace_channel(
                    user_id,
                    workspace_id,
                    workspace_channel_id,
                    conversation_id,
                ),
            )
            .await?;
            Handler::enqueue_compose(
                txn,
                ComposeWork::workspace_unread(user_id, workspace_id, conversation_id),
            )
            .await?;
        }
        ConversationTargetType::Dm => {
            Handler::enqueue_compose(txn, ComposeWork::dm(user_id, None, conversation_id)).await?;
        }
    }

    Ok(())
}

fn target_type_name(target_type: ConversationTargetType) -> &'static str {
    match target_type {
        ConversationTargetType::Dm => "dm",
        ConversationTargetType::WorkspaceChannel => "workspace_channel",
    }
}

pub(super) fn register(subscriber: RegisteredSubscriber<Handler>) -> RegisteredSubscriber<Handler> {
    subscriber
        .event(
            "chat.ConversationCreated",
            route(Handler::handle_conversation_created),
        )
        .event("chat.DmPairCreated", route(Handler::handle_dm_pair_created))
        .event(
            "chat.MessageCreated",
            route(Handler::handle_message_created),
        )
        .event("chat.MessageEdited", route(Handler::handle_message_edited))
        .event(
            "chat.MessageDeleted",
            route(Handler::handle_message_deleted),
        )
        .event(
            "chat.ConversationReadCursorUpdated",
            route(Handler::handle_conversation_read_cursor_updated),
        )
}
