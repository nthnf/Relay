use chrono::Utc;
use relay_proto::friendship::{BlockUserRequest, BlockUserResponse};
use sea_orm::{
    ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set, TransactionError, TransactionTrait,
};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::{
    entity::{friend_request, friendship_edge, outbox_event, user_block},
    events::{
        FriendRequestCanceledByBlockPayload, FriendshipPairPayload, FriendshipRemovedPayload,
        UserBlockedPayload,
    },
};

use super::handler::Handler;
use super::lib::{actor_user_id, payload_value, to_timestamp, user_account_exists};

impl Handler {
    pub(super) async fn block_user(
        &self,
        request: Request<BlockUserRequest>,
    ) -> Result<Response<BlockUserResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;

        let BlockUserRequest { target_user_id } = request.into_inner();
        let target_user_id = Uuid::parse_str(&target_user_id)
            .map_err(|_| Status::invalid_argument("Invalid UUID"))?;

        if actor_user_id == target_user_id {
            return Err(Status::invalid_argument("Cannot block yourself"));
        }

        if !user_account_exists(&self.connection, target_user_id).await? {
            return Err(Status::not_found("User not found"));
        }

        let response = self
            .connection
            .transaction::<_, Response<BlockUserResponse>, Status>(|txn| {
                Box::pin(async move {
                    // Check if a block already exists
                    if let Some(block) = user_block::Entity::find()
                        .filter(user_block::Column::BlockerUserId.eq(actor_user_id))
                        .filter(user_block::Column::BlockedUserId.eq(target_user_id))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "User block lookup failed");
                            Status::internal("Internal Server Error")
                        })?
                    {
                        return Ok(Response::new(BlockUserResponse {
                            blocked: true,
                            already_blocked: true,
                            blocked_at: Some(to_timestamp(block.created_at.with_timezone(&Utc))),
                        }));
                    }

                    let now = Utc::now();

                    // Insert the block record
                    let block = user_block::ActiveModel {
                        blocker_user_id: Set(actor_user_id),
                        blocked_user_id: Set(target_user_id),
                        created_at: Set(now.into()),
                        reason: Set(None),
                    };
                    user_block::Entity::insert(block)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Failed to insert user block");
                            Status::internal("Internal Server Error")
                        })?;

                    let friendship_removed_1 = friendship_edge::Entity::delete_many()
                        .filter(friendship_edge::Column::UserId.eq(actor_user_id))
                        .filter(friendship_edge::Column::FriendUserId.eq(target_user_id))
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Friendship delete failed");
                            Status::internal("Internal Server Error")
                        })?;

                    let friendship_removed_2 = friendship_edge::Entity::delete_many()
                        .filter(friendship_edge::Column::UserId.eq(target_user_id))
                        .filter(friendship_edge::Column::FriendUserId.eq(actor_user_id))
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Friendship delete failed");
                            Status::internal("Internal Server Error")
                        })?;

                    cancel_pending_request(txn, actor_user_id, target_user_id, actor_user_id, now)
                        .await?;
                    cancel_pending_request(txn, target_user_id, actor_user_id, actor_user_id, now)
                        .await?;

                    let removed_any_friendship =
                        friendship_removed_1.rows_affected + friendship_removed_2.rows_affected > 0;
                    if removed_any_friendship {
                        let friendship_removed_event = outbox_event::ActiveModel {
                            event_id: Set(Uuid::new_v4()),
                            aggregate_type: Set("friendship".to_string()),
                            aggregate_id: Set(actor_user_id),
                            event_type: Set("FriendshipRemoved".to_string()),
                            created_at: Set(now.into()),
                            available_at: Set(now.into()),
                            occurred_at: Set(now.into()),
                            claimed_at: Set(None),
                            claimed_by: Set(None),
                            published_at: Set(None),
                            last_error: Set(None),
                            publish_attempts: Set(0),
                            status: Set("pending".to_string()),
                            payload: Set(payload_value(FriendshipRemovedPayload {
                                friendship_pairs: vec![
                                    FriendshipPairPayload {
                                        user_id: actor_user_id.to_string(),
                                        friend_user_id: target_user_id.to_string(),
                                    },
                                    FriendshipPairPayload {
                                        user_id: target_user_id.to_string(),
                                        friend_user_id: actor_user_id.to_string(),
                                    },
                                ],
                                removed_at: now.to_rfc3339(),
                                reason: "blocked".to_string(),
                            })),
                        };
                        outbox_event::Entity::insert(friendship_removed_event)
                            .exec(txn)
                            .await
                            .map_err(|e| {
                                error!(error = %e, "Friendship removed outbox insert failed");
                                Status::internal("Internal Server Error")
                            })?;
                    }

                    let blocked_event = outbox_event::ActiveModel {
                        event_id: Set(Uuid::new_v4()),
                        aggregate_type: Set("user_block".to_string()),
                        aggregate_id: Set(actor_user_id),
                        event_type: Set("UserBlocked".to_string()),
                        created_at: Set(now.into()),
                        available_at: Set(now.into()),
                        occurred_at: Set(now.into()),
                        claimed_at: Set(None),
                        claimed_by: Set(None),
                        published_at: Set(None),
                        last_error: Set(None),
                        publish_attempts: Set(0),
                        status: Set("pending".to_string()),
                        payload: Set(payload_value(UserBlockedPayload {
                            blocker_user_id: actor_user_id.to_string(),
                            blocked_user_id: target_user_id.to_string(),
                            blocked_at: now.to_rfc3339(),
                        })),
                    };
                    outbox_event::Entity::insert(blocked_event)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "User blocked outbox insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    Ok(Response::new(BlockUserResponse {
                        blocked: true,
                        already_blocked: false,
                        blocked_at: Some(to_timestamp(now)),
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "Block user transaction connection failure");
                    Status::internal("Internal Server Error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}

async fn cancel_pending_request<Txn>(
    txn: &Txn,
    requester_user_id: Uuid,
    addressee_user_id: Uuid,
    blocked_by_user_id: Uuid,
    now: chrono::DateTime<Utc>,
) -> Result<(), Status>
where
    Txn: sea_orm::ConnectionTrait,
{
    let pending_request = friend_request::Entity::find()
        .filter(friend_request::Column::RequesterUserId.eq(requester_user_id))
        .filter(friend_request::Column::AddresseeUserId.eq(addressee_user_id))
        .filter(friend_request::Column::Status.eq("pending"))
        .one(txn)
        .await
        .map_err(|e| {
            error!(error = %e, "Friend request lookup failed");
            Status::internal("Internal Server Error")
        })?;

    let Some(pending_request) = pending_request else {
        return Ok(());
    };

    let friend_request_id = pending_request.friend_request_id;
    let mut active_request = pending_request.into_active_model();
    active_request.status = Set("canceled_by_block".to_string());
    active_request.resolved_at = Set(Some(now.into()));
    active_request.resolution_reason = Set(Some("blocked".to_string()));
    friend_request::Entity::update(active_request)
        .exec(txn)
        .await
        .map_err(|e| {
            error!(error = %e, "Friend request update failed");
            Status::internal("Internal Server Error")
        })?;

    let canceled_by_block_event = outbox_event::ActiveModel {
        event_id: Set(Uuid::new_v4()),
        aggregate_type: Set("friend_request".to_string()),
        aggregate_id: Set(friend_request_id),
        event_type: Set("FriendRequestCanceledByBlock".to_string()),
        created_at: Set(now.into()),
        available_at: Set(now.into()),
        occurred_at: Set(now.into()),
        claimed_at: Set(None),
        claimed_by: Set(None),
        published_at: Set(None),
        last_error: Set(None),
        publish_attempts: Set(0),
        status: Set("pending".to_string()),
        payload: Set(payload_value(FriendRequestCanceledByBlockPayload {
            friend_request_id: friend_request_id.to_string(),
            requester_user_id: requester_user_id.to_string(),
            addressee_user_id: addressee_user_id.to_string(),
            blocked_by_user_id: blocked_by_user_id.to_string(),
            canceled_at: now.to_rfc3339(),
            status: "canceled_by_block".to_string(),
        })),
    };
    outbox_event::Entity::insert(canceled_by_block_event)
        .exec(txn)
        .await
        .map_err(|e| {
            error!(error = %e, "Friend request canceled outbox insert failed");
            Status::internal("Internal Server Error")
        })?;

    Ok(())
}
