use chrono::Utc;
use relay_proto::friendship::{CreateFriendRequestRequest, FriendRequestRecord};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set, TransactionError, TransactionTrait};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::{
    entity::{friend_request, friendship_edge, outbox_event, user_block},
    events::FriendRequestCreatedPayload,
};

use super::handler::Handler;
use super::lib::{actor_user_id, payload_value, to_timestamp};

impl Handler {
    pub(super) async fn create_friend_request(
        &self,
        request: Request<CreateFriendRequestRequest>,
    ) -> Result<Response<FriendRequestRecord>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let CreateFriendRequestRequest { target_user_id } = request.into_inner();
        let user_id = Uuid::parse_str(&target_user_id)
            .map_err(|_| Status::invalid_argument("Invalid UUID"))?;

        if actor_user_id == user_id {
            return Err(Status::invalid_argument("Cannot friend yourself"));
        }

        // Check if user exists
        if !self.identity.user_exists(actor_user_id, user_id).await? {
            return Err(Status::not_found("User not found"));
        }

        let now = Utc::now();
        let friend_request_id = Uuid::new_v4();

        let response = self
            .connection
            .transaction::<_, Response<FriendRequestRecord>, Status>(|txn| {
                Box::pin(async move {
                    // Check for friendship, block, and existing request inside transaction.
                    if friendship_edge::Entity::find()
                        .filter(friendship_edge::Column::UserId.eq(actor_user_id))
                        .filter(friendship_edge::Column::FriendUserId.eq(user_id))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Friendship lookup failed");
                            Status::internal("Internal Server Error")
                        })?
                        .is_some()
                    {
                        return Err(Status::already_exists("Already a friend"));
                    }

                    if friendship_edge::Entity::find()
                        .filter(friendship_edge::Column::UserId.eq(user_id))
                        .filter(friendship_edge::Column::FriendUserId.eq(actor_user_id))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Friendship lookup failed");
                            Status::internal("Internal Server Error")
                        })?
                        .is_some()
                    {
                        return Err(Status::already_exists("Already a friend"));
                    }

                    if user_block::Entity::find()
                        .filter(user_block::Column::BlockerUserId.eq(actor_user_id))
                        .filter(user_block::Column::BlockedUserId.eq(user_id))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "User block lookup failed");
                            Status::internal("Internal Server Error")
                        })?
                        .is_some()
                    {
                        return Err(Status::failed_precondition("Blocked"));
                    }

                    if user_block::Entity::find()
                        .filter(user_block::Column::BlockerUserId.eq(user_id))
                        .filter(user_block::Column::BlockedUserId.eq(actor_user_id))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "User block lookup failed");
                            Status::internal("Internal Server Error")
                        })?
                        .is_some()
                    {
                        return Err(Status::failed_precondition("Blocked"));
                    }

                    if friend_request::Entity::find()
                        .filter(friend_request::Column::RequesterUserId.eq(actor_user_id))
                        .filter(friend_request::Column::AddresseeUserId.eq(user_id))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Friend request lookup failed");
                            Status::internal("Internal Server Error")
                        })?
                        .is_some()
                    {
                        return Err(Status::already_exists("Request Existed"));
                    }

                    if friend_request::Entity::find()
                        .filter(friend_request::Column::RequesterUserId.eq(user_id))
                        .filter(friend_request::Column::AddresseeUserId.eq(actor_user_id))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Friend request lookup failed");
                            Status::internal("Internal Server Error")
                        })?
                        .is_some()
                    {
                        return Err(Status::already_exists("Request Existed"));
                    }

                    let friend_request = friend_request::ActiveModel {
                        friend_request_id: Set(friend_request_id),
                        requester_user_id: Set(actor_user_id),
                        addressee_user_id: Set(user_id),
                        status: Set("pending".to_string()),
                        created_at: Set(now.into()),
                        resolved_at: Set(None),
                        resolution_reason: Set(None),
                    };
                    friend_request::Entity::insert(friend_request)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Friend request insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    let friend_request_created_event = outbox_event::ActiveModel {
                        event_id: Set(Uuid::new_v4()),
                        aggregate_type: Set("friend_request".to_string()),
                        aggregate_id: Set(friend_request_id),
                        event_type: Set("FriendRequestCreated".to_string()),
                        created_at: Set(now.into()),
                        available_at: Set(now.into()),
                        occurred_at: Set(now.into()),
                        claimed_at: Set(None),
                        claimed_by: Set(None),
                        published_at: Set(None),
                        last_error: Set(None),
                        publish_attempts: Set(0),
                        status: Set("pending".to_string()),
                        payload: Set(payload_value(FriendRequestCreatedPayload {
                            friend_request_id: friend_request_id.to_string(),
                            requester_user_id: actor_user_id.to_string(),
                            addressee_user_id: user_id.to_string(),
                            created_at: now.to_rfc3339(),
                            status: "pending".to_string(),
                        })),
                    };
                    outbox_event::Entity::insert(friend_request_created_event)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Friend request created outbox insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    Ok(Response::new(FriendRequestRecord {
                        friend_request_id: friend_request_id.to_string(),
                        requester_user_id: actor_user_id.to_string(),
                        addressee_user_id: user_id.to_string(),
                        status: "pending".to_string(),
                        created_at: Some(to_timestamp(now)),
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "Friend request transaction connection failure");
                    Status::internal("Internal Server Error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}
