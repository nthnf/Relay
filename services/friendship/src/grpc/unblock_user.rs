use chrono::Utc;
use relay_proto::friendship::{UnblockUserRequest, UnblockUserResponse};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set, TransactionError, TransactionTrait};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::{
    entity::{outbox_event, user_block},
    events::UserUnblockedPayload,
};

use super::handler::Handler;
use super::lib::{actor_user_id, payload_value, to_timestamp};

impl Handler {
    pub(super) async fn unblock_user(
        &self,
        request: Request<UnblockUserRequest>,
    ) -> Result<Response<UnblockUserResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;

        let UnblockUserRequest { target_user_id } = request.into_inner();
        let target_user_id = Uuid::parse_str(&target_user_id)
            .map_err(|_| Status::invalid_argument("Invalid UUID"))?;

        if actor_user_id == target_user_id {
            return Err(Status::invalid_argument("Cannot unblock yourself"));
        }

        if !self
            .identity
            .user_exists(actor_user_id, target_user_id)
            .await?
        {
            return Err(Status::not_found("User not found"));
        }

        let response = self
            .connection
            .transaction::<_, Response<UnblockUserResponse>, Status>(|txn| {
                Box::pin(async move {
                    let delete_result = user_block::Entity::delete_many()
                        .filter(user_block::Column::BlockerUserId.eq(actor_user_id))
                        .filter(user_block::Column::BlockedUserId.eq(target_user_id))
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "User block delete failed");
                            Status::internal("Internal Server Error")
                        })?;

                    if delete_result.rows_affected == 0 {
                        return Ok(Response::new(UnblockUserResponse {
                            unblocked: false,
                            unblocked_at: None,
                        }));
                    }

                    let now = Utc::now();

                    let unblocked_event = outbox_event::ActiveModel {
                        event_id: Set(Uuid::new_v4()),
                        aggregate_type: Set("user_block".to_string()),
                        aggregate_id: Set(actor_user_id),
                        event_type: Set("UserUnblocked".to_string()),
                        created_at: Set(now.into()),
                        available_at: Set(now.into()),
                        occurred_at: Set(now.into()),
                        claimed_at: Set(None),
                        claimed_by: Set(None),
                        published_at: Set(None),
                        last_error: Set(None),
                        publish_attempts: Set(0),
                        status: Set("pending".to_string()),
                        payload: Set(payload_value(UserUnblockedPayload {
                            blocker_user_id: actor_user_id.to_string(),
                            blocked_user_id: target_user_id.to_string(),
                            unblocked_at: now.to_rfc3339(),
                        })),
                    };
                    outbox_event::Entity::insert(unblocked_event)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "User unblocked outbox insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    Ok(Response::new(UnblockUserResponse {
                        unblocked: true,
                        unblocked_at: Some(to_timestamp(now)),
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "Unblock transaction connection failure");
                    Status::internal("Internal Server Error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}
