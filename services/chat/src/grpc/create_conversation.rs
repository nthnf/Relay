use chrono::Utc;
use relay_proto::chat::{
    ConversationTargetType, CreateConversationRequest, CreateConversationResponse,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set, TransactionError, TransactionTrait};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::entity::{
    conversation, dm_pair, outbox_event, user_snapshot, workspace_channel_snapshot,
};

use super::handler::Handler;
use crate::events::{
    ConversationCreatedPayload, ConversationTargetType as EventConversationTargetType,
    DmPairCreatedPayload,
};
use relay_types::{actor_user_id, payload_value, to_timestamp};

impl Handler {
    pub(super) async fn create_conversation(
        &self,
        request: Request<CreateConversationRequest>,
    ) -> Result<Response<CreateConversationResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let CreateConversationRequest {
            target_type,
            peer_user_id,
            workspace_channel_id,
        } = request.into_inner();

        let target_type = ConversationTargetType::try_from(target_type)
            .map_err(|_| Status::invalid_argument("Invalid conversation target type"))?;

        let response = self
            .connection
            .transaction::<_, Response<CreateConversationResponse>, Status>(|txn| {
                Box::pin(async move {
                    let now = Utc::now();
                    let conversation_id = match target_type {
                        ConversationTargetType::Dm => {
                            let Some(peer_user_id) = peer_user_id else {
                                return Err(Status::invalid_argument("Peer user ID is required"));
                            };
                            let peer_user_id = Uuid::parse_str(&peer_user_id)
                                .map_err(|_| Status::invalid_argument("Invalid peer user ID"))?;

                            if peer_user_id == actor_user_id {
                                return Err(Status::invalid_argument("Cannot target self"));
                            }

                            user_snapshot::Entity::find_by_id(peer_user_id)
                                .one(txn)
                                .await
                                .map_err(|e| {
                                    error!(error = %e, "User snapshot lookup failed");
                                    Status::internal("Internal Server Error")
                                })?
                                .ok_or_else(|| Status::not_found("User not found"))?;

                            let (low_user_id, high_user_id) = if peer_user_id < actor_user_id {
                                (peer_user_id, actor_user_id)
                            } else {
                                (actor_user_id, peer_user_id)
                            };

                            let conversation_id = Uuid::new_v4();
                            let dm_pair = dm_pair::Entity::find()
                                .filter(dm_pair::Column::LowUserId.eq(low_user_id))
                                .filter(dm_pair::Column::HighUserId.eq(high_user_id))
                                .one(txn)
                                .await
                                .map_err(|e| {
                                    error!(error = %e, "DM pair lookup failed");
                                    Status::internal("Internal Server Error")
                                })?;

                            if let Some(dm_pair) = dm_pair {
                                if let Some(existing_convo) = conversation::Entity::find()
                                    .filter(conversation::Column::DmPairId.eq(dm_pair.id))
                                    .one(txn)
                                    .await
                                    .map_err(|e| {
                                        error!(error = %e, "Conversation lookup failed");
                                        Status::internal("Internal Server Error")
                                    })?
                                {
                                    return Err(Status::already_exists(format!(
                                        "Conversation already exists: {}",
                                        existing_convo.id
                                    )));
                                }

                                conversation::Entity::insert(conversation::ActiveModel {
                                    id: Set(conversation_id),
                                    target_type: Set("dm".to_string()),
                                    dm_pair_id: Set(Some(dm_pair.id)),
                                    workspace_channel_id: Set(None),
                                    created_at: Set(now.into()),
                                })
                                .exec(txn)
                                .await
                                .map_err(|e| {
                                    error!(error = %e, "Conversation insert failed");
                                    Status::internal("Internal Server Error")
                                })?;

                                let event_id = Uuid::new_v4();
                                outbox_event::Entity::insert(outbox_event::ActiveModel {
                                    event_id: Set(event_id),
                                    aggregate_type: Set("conversation".to_string()),
                                    aggregate_id: Set(conversation_id),
                                    event_type: Set("ConversationCreated".to_string()),
                                    payload: Set(payload_value(ConversationCreatedPayload {
                                        conversation_id: conversation_id.to_string(),
                                        target_type: EventConversationTargetType::Dm,
                                        dm_pair_id: Some(dm_pair.id.to_string()),
                                        workspace_channel_id: None,
                                        created_at: now.to_rfc3339(),
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
                                    error!(error = %e, "Conversation created event insert failed");
                                    Status::internal("Internal Server Error")
                                })?;

                                conversation_id
                            } else {
                                let dm_pair_id = Uuid::new_v4();
                                dm_pair::Entity::insert(dm_pair::ActiveModel {
                                    id: Set(dm_pair_id),
                                    low_user_id: Set(low_user_id),
                                    high_user_id: Set(high_user_id),
                                    created_at: Set(now.into()),
                                })
                                .exec(txn)
                                .await
                                .map_err(|e| {
                                    error!(error = %e, "DM pair insert failed");
                                    Status::internal("Internal Server Error")
                                })?;

                                let dm_pair_event_id = Uuid::new_v4();
                                outbox_event::Entity::insert(outbox_event::ActiveModel {
                                    event_id: Set(dm_pair_event_id),
                                    aggregate_type: Set("dm_pair".to_string()),
                                    aggregate_id: Set(dm_pair_id),
                                    event_type: Set("DmPairCreated".to_string()),
                                    payload: Set(payload_value(DmPairCreatedPayload {
                                        dm_pair_id: dm_pair_id.to_string(),
                                        low_user_id: low_user_id.to_string(),
                                        high_user_id: high_user_id.to_string(),
                                        created_at: now.to_rfc3339(),
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
                                    error!(error = %e, "DM pair created event insert failed");
                                    Status::internal("Internal Server Error")
                                })?;

                                conversation::Entity::insert(conversation::ActiveModel {
                                    id: Set(conversation_id),
                                    target_type: Set("dm".to_string()),
                                    dm_pair_id: Set(Some(dm_pair_id)),
                                    workspace_channel_id: Set(None),
                                    created_at: Set(now.into()),
                                })
                                .exec(txn)
                                .await
                                .map_err(|e| {
                                    error!(error = %e, "Conversation insert failed");
                                    Status::internal("Internal Server Error")
                                })?;

                                let conversation_event_id = Uuid::new_v4();
                                outbox_event::Entity::insert(outbox_event::ActiveModel {
                                    event_id: Set(conversation_event_id),
                                    aggregate_type: Set("conversation".to_string()),
                                    aggregate_id: Set(conversation_id),
                                    event_type: Set("ConversationCreated".to_string()),
                                    payload: Set(payload_value(ConversationCreatedPayload {
                                        conversation_id: conversation_id.to_string(),
                                        target_type: EventConversationTargetType::Dm,
                                        dm_pair_id: Some(dm_pair_id.to_string()),
                                        workspace_channel_id: None,
                                        created_at: now.to_rfc3339(),
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
                                    error!(error = %e, "Conversation created event insert failed");
                                    Status::internal("Internal Server Error")
                                })?;

                                conversation_id
                            }
                        }
                        ConversationTargetType::WorkspaceChannel => {
                            let Some(workspace_channel_id) = workspace_channel_id else {
                                return Err(Status::invalid_argument(
                                    "Workspace channel ID is required",
                                ));
                            };
                            let workspace_channel_id = Uuid::parse_str(&workspace_channel_id)
                                .map_err(|_| {
                                    Status::invalid_argument("Invalid workspace channel ID")
                                })?;

                            // Check valid workspace channel
                            workspace_channel_snapshot::Entity::find_by_id(workspace_channel_id)
                                .one(txn)
                                .await
                                .map_err(|e| {
                                    error!(error = %e, "Workspace channel snapshot lookup failed");
                                    Status::internal("Internal Server Error")
                                })?
                                .ok_or_else(|| Status::not_found("Workspace channel not found"))?;

                            // Check existing conversation
                            let existing_convo = conversation::Entity::find()
                                .filter(
                                    conversation::Column::WorkspaceChannelId
                                        .eq(workspace_channel_id),
                                )
                                .one(txn)
                                .await
                                .map_err(|e| {
                                    error!(error = %e, "Workspace channel snapshot lookup failed");
                                    Status::internal("Internal Server Error")
                                })?;
                            if existing_convo.is_some() {
                                return Err(Status::already_exists("Conversation already exists"));
                            }

                            // Create conversation
                            let conversation_id = Uuid::new_v4();
                            let conversation = conversation::ActiveModel {
                                id: Set(conversation_id),
                                target_type: Set("channel".to_string()),
                                dm_pair_id: Set(None),
                                workspace_channel_id: Set(Some(workspace_channel_id)),
                                created_at: Set(now.into()),
                            };
                            conversation::Entity::insert(conversation)
                                .exec(txn)
                                .await
                                .map_err(|e| {
                                    error!(error = %e, "Conversation insert failed");
                                    Status::internal("Internal Server Error")
                                })?;

                            let event_id = Uuid::new_v4();
                            outbox_event::Entity::insert(outbox_event::ActiveModel {
                                event_id: Set(event_id),
                                aggregate_type: Set("conversation".to_string()),
                                aggregate_id: Set(conversation_id),
                                event_type: Set("ConversationCreated".to_string()),
                                payload: Set(payload_value(ConversationCreatedPayload {
                                    conversation_id: conversation_id.to_string(),
                                    target_type: EventConversationTargetType::WorkspaceChannel,
                                    dm_pair_id: None,
                                    workspace_channel_id: Some(workspace_channel_id.to_string()),
                                    created_at: now.to_rfc3339(),
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
                                error!(error = %e, "Conversation created event insert failed");
                                Status::internal("Internal Server Error")
                            })?;

                            conversation_id
                        }
                        ConversationTargetType::Unspecified => {
                            return Err(Status::invalid_argument(
                                "Invalid conversation target type",
                            ));
                        }
                    };

                    Ok(Response::new(CreateConversationResponse {
                        conversation_id: conversation_id.to_string(),
                        created_at: Some(to_timestamp(now)),
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "Create conversation transaction connection failure");
                    Status::internal("Internal Server Error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}
