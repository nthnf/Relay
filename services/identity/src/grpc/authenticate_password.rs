use chrono::{Duration, Utc};
use relay_proto::identity::{AuthenticatePasswordRequest, TokenPairResponse};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set, TransactionError, TransactionTrait};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::auth::{ACCESS_TOKEN_VALIDITY, hash_token, verify_password};
use crate::entity::{user_account, user_profile, user_credential_password, user_session};

use super::handler::{Handler, to_timestamp};

impl Handler {
    pub(crate) async fn authenticate_password(
        &self,
        request: Request<AuthenticatePasswordRequest>,
    ) -> Result<Response<TokenPairResponse>, Status> {
        let AuthenticatePasswordRequest {
            email, password, ..
        } = request.into_inner();
        let email_normalized = email.to_lowercase();

        let user = user_account::Entity::find()
            .filter(user_account::Column::EmailNormalized.eq(email_normalized))
            .one(&self.connection)
            .await
            .map_err(|e| {
                error!(error = %e, "identity authentication lookup failed");
                Status::internal("internal server error")
            })?;

        let user = user.ok_or_else(|| Status::unauthenticated("invalid credentials"))?;

        if user.email_verified_at.is_none() {
            return Err(Status::failed_precondition("email not verified"));
        }

        if user.account_status != "active" {
            return Err(Status::failed_precondition("account is not active"));
        }

        let hashed_password = user_credential_password::Entity::find_by_id(user.user_id)
            .one(&self.connection)
            .await
            .map_err(|e| {
                error!(error = %e, "identity authentication credential lookup failed");
                Status::internal("internal server error")
            })?
            .ok_or_else(|| Status::unauthenticated("invalid credentials"))?
            .password_hash;

        verify_password(&password, &hashed_password)
            .map_err(|e| {
                error!(error = %e, "identity password verification failed");
                Status::internal("internal server error")
            })
            .and_then(|is_valid| {
                if is_valid {
                    Ok(())
                } else {
                    Err(Status::unauthenticated("invalid credentials"))
                }
            })?;

        let session_id = Uuid::new_v4();
        let refresh_token = Uuid::new_v4().to_string();
        let refresh_token_hash = hash_token(&refresh_token);
        let now = Utc::now();

        let access_token = self
            .auth
            .sign_access_token(crate::auth::AccessClaims {
                user_id: user.user_id,
                session_id,
            })
            .map_err(|e| {
                error!(error = %e, "identity access token signing failed");
                Status::internal("internal server error")
            })?;

        let profile = self
            .connection
            .transaction::<_, user_profile::Model, Status>(|txn| {
                Box::pin(async move {
                    let profile = user_profile::Entity::find_by_id(user.user_id)
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "identity user profile lookup failed");
                            Status::internal("internal server error")
                        })?
                        .ok_or_else(|| Status::internal("internal server error"))?;

                    let session = user_session::ActiveModel {
                        session_id: Set(session_id),
                        user_id: Set(user.user_id),
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
                            error!(error = %e, "identity session insert failed");
                            Status::internal("internal server error")
                        })?;

                    Ok(profile)
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "identity authentication transaction connection failure");
                    Status::internal("internal server error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(Response::new(TokenPairResponse {
            user_id: user.user_id.to_string(),
            session_id: session_id.to_string(),
            access_token,
            access_token_expires_at: Some(to_timestamp(
                now + Duration::from_std(ACCESS_TOKEN_VALIDITY)
                    .expect("access token validity should fit chrono"),
            )),
            refresh_token,
            refresh_token_expires_at: Some(to_timestamp(now + Duration::days(7))),
            email_verified: user.email_verified_at.is_some(),
            profile: Some(relay_proto::identity::UserProfile {
                user_id: profile.user_id.to_string(),
                username: profile.username,
                display_name: profile.display_name,
                avatar_url: profile.avatar_url,
            }),
        }))
    }
}
