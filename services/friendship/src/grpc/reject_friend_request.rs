use chrono::Utc;
use relay_proto::friendship::{RejectFriendRequestRequest, RejectFriendRequestResponse};
use sea_orm::{EntityTrait, IntoActiveModel, Set, TransactionError, TransactionTrait};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::{
    entity::{friend_request, outbox_event},
    events::FriendRequestRejectedPayload,
};

use super::handler::Handler;
use relay_types::{actor_user_id, payload_value, to_timestamp};

impl Handler {
    pub(super) async fn reject_friend_request(
        &self,
        request: Request<RejectFriendRequestRequest>,
    ) -> Result<Response<RejectFriendRequestResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;

        let RejectFriendRequestRequest { friend_request_id } = request.into_inner();
        let friend_request_id = Uuid::parse_str(&friend_request_id)
            .map_err(|_| Status::invalid_argument("Invalid UUID"))?;

        let response = self
            .connection
            .transaction::<_, Response<RejectFriendRequestResponse>, Status>(|txn| {
                Box::pin(async move {
                    // Read, validate, and mutate request inside one transaction.
                    let request = friend_request::Entity::find_by_id(friend_request_id)
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Friend request lookup failed");
                            Status::internal("Internal Server Error")
                        })?;

                    let request =
                        request.ok_or_else(|| Status::not_found("Friend request not found"))?;

                    if actor_user_id != request.addressee_user_id {
                        return Err(Status::permission_denied("Permission Denied"));
                    }

                    if request.status != "pending" {
                        return Err(Status::failed_precondition("Friend request is not pending"));
                    }

                    let requester_id = request.requester_user_id;

                    let now = Utc::now();

                    let mut accept_friend_request = request.into_active_model();
                    accept_friend_request.status = Set("rejected".to_string());
                    accept_friend_request.resolved_at = Set(Some(now.into()));
                    accept_friend_request.resolution_reason = Set(Some("rejected".to_string()));
                    friend_request::Entity::update(accept_friend_request)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Friend request update failed");
                            Status::internal("Internal Server Error")
                        })?;

                    let friend_request_rejected_event = outbox_event::ActiveModel {
                        event_id: Set(Uuid::new_v4()),
                        aggregate_type: Set("friend_request".to_string()),
                        aggregate_id: Set(friend_request_id),
                        event_type: Set("FriendRequestRejected".to_string()),
                        created_at: Set(now.into()),
                        available_at: Set(now.into()),
                        occurred_at: Set(now.into()),
                        claimed_at: Set(None),
                        claimed_by: Set(None),
                        published_at: Set(None),
                        last_error: Set(None),
                        publish_attempts: Set(0),
                        status: Set("pending".to_string()),
                        payload: Set(payload_value(FriendRequestRejectedPayload {
                            addressee_user_id: actor_user_id.to_string(),
                            requester_user_id: requester_id.to_string(),
                            friend_request_id: friend_request_id.to_string(),
                            rejected_at: now.to_rfc3339(),
                        })?),
                    };
                    outbox_event::Entity::insert(friend_request_rejected_event)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Friend request rejected outbox insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    Ok(Response::new(RejectFriendRequestResponse {
                        friend_request_id: friend_request_id.to_string(),
                        resolved_at: Some(to_timestamp(now)),
                        status: "rejected".to_string(),
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
