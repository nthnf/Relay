use chrono::Utc;
use relay_proto::identity::{ResendVerificationEmailRequest, ResendVerificationEmailResponse};
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set, TransactionError, TransactionTrait};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::entity::{email_verification_token, outbox_event, user_account};
use crate::event::VerificationEmailRequestedPayload;

use super::handler::{Handler, payload_value};

impl Handler {
    pub(super) async fn resend_verification_email(
        &self,
        request: Request<ResendVerificationEmailRequest>,
    ) -> Result<Response<ResendVerificationEmailResponse>, Status> {
        let ResendVerificationEmailRequest { email } = request.into_inner();

        let email_normalized = email.to_lowercase();
        let user = user_account::Entity::find()
            .filter(user_account::Column::EmailNormalized.eq(email_normalized))
            .one(&self.connection)
            .await
            .map_err(|e| {
                error!(error = %e, "identity resend verification email lookup failed");
                Status::internal("internal server error")
            })?;

        let Some(user) = user else {
            return Ok(Response::new(ResendVerificationEmailResponse { accepted: true }));
        };

        if user.email_verified_at.is_some() || user.account_status != "active" {
            return Ok(Response::new(ResendVerificationEmailResponse { accepted: true }));
        }

        let now = Utc::now();

        let response = self
            .connection
            .transaction::<_, Response<ResendVerificationEmailResponse>, Status>(|txn| {
                Box::pin(async move {
                    let existing_tokens = email_verification_token::Entity::find()
                        .filter(email_verification_token::Column::UserId.eq(user.user_id))
                        .filter(email_verification_token::Column::ConsumedAt.is_null())
                        .filter(email_verification_token::Column::ExpiresAt.gt(now))
                        .all(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "identity resend verification email token lookup failed");
                            Status::internal("internal server error")
                        })?;

                    for token in existing_tokens {
                        let mut token = token.into_active_model();
                        token.consumed_at = Set(Some(now.into()));
                        email_verification_token::Entity::update(token)
                            .exec(txn)
                            .await
                            .map_err(|e| {
                                error!(error = %e, "identity resend verification email existing token consume failed");
                                Status::internal("internal server error")
                            })?;
                    }

                    let new_token_id = Uuid::new_v4();
                    let new_token = Uuid::new_v4().to_string();
                    let new_token_hash = crate::auth::hash_token(&new_token);

                    let email_verification_token = email_verification_token::ActiveModel {
                        token_id: Set(new_token_id),
                        token_hash: Set(new_token_hash.clone()),
                        user_id: Set(user.user_id),
                        created_at: Set(now.into()),
                        expires_at: Set((now + chrono::Duration::hours(6)).into()),
                        consumed_at: Set(None),
                    };
                    email_verification_token::Entity::insert(email_verification_token)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "identity resend verification email new token insert failed");
                            Status::internal("internal server error")
                        })?;

                    let event = outbox_event::ActiveModel {
                        event_id: Set(Uuid::new_v4()),
                        aggregate_type: Set("user_account".to_string()),
                        aggregate_id: Set(user.user_id),
                        event_type: Set("VerificationEmailRequested".to_string()),
                        created_at: Set(now.into()),
                        available_at: Set(now.into()),
                        occurred_at: Set(now.into()),
                        claimed_at: Set(None),
                        claimed_by: Set(None),
                        published_at: Set(None),
                        last_error: Set(None),
                        publish_attempts: Set(0),
                        status: Set("pending".to_string()),
                        payload: Set(payload_value(VerificationEmailRequestedPayload {
                            user_id: user.user_id.to_string(),
                            email: user.email.clone(),
                            verification_token: new_token,
                            verification_token_expires_at: (now + chrono::Duration::hours(6))
                                .to_rfc3339(),
                            verification_token_id: new_token_id.to_string(),
                            reason: "resend_verification".to_string(),
                            requested_at: now.to_rfc3339(),
                        })),
                    };
                    outbox_event::Entity::insert(event).exec(txn).await.map_err(|e| {
                        error!(error = %e, "identity resend verification email outbox insert failed");
                        Status::internal("internal server error")
                    })?;

                    Ok(Response::new(ResendVerificationEmailResponse { accepted: true }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "identity resend verification email transaction connection failure");
                    Status::internal("internal server error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}
