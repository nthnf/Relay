use chrono::Utc;
use relay_proto::chat::{DeleteMessageRequest, DeleteMessageResponse};
use relay_proto::realtime::{
    DeliverMessageRequest, DeliverTargetKind,
    MessageDeletedPayload as RealtimeMessageDeletedPayload, deliver_message_request,
};
use sea_orm::{
    ActiveModelTrait, EntityTrait, IntoActiveModel, Set, TransactionError, TransactionTrait,
};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::{
    entity::{chat_message, conversation, dm_pair, outbox_event},
    events::{ConversationTargetType as EventConversationTargetType, MessageDeletedPayload},
};

use super::channel_write_auth::{ChannelWriteContext, authorize_channel_write};
use super::handler::Handler;
use relay_types::{actor_user_id, payload_value, to_timestamp};

impl Handler {
    pub(super) async fn delete_message(
        &self,
        request: Request<DeleteMessageRequest>,
    ) -> Result<Response<DeleteMessageResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let DeleteMessageRequest { message_id } = request.into_inner();

        let message_id = Uuid::parse_str(&message_id)
            .map_err(|_| Status::invalid_argument("Invalid message ID"))?;

        let mut workspace_client = self.clients.workspace.clone();
        let mut realtime_client = self.clients.realtime.clone();

        let (response, realtime_message) = self
            .connection
            .transaction::<_, (
                Response<DeleteMessageResponse>,
                Option<DeliverMessageRequest>,
            ), Status>(|txn| {
                Box::pin(async move {
                    let now = Utc::now();
                    let message = chat_message::Entity::find_by_id(message_id)
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Chat message lookup failed");
                            Status::internal("Internal Server Error")
                        })?
                        .ok_or_else(|| Status::not_found("Message not found"))?;

                    let conversation = conversation::Entity::find_by_id(message.conversation_id)
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Conversation lookup failed");
                            Status::internal("Internal Server Error")
                        })?
                        .ok_or_else(|| Status::not_found("Conversation not found"))?;

                    let channel_context = if conversation.target_type == "channel" {
                        Some(
                            authorize_channel_write(
                                txn,
                                &mut workspace_client,
                                actor_user_id,
                                &conversation,
                            )
                            .await?,
                        )
                    } else {
                        None
                    };

                    if message.author_user_id != actor_user_id {
                        return Err(Status::permission_denied("Permission denied"));
                    }

                    if message.message_status == "deleted" {
                        return Ok((
                            Response::new(DeleteMessageResponse {
                                message_id: message.message_id.to_string(),
                                conversation_id: message.conversation_id.to_string(),
                                deleted_at: message
                                    .deleted_at
                                    .map(|dt| to_timestamp(dt.with_timezone(&Utc))),
                                deleted: false,
                                already_deleted: true,
                            }),
                            None,
                        ));
                    }

                    let mut target_type = EventConversationTargetType::Dm;
                    let mut workspace_id = None;
                    let mut workspace_channel_id = None;

                    match conversation.target_type.as_str() {
                        "dm" => {
                            let dm_pair_id = conversation
                                .dm_pair_id
                                .ok_or_else(|| Status::not_found("Conversation not found"))?;

                            dm_pair::Entity::find_by_id(dm_pair_id)
                                .one(txn)
                                .await
                                .map_err(|e| {
                                    error!(error = %e, "DM pair lookup failed");
                                    Status::internal("Internal Server Error")
                                })?
                                .ok_or_else(|| Status::not_found("Conversation not found"))?;
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

                    let target_message_seq = message.conversation_message_seq;
                    let mut active: chat_message::ActiveModel = message.into_active_model();
                    active.message_status = Set("deleted".to_string());
                    active.deleted_at = Set(Some(now.into()));
                    active.deleted_by_user_id = Set(Some(actor_user_id));
                    active.updated_at = Set(now.into());
                    active.update(txn).await.map_err(|e| {
                        error!(error = %e, "Chat message delete failed");
                        Status::internal("Internal Server Error")
                    })?;

                    let event_id = Uuid::new_v4();
                    let payload = payload_value(MessageDeletedPayload {
                        delivery_id: event_id.to_string(),
                        message_id: message_id.to_string(),
                        conversation_id: conversation.id.to_string(),
                        target_type,
                        workspace_id: workspace_id.map(|id| id.to_string()),
                        workspace_channel_id: workspace_channel_id.map(|id| id.to_string()),
                        deleted_by_user_id: actor_user_id.to_string(),
                        deleted_at: now.to_rfc3339(),
                    })?;

                    outbox_event::Entity::insert(outbox_event::ActiveModel {
                        event_id: Set(event_id),
                        aggregate_type: Set("chat_message".to_string()),
                        aggregate_id: Set(message_id),
                        event_type: Set("MessageDeleted".to_string()),
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
                        Response::new(DeleteMessageResponse {
                            message_id: message_id.to_string(),
                            conversation_id: conversation.id.to_string(),
                            deleted_at: Some(to_timestamp(now)),
                            deleted: true,
                            already_deleted: false,
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
                                conversation.id.to_string()
                            },
                            occurred_at: Some(to_timestamp(now)),
                            payload: Some(deliver_message_request::Payload::MessageDeleted(
                                RealtimeMessageDeletedPayload {
                                    message_id: message_id.to_string(),
                                    deleted_by_user_id: actor_user_id.to_string(),
                                    target_message_seq,
                                    deleted_at: Some(to_timestamp(now)),
                                },
                            )),
                        }),
                    ))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "Delete message transaction connection failure");
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
