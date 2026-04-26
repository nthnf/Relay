use chrono::Utc;
use relay_proto::identity::{UpdateUserProfileRequest, UpdateUserProfileResponse};
use sea_orm::{EntityTrait, IntoActiveModel, Set, TransactionError, TransactionTrait};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::entity::{outbox_event, user_profile};
use crate::events::UserProfileUpdatedPayload;

use super::handler::{Handler, actor_user_id, payload_value, to_timestamp};

impl Handler {
    pub(super) async fn update_user_profile(
        &self,
        request: Request<UpdateUserProfileRequest>,
    ) -> Result<Response<UpdateUserProfileResponse>, Status> {
        let user_id = actor_user_id(&request)?;
        let UpdateUserProfileRequest {
            display_name,
            avatar_url,
        } = request.into_inner();

        let now = Utc::now();

        let response = self
            .connection
            .transaction::<_, Response<UpdateUserProfileResponse>, Status>(|txn| {
                Box::pin(async move {
                    let profile = user_profile::Entity::find_by_id(user_id)
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "identity update user profile lookup failed");
                            Status::internal("internal server error")
                        })?
                        .ok_or_else(|| Status::not_found("user profile not found"))?;
                    let username = profile.username.clone();
                    let final_avatar_url = match avatar_url.clone() {
                        Some(avt) => {
                            let avt = avt.trim().to_string();
                            if avt.is_empty() {
                                None
                            } else {
                                Some(avt)
                            }
                        }
                        None => profile.avatar_url.clone(),
                    };

                    let mut profile = profile.into_active_model();
                    profile.display_name = Set(display_name.clone());
                    if avatar_url.is_some() {
                        profile.avatar_url = Set(final_avatar_url.clone());
                    }
                    user_profile::Entity::update(profile)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "identity update user profile update failed");
                            Status::internal("internal server error")
                        })?;

                    let event = outbox_event::ActiveModel {
                        event_id: Set(Uuid::new_v4()),
                        aggregate_type: Set("user_profile".to_string()),
                        aggregate_id: Set(user_id),
                        event_type: Set("UserProfileUpdated".to_string()),
                        status: Set("pending".to_string()),
                        publish_attempts: Set(0),
                        created_at: Set(now.into()),
                        available_at: Set(now.into()),
                        occurred_at: Set(now.into()),
                        claimed_by: Set(None),
                        claimed_at: Set(None),
                        published_at: Set(None),
                        last_error: Set(None),
                        payload: Set(payload_value(UserProfileUpdatedPayload {
                            user_id: user_id.to_string(),
                            username: username.clone(),
                            display_name: display_name.clone(),
                            avatar_url: final_avatar_url.clone(),
                            updated_at: now.to_rfc3339(),
                        })?),
                    };
                    outbox_event::Entity::insert(event).exec(txn).await.map_err(|e| {
                        error!(error = %e, "identity update user profile outbox insert failed");
                        Status::internal("internal server error")
                    })?;

                    Ok(Response::new(UpdateUserProfileResponse {
                        user_id: user_id.to_string(),
                        username,
                        updated_at: Some(to_timestamp(now)),
                        display_name,
                        avatar_url: final_avatar_url,
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "identity update user profile transaction connection failure");
                    Status::internal("internal server error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthKeys;
    use sea_orm::{DbBackend, MockDatabase};
    use tonic::Request;
    use uuid::Uuid;

    fn update_profile_request(user_id: Option<Uuid>) -> Request<UpdateUserProfileRequest> {
        let mut request = Request::new(UpdateUserProfileRequest {
            display_name: "Alice Updated".to_string(),
            avatar_url: None,
        });

        if let Some(user_id) = user_id {
            request.metadata_mut().insert(
                relay_types::ACTOR_USER_ID_METADATA,
                user_id
                    .to_string()
                    .parse()
                    .expect("user id metadata should be valid"),
            );
        }

        request
    }

    fn test_service() -> Handler {
        Handler {
            connection: MockDatabase::new(DbBackend::Postgres).into_connection(),
            auth: AuthKeys::from_shared_secret(b"test-secret-key"),
        }
    }

    #[tokio::test]
    async fn update_user_profile_requires_actor_context() {
        let error = test_service()
            .update_user_profile(update_profile_request(None))
            .await
            .expect_err("missing actor metadata should fail");

        assert_eq!(error.code(), tonic::Code::Unauthenticated);
        assert_eq!(error.message(), "missing authenticated actor context");
    }
}
