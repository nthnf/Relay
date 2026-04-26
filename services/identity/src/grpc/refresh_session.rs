use chrono::{Duration, Utc};
use relay_proto::identity::{RefreshSessionRequest, TokenPairResponse};
use sea_orm::{
    ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set, TransactionError, TransactionTrait,
};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::auth::{ACCESS_TOKEN_VALIDITY, hash_token};
use crate::entity::{user_account, user_profile, user_session};

use super::handler::{Handler, to_timestamp};

impl Handler {
    pub(super) async fn refresh_session(
        &self,
        request: Request<RefreshSessionRequest>,
    ) -> Result<Response<TokenPairResponse>, Status> {
        let RefreshSessionRequest { refresh_token, .. } = request.into_inner();

        let session = user_session::Entity::find()
            .filter(user_session::Column::RefreshTokenHash.eq(hash_token(&refresh_token)))
            .one(&self.connection)
            .await
            .map_err(|e| {
                error!(error = %e, "identity refresh session lookup failed");
                Status::internal("internal server error")
            })?
            .ok_or_else(|| Status::unauthenticated("invalid refresh token"))?;

        if session.revoked_at.is_some() {
            return Err(Status::unauthenticated("invalid refresh token"));
        }
        if session.refresh_expires_at < Utc::now() {
            return Err(Status::unauthenticated("invalid refresh token"));
        }

        let user = user_account::Entity::find_by_id(session.user_id)
            .one(&self.connection)
            .await
            .map_err(|e| {
                error!(error = %e, "identity refresh session user lookup failed");
                Status::internal("internal server error")
            })?
            .ok_or_else(|| Status::internal("internal server error"))?;

        if user.account_status != "active" {
            return Err(Status::failed_precondition("account is not active"));
        }

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
                error!(error = %e, "identity refresh session access token signing failed");
                Status::internal("internal server error")
            })?;
        let profile = user_profile::Entity::find_by_id(user.user_id)
            .one(&self.connection)
            .await
            .map_err(|e| {
                error!(error = %e, "identity refresh session user profile lookup failed");
                Status::internal("internal server error")
            })?
            .ok_or_else(|| Status::internal("internal server error"))?;

        let response = self
            .connection
            .transaction::<_, Response<TokenPairResponse>, Status>(|txn| {
                Box::pin(async move {
                    let mut session = session.into_active_model();
                    session.revoked_at = Set(Some(now.into()));
                    session.revoke_reason = Set(Some("rotated".to_string()));
                    session.replaced_by_session_id = Set(Some(session_id));
                    user_session::Entity::update(session)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "identity refresh session revoke failed");
                            Status::internal("internal server error")
                        })?;

                    let new_session = user_session::ActiveModel {
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
                    user_session::Entity::insert(new_session)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "identity refresh session insert failed");
                            Status::internal("internal server error")
                        })?;

                    let access_token_expires_at = now + Duration::from_std(ACCESS_TOKEN_VALIDITY)
                        .map_err(|e| {
                            error!(error = %e, "access token validity conversion failed");
                            Status::internal("internal server error")
                        })?;

                    Ok(Response::new(TokenPairResponse {
                        user_id: user.user_id.to_string(),
                        session_id: session_id.to_string(),
                        access_token,
                        access_token_expires_at: Some(to_timestamp(access_token_expires_at)),
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
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "identity refresh session transaction connection failure");
                    Status::internal("internal server error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}
