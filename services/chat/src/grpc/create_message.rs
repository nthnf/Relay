use chrono::Utc;
use relay_proto::chat::{CreateMessageRequest, CreateMessageResponse};
use relay_proto::realtime::{
    DeliverMessageRequest, DeliverTargetKind,
    MessageCreatedPayload as RealtimeMessageCreatedPayload, deliver_message_request,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect, Set, TransactionError, TransactionTrait};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::{
    entity::{chat_message, conversation, dm_pair, outbox_event},
    events::{ConversationTargetType as EventConversationTargetType, MessageCreatedPayload},
};

use super::channel_write_auth::{authorize_channel_write, ChannelWriteContext};
use super::handler::Handler;
use relay_types::{actor_user_id, payload_value, to_timestamp};

impl Handler {
    pub(super) async fn create_message(
        &self,
        request: Request<CreateMessageRequest>,
    ) -> Result<Response<CreateMessageResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let CreateMessageRequest {
            conversation_id,
            body,
            client_message_id,
        } = request.into_inner();

        let conversation_id = Uuid::parse_str(&conversation_id)
            .map_err(|_| Status::invalid_argument("Invalid conversation ID"))?;

        let now = Utc::now();
        let mut workspace_client = self.clients.workspace.clone();
        let mut realtime_client = self.clients.realtime.clone();

        let conversation = conversation::Entity::find_by_id(conversation_id)
            .one(&self.connection)
            .await
            .map_err(|e| {
                error!(error = %e, "Conversation lookup failed");
                Status::internal("Internal Server Error")
            })?
            .ok_or_else(|| Status::not_found("Conversation not found"))?;

        let channel_context = if conversation.target_type == "channel" {
            Some(
                authorize_channel_write(
                    &self.connection,
                    &mut workspace_client,
                    actor_user_id,
                    &conversation,
                )
                .await?,
            )
        } else {
            None
        };

        let (response, realtime_message) = self
            .connection
            .transaction::<_, (Response<CreateMessageResponse>, Option<DeliverMessageRequest>), Status>(|txn| {
                Box::pin(async move {
                    let mut target_type = EventConversationTargetType::Dm;
                    let mut workspace_id = None;
                    let mut workspace_channel_id = None;

                    match conversation.target_type.as_str() {
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

                            if actor_user_id != dm_pair.low_user_id && actor_user_id != dm_pair.high_user_id {
                                return Err(Status::permission_denied("Permission denied"));
                            }
                        }
                        "channel" => {
                            let ChannelWriteContext {
                                workspace_id: resolved_workspace_id,
                                workspace_channel_id: resolved_workspace_channel_id,
                            } = channel_context
                                .ok_or_else(|| Status::not_found("Conversation not found"))?;

                            target_type = EventConversationTargetType::WorkspaceChannel;
                            workspace_id = Some(resolved_workspace_id);
                            workspace_channel_id = Some(resolved_workspace_channel_id);
                        }
                        _ => return Err(Status::internal("Internal Server Error")),
                    }

                    if let Some(client_message_id) = client_message_id.as_ref()
                        && let Some(existing) = chat_message::Entity::find()
                            .filter(chat_message::Column::AuthorUserId.eq(actor_user_id))
                            .filter(chat_message::Column::ConversationId.eq(conversation_id))
                            .filter(chat_message::Column::ClientMessageId.eq(client_message_id))
                            .one(txn)
                            .await
                            .map_err(|e| {
                                error!(error = %e, "Idempotent message lookup failed");
                                Status::internal("Internal Server Error")
                            })?
                    {
                        return Ok((
                            Response::new(CreateMessageResponse {
                                message_id: existing.message_id.to_string(),
                                conversation_id: existing.conversation_id.to_string(),
                                author_user_id: existing.author_user_id.to_string(),
                                conversation_message_seq: existing.conversation_message_seq,
                                body: existing.body,
                                created_at: Some(to_timestamp(existing.created_at.with_timezone(&Utc))),
                            }),
                            None,
                        ));
                    }

                    let next_conversation_message_seq = chat_message::Entity::find()
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
                        .unwrap_or(0)
                        + 1;

                    let message_id = Uuid::new_v4();
                    chat_message::Entity::insert(chat_message::ActiveModel {
                        message_id: Set(message_id),
                        conversation_id: Set(conversation_id),
                        author_user_id: Set(actor_user_id),
                        client_message_id: Set(client_message_id.clone()),
                        conversation_message_seq: Set(next_conversation_message_seq),
                        body: Set(body.clone()),
                        message_status: Set("active".to_string()),
                        created_at: Set(now.into()),
                        updated_at: Set(now.into()),
                        deleted_at: Set(None),
                        deleted_by_user_id: Set(None),
                        last_edited_at: Set(None),
                        last_edited_by_user_id: Set(None),
                    })
                    .exec(txn)
                    .await
                    .map_err(|e| {
                        error!(error = %e, "Chat message insert failed");
                        Status::internal("Internal Server Error")
                    })?;

                    let event_id = Uuid::new_v4();
                    let payload = payload_value(MessageCreatedPayload {
                        delivery_id: event_id.to_string(),
                        message_id: message_id.to_string(),
                        conversation_id: conversation_id.to_string(),
                        target_type,
                        workspace_id: workspace_id.map(|id| id.to_string()),
                        workspace_channel_id: workspace_channel_id.map(|id| id.to_string()),
                        author_user_id: actor_user_id.to_string(),
                        conversation_message_seq: next_conversation_message_seq,
                        body: body.clone(),
                        created_at: now.to_rfc3339(),
                    })?;

                    outbox_event::Entity::insert(outbox_event::ActiveModel {
                        event_id: Set(event_id),
                        aggregate_type: Set("chat_message".to_string()),
                        aggregate_id: Set(message_id),
                        event_type: Set("MessageCreated".to_string()),
                        payload: Set(payload),
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
                        error!(error = %e, "Chat message outbox insert failed");
                        Status::internal("Internal Server Error")
                    })?;

                    Ok((
                        Response::new(CreateMessageResponse {
                            message_id: message_id.to_string(),
                            conversation_id: conversation_id.to_string(),
                            author_user_id: actor_user_id.to_string(),
                            conversation_message_seq: next_conversation_message_seq,
                            body: body.clone(),
                            created_at: Some(to_timestamp(now)),
                        }),
                        Some(DeliverMessageRequest {
                            delivery_id: event_id.to_string(),
                            target_kind: if workspace_channel_id.is_some() {
                                DeliverTargetKind::WorkspaceChannel as i32
                            } else {
                                DeliverTargetKind::DirectMessage as i32
                            },
                            target_id: if let Some(workspace_channel_id) = workspace_channel_id {
                                workspace_channel_id.to_string()
                            } else {
                                conversation_id.to_string()
                            },
                            occurred_at: Some(to_timestamp(now)),
                            payload: Some(deliver_message_request::Payload::MessageCreated(
                                RealtimeMessageCreatedPayload {
                                    message_id: message_id.to_string(),
                                    author_user_id: actor_user_id.to_string(),
                                    body: body.clone(),
                                    target_message_seq: next_conversation_message_seq,
                                    created_at: Some(to_timestamp(now)),
                                },
                            )),
                        }),
                    ))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "Create message transaction connection failure");
                    Status::internal("Internal Server Error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        if let Some(realtime_message) = realtime_message
            && let Err(error) = realtime_client
                .deliver_message(Request::new(realtime_message))
                .await
        {
            error!(error = %error, "Realtime deliver_message failed");
        }

        Ok(response)
    }
}
