use chrono::Utc;
use relay_proto::chat::{MarkConversationReadRequest, MarkConversationReadResponse};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QuerySelect, Set,
    TransactionError, TransactionTrait,
};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::{
    entity::{chat_message, conversation, conversation_read_cursor, dm_pair, outbox_event},
    events::{
        ConversationReadCursorUpdatedPayload, ConversationTargetType as EventConversationTargetType,
    },
};

use super::{channel_write_auth::authorize_channel_read, handler::Handler};
use relay_types::{actor_user_id, payload_value, to_timestamp};

impl Handler {
    pub(super) async fn mark_conversation_read(
        &self,
        request: Request<MarkConversationReadRequest>,
    ) -> Result<Response<MarkConversationReadResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let MarkConversationReadRequest {
            conversation_id,
            last_read_conversation_message_seq,
        } = request.into_inner();

        if last_read_conversation_message_seq < 1 {
            return Err(Status::invalid_argument(
                "Last read conversation message seq must be positive",
            ));
        }

        let conversation_id = Uuid::parse_str(&conversation_id)
            .map_err(|_| Status::invalid_argument("Invalid conversation ID"))?;

        let mut workspace_client = self.clients.workspace.clone();

        let response = self
            .connection
            .transaction::<_, Response<MarkConversationReadResponse>, Status>(|txn| {
                Box::pin(async move {
                    let conversation = conversation::Entity::find_by_id(conversation_id)
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Conversation lookup failed");
                            Status::internal("Internal Server Error")
                        })?
                        .ok_or_else(|| Status::not_found("Conversation not found"))?;

                    let (target_type, workspace_channel_id) = match conversation.target_type.as_str() {
                        "dm" => {
                            let dm_pair_id = conversation
                                .dm_pair_id
                                .ok_or_else(|| Status::not_found("Conversation not found"))?;

                            let dm_pair = dm_pair::Entity::find_by_id(dm_pair_id)
                                .one(txn)
                                .await
                                .map_err(|e| {
                                    error!(error = %e, "DM pair lookup failed");
                                    Status::internal("Internal Server Error")
                                })?
                                .ok_or_else(|| Status::not_found("Conversation not found"))?;

                            if actor_user_id != dm_pair.low_user_id
                                && actor_user_id != dm_pair.high_user_id
                            {
                                return Err(Status::permission_denied("Permission denied"));
                            }

                            (EventConversationTargetType::Dm, None)
                        }
                        "channel" => {
                            let context = authorize_channel_read(
                                txn,
                                &mut workspace_client,
                                actor_user_id,
                                &conversation,
                            )
                            .await?;
                            (
                                EventConversationTargetType::WorkspaceChannel,
                                Some(context.workspace_channel_id),
                            )
                        }
                        _ => return Err(Status::internal("Internal Server Error")),
                    };

                    let max_conversation_message_seq = chat_message::Entity::find()
                        .filter(chat_message::Column::ConversationId.eq(conversation_id))
                        .select_only()
                        .column_as(chat_message::Column::ConversationMessageSeq.max(), "max_seq")
                        .into_tuple::<Option<i64>>()
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Conversation message seq lookup failed");
                            Status::internal("Internal Server Error")
                        })?
                        .flatten()
                        .unwrap_or(0);

                    if max_conversation_message_seq == 0 {
                        return Err(Status::failed_precondition("Conversation has no messages"));
                    }

                    if last_read_conversation_message_seq > max_conversation_message_seq {
                        return Err(Status::invalid_argument(
                            "Read cursor exceeds latest conversation message seq",
                        ));
                    }

                    let existing = conversation_read_cursor::Entity::find_by_id((
                        actor_user_id,
                        conversation_id,
                    ))
                    .one(txn)
                    .await
                    .map_err(|e| {
                        error!(error = %e, "Conversation read cursor lookup failed");
                        Status::internal("Internal Server Error")
                    })?;

                    if let Some(existing) = existing.as_ref() && existing.last_read_conversation_message_seq
                            >= last_read_conversation_message_seq
                        {
                            return Ok(Response::new(MarkConversationReadResponse {
                                conversation_id: conversation_id.to_string(),
                                last_read_conversation_message_seq: existing
                                    .last_read_conversation_message_seq,
                                read_at: Some(to_timestamp(existing.read_at.with_timezone(&Utc))),
                                updated: false,
                            }));
                    }

                    let now = Utc::now();

                    match existing {
                        Some(existing) => {
                            let mut active = existing.into_active_model();
                            active.last_read_conversation_message_seq =
                                Set(last_read_conversation_message_seq);
                            active.read_at = Set(now.into());
                            active.updated_at = Set(now.into());
                            active.update(txn).await.map_err(|e| {
                                error!(error = %e, "Conversation read cursor update failed");
                                Status::internal("Internal Server Error")
                            })?;
                        }
                        None => {
                            conversation_read_cursor::Entity::insert(
                                conversation_read_cursor::ActiveModel {
                                    user_id: Set(actor_user_id),
                                    conversation_id: Set(conversation_id),
                                    last_read_conversation_message_seq: Set(
                                        last_read_conversation_message_seq,
                                    ),
                                    read_at: Set(now.into()),
                                    updated_at: Set(now.into()),
                                },
                            )
                            .exec(txn)
                            .await
                            .map_err(|e| {
                                error!(error = %e, "Conversation read cursor insert failed");
                                Status::internal("Internal Server Error")
                            })?;
                        }
                    }

                    let event_id = Uuid::new_v4();
                    outbox_event::Entity::insert(outbox_event::ActiveModel {
                        event_id: Set(event_id),
                        aggregate_type: Set("conversation_read_cursor".to_string()),
                        aggregate_id: Set(conversation_id),
                        event_type: Set("ConversationReadCursorUpdated".to_string()),
                        payload: Set(payload_value(ConversationReadCursorUpdatedPayload {
                            conversation_id: conversation_id.to_string(),
                            target_type,
                            workspace_channel_id: workspace_channel_id.map(|id| id.to_string()),
                            user_id: actor_user_id.to_string(),
                            last_read_conversation_message_seq,
                            read_at: now.to_rfc3339(),
                        })?),
                        status: Set("pending".to_string()),
                        publish_attempts: Set(0),
                        occurred_at: Set(now.into()),
                        available_at: Set(now.into()),
                        claimed_by: Set(None),
                        claimed_at: Set(None),
                        published_at: Set(None),
                        last_error: Set(None),
                        created_at: Set(now.into()),
                    })
                    .exec(txn)
                    .await
                    .map_err(|e| {
                        error!(error = %e, "Conversation read cursor outbox insert failed");
                        Status::internal("Internal Server Error")
                    })?;

                    Ok(Response::new(MarkConversationReadResponse {
                        conversation_id: conversation_id.to_string(),
                        last_read_conversation_message_seq,
                        read_at: Some(to_timestamp(now)),
                        updated: true,
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "Mark conversation read transaction connection failure");
                    Status::internal("Internal Server Error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}
