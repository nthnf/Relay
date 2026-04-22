use chrono::Utc;
use relay_proto::chat::{ListConversationMessagesRequest, ListMessagesResponse, MessageSummary};
use sea_orm::{
    ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, TransactionError,
    TransactionTrait,
};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::entity::{chat_message, conversation, dm_pair, workspace_channel_snapshot};

use super::handler::Handler;
use relay_types::{actor_user_id, to_timestamp};

impl Handler {
    pub(super) async fn list_conversation_messages(
        &self,
        request: Request<ListConversationMessagesRequest>,
    ) -> Result<Response<ListMessagesResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let ListConversationMessagesRequest {
            conversation_id,
            page_size,
            before_conversation_message_seq,
        } = request.into_inner();

        let conversation_id = Uuid::parse_str(&conversation_id)
            .map_err(|_| Status::invalid_argument("Invalid conversation ID"))?;

        let page_size = page_size.unwrap_or(20).clamp(1, 50) as u64;

        let response = self
            .connection
            .transaction::<_, Response<ListMessagesResponse>, Status>(|txn| {
                Box::pin(async move {
                    // Check valid conversation
                    let conversation = conversation::Entity::find_by_id(conversation_id)
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Conversation lookup failed");
                            Status::internal("Internal Server Error")
                        })?
                        .ok_or_else(|| Status::not_found("Conversation not found"))?;

                    // Match target
                    let target_type = conversation.target_type.as_str();
                    match target_type {
                        "dm" => {
                            let dm_pair_id = conversation
                                .dm_pair_id
                                .ok_or_else(|| Status::not_found("Conversation not found"))?;

                            let dm_pair = dm_pair::Entity::find_by_id(dm_pair_id)
                                .one(txn)
                                .await
                                .map_err(|e| {
                                    error!(error = %e, "Conversation lookup failed");
                                    Status::internal("Internal Server Error")
                                })?
                                .ok_or_else(|| Status::not_found("Conversation not found"))?;

                            if actor_user_id != dm_pair.low_user_id
                                && actor_user_id != dm_pair.high_user_id
                            {
                                return Err(Status::permission_denied("Permission denied"));
                            }
                        }
                        "channel" => {
                            let Some(workspace_channel_id) = conversation.workspace_channel_id else {
                                return Err(Status::not_found("Conversation not found"));
                            };
                            workspace_channel_snapshot::Entity::find_by_id(workspace_channel_id)
                                .one(txn)
                                .await
                            .map_err(|e| {
                                    error!(error = %e, "Workspace channel snapshot lookup failed");
                                    Status::internal("Internal Server Error")
                                })?.ok_or_else(|| Status::not_found("Workspace channel not found"))?;
                        }
                        _ => {
                            return Err(Status::internal("Internal Server Error"));
                        }
                    }

                    let mut query = chat_message::Entity::find()
                        .filter(chat_message::Column::ConversationId.eq(conversation_id))
                        .order_by_desc(chat_message::Column::ConversationMessageSeq)
                        .limit(page_size + 1);

                    if let Some(before_conversation_message_seq) = before_conversation_message_seq {
                        query = query.filter(
                            chat_message::Column::ConversationMessageSeq
                                .lt(before_conversation_message_seq),
                        );
                    }

                    let mut rows = query.all(txn).await.map_err(|e| {
                        error!(error = %e, "Chat message lookup failed");
                        Status::internal("Internal Server Error")
                    })?;

                    let has_more = rows.len() > page_size as usize;
                    if has_more {
                        rows.pop();
                    }

                    let next_before_conversation_message_seq = if has_more {
                        rows.last().map(|row| row.conversation_message_seq)
                    } else {
                        None
                    };

                    Ok(Response::new(ListMessagesResponse {
                        messages: rows
                            .into_iter()
                            .map(|row| MessageSummary {
                                message_id: row.message_id.to_string(),
                                author_user_id: row.author_user_id.to_string(),
                                conversation_message_seq: row.conversation_message_seq,
                                body: row.body,
                                message_status: row.message_status,
                                created_at: Some(to_timestamp(row.created_at.with_timezone(&Utc))),
                                last_edited_at: row
                                    .last_edited_at
                                    .map(|dt| to_timestamp(dt.with_timezone(&Utc))),
                                deleted_at: row.deleted_at.map(|dt| to_timestamp(dt.with_timezone(&Utc))),
                            })
                            .collect(),
                        next_before_conversation_message_seq,
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "List conversation messages transaction connection failure");
                    Status::internal("Internal Server Error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}
