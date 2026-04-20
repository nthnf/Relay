use chrono::Utc;
use relay_proto::friendship::{RemoveFriendRequest, RemoveFriendResponse};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set, TransactionError, TransactionTrait};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::{
    entity::{friendship_edge, outbox_event},
    events::{FriendshipPairPayload, FriendshipRemovedPayload},
};

use super::handler::Handler;
use relay_types::{actor_user_id, payload_value, to_timestamp};

impl Handler {
    pub(super) async fn remove_friend(
        &self,
        request: Request<RemoveFriendRequest>,
    ) -> Result<Response<RemoveFriendResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;

        let RemoveFriendRequest { friend_user_id } = request.into_inner();
        let friend_user_id = Uuid::parse_str(&friend_user_id)
            .map_err(|_| Status::invalid_argument("Invalid UUID"))?;

        if actor_user_id == friend_user_id {
            return Err(Status::invalid_argument("Cannot remove yourself"));
        }

        let response = self
            .connection
            .transaction::<_, Response<RemoveFriendResponse>, Status>(|txn| {
                Box::pin(async move {
                    let edge_1 = friendship_edge::Entity::delete_many()
                        .filter(friendship_edge::Column::UserId.eq(actor_user_id))
                        .filter(friendship_edge::Column::FriendUserId.eq(friend_user_id))
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Friendship delete failed");
                            Status::internal("Internal Server Error")
                        })?;

                    let edge_2 = friendship_edge::Entity::delete_many()
                        .filter(friendship_edge::Column::UserId.eq(friend_user_id))
                        .filter(friendship_edge::Column::FriendUserId.eq(actor_user_id))
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Friendship delete failed");
                            Status::internal("Internal Server Error")
                        })?;

                    if edge_1.rows_affected + edge_2.rows_affected == 0 {
                        return Ok(Response::new(RemoveFriendResponse {
                            removed: false,
                            removed_at: None,
                        }));
                    }

                    let now = Utc::now();

                    let removed_event = outbox_event::ActiveModel {
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
                                    friend_user_id: friend_user_id.to_string(),
                                },
                                FriendshipPairPayload {
                                    user_id: friend_user_id.to_string(),
                                    friend_user_id: actor_user_id.to_string(),
                                },
                            ],
                            removed_at: now.to_rfc3339(),
                            reason: "removed_by_user".to_string(),
                        })?),
                    };
                    outbox_event::Entity::insert(removed_event)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Friendship removed outbox insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    Ok(Response::new(RemoveFriendResponse {
                        removed: true,
                        removed_at: Some(to_timestamp(now)),
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "Remove friend transaction connection failure");
                    Status::internal("Internal Server Error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}
