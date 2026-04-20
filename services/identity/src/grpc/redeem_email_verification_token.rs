use chrono::{Duration, Utc};
use relay_proto::identity::{RedeemEmailVerificationTokenRequest, TokenPairResponse};
use sea_orm::{
    ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set, TransactionError, TransactionTrait,
};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::auth::{ACCESS_TOKEN_VALIDITY, hash_token};
use crate::entity::{
    email_verification_token, outbox_event, user_account, user_profile, user_session,
};
use crate::events::UserEmailVerifiedPayload;

use super::handler::{Handler, payload_value, to_timestamp};

impl Handler {
    pub(super) async fn redeem_email_verification_token(
        &self,
        request: Request<RedeemEmailVerificationTokenRequest>,
    ) -> Result<Response<TokenPairResponse>, Status> {
        let RedeemEmailVerificationTokenRequest { token } = request.into_inner();

        let existing_token = email_verification_token::Entity::find()
            .filter(email_verification_token::Column::TokenHash.eq(hash_token(&token)))
            .one(&self.connection)
            .await
            .map_err(|e| {
                error!(error = %e, "identity redeem verification token lookup failed");
                Status::internal("internal server error")
            })?
            .ok_or_else(|| Status::not_found("invalid verification token"))?;
        if existing_token.consumed_at.is_some() {
            return Err(Status::not_found("invalid verification token"));
        }
        if existing_token.expires_at < Utc::now() {
            return Err(Status::not_found("invalid verification token"));
        }

        let user_id = existing_token.user_id;
        let now = Utc::now();
        let session_id = Uuid::new_v4();
        let refresh_token = Uuid::new_v4().to_string();
        let refresh_token_hash = hash_token(&refresh_token);
        let access_token = self
            .auth
            .sign_access_token(crate::auth::AccessClaims {
                user_id,
                session_id,
            })
            .map_err(|e| {
                error!(error = %e, "identity refresh session access token signing failed");
                Status::internal("internal server error")
            })?;

        let account = user_account::Entity::find_by_id(user_id)
            .one(&self.connection)
            .await
            .map_err(|e| {
                error!(error = %e, "identity redeem verification token user lookup failed");
                Status::internal("internal server error")
            })?
            .ok_or_else(|| Status::internal("internal server error"))?;
        if account.account_status != "active" {
            return Err(Status::failed_precondition("account is not active"));
        }
        let email = account.email.clone();

        let profile = user_profile::Entity::find_by_id(user_id)
            .one(&self.connection)
            .await
            .map_err(|e| {
                error!(error = %e, "identity redeem verification token user profile lookup failed");
                Status::internal("internal server error")
            })?
            .ok_or_else(|| Status::internal("internal server error"))?;

        let response = self
            .connection
            .transaction::<_, Response<TokenPairResponse>, Status>(|txn| {
                Box::pin(async move {
                    let mut account = account.into_active_model();
                    account.email_verified_at = Set(Some(now.into()));
                    user_account::Entity::update(account)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "identity redeem verification token account update failed");
                            Status::internal("internal server error")
                        })?;

                    let mut email_verification_token = existing_token.into_active_model();
                    email_verification_token.consumed_at = Set(Some(now.into()));
                    email_verification_token::Entity::update(email_verification_token)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "identity redeem verification token update failed");
                            Status::internal("internal server error")
                        })?;

                    let session = user_session::ActiveModel {
                        session_id: Set(session_id),
                        user_id: Set(user_id),
                        refresh_token_hash: Set(refresh_token_hash),
                        issued_at: Set(now.into()),
                        created_at: Set(now.into()),
                        refresh_expires_at: Set((now + chrono::Duration::days(7)).into()),
                        replaced_by_session_id: Set(None),
                        revoke_reason: Set(None),
                        revoked_at: Set(None),
                        client_instance_id: Set(None),
                    };
                    user_session::Entity::insert(session)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "identity redeem verification token session insert failed");
                            Status::internal("internal server error")
                        })?;

                    let event = outbox_event::ActiveModel {
                        event_id: Set(Uuid::new_v4()),
                        aggregate_type: Set("user_account".to_string()),
                        aggregate_id: Set(user_id),
                        event_type: Set("UserEmailVerified".to_string()),
                        status: Set("pending".to_string()),
                        created_at: Set(now.into()),
                        available_at: Set(now.into()),
                        occurred_at: Set(now.into()),
                        publish_attempts: Set(0),
                        published_at: Set(None),
                        last_error: Set(None),
                        claimed_by: Set(None),
                        claimed_at: Set(None),
                        payload: Set(payload_value(UserEmailVerifiedPayload {
                            user_id: user_id.to_string(),
                            email: email.clone(),
                            email_verified_at: now.to_rfc3339(),
                        })?),
                    };
                    outbox_event::Entity::insert(event).exec(txn).await.map_err(|e| {
                        error!(error = %e, "identity redeem verification token UserEmailVerified outbox insert failed");
                        Status::internal("internal server error")
                    })?;

                    Ok(Response::new(TokenPairResponse {
                        user_id: user_id.to_string(),
                        session_id: session_id.to_string(),
                        access_token,
                        access_token_expires_at: Some(to_timestamp(
                            now + Duration::from_std(ACCESS_TOKEN_VALIDITY)
                                .expect("access token validity should fit chrono"),
                        )),
                        refresh_token,
                        refresh_token_expires_at: Some(to_timestamp(now + Duration::days(7))),
                        email_verified: true,
                        profile: Some(relay_proto::identity::UserProfile {
                            user_id: profile.user_id.to_string(),
                            username: profile.username,
                            display_name: profile.display_name,
                            avatar_url: profile.avatar_url,
                        }),
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "identity redeem verification token transaction connection failure");
                    Status::internal("internal server error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}
