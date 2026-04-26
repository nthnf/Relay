use chrono::Utc;
use relay_proto::identity::{RevokeSessionRequest, RevokeSessionResponse};
use sea_orm::{EntityTrait, IntoActiveModel, Set, TransactionError, TransactionTrait};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::entity::{outbox_event, user_session};
use crate::events::SessionRevokedPayload;

use super::handler::{Handler, payload_value, to_timestamp};

impl Handler {
    pub(super) async fn revoke_session(
        &self,
        request: Request<RevokeSessionRequest>,
    ) -> Result<Response<RevokeSessionResponse>, Status> {
        let RevokeSessionRequest {
            session_id,
            revoke_reason,
        } = request.into_inner();

        let session_id = Uuid::parse_str(&session_id)
            .map_err(|_| Status::invalid_argument("invalid session_id"))?;

        let session = user_session::Entity::find_by_id(session_id)
            .one(&self.connection)
            .await
            .map_err(|e| {
                error!(error = %e, "identity revoke session lookup failed");
                Status::internal("internal server error")
            })?
            .ok_or_else(|| Status::unauthenticated("invalid session"))?;

        if let Some(revoked_at) = session.revoked_at {
            return Ok(Response::new(RevokeSessionResponse {
                revoked: true,
                revoked_at: Some(to_timestamp(revoked_at.into())),
            }));
        }

        let user_id = session.user_id;
        let revoked_at = Utc::now();
        let revoke_reason = revoke_reason.unwrap_or_else(|| "logout".to_string());

        let response = self
            .connection
            .transaction::<_, Response<RevokeSessionResponse>, Status>(|txn| {
                Box::pin(async move {
                    let mut session = session.into_active_model();
                    session.revoked_at = Set(Some(revoked_at.into()));
                    session.revoke_reason = Set(Some(revoke_reason.clone()));
                    user_session::Entity::update(session)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "identity revoke session update failed");
                            Status::internal("internal server error")
                        })?;

                    outbox_event::Entity::insert(outbox_event::ActiveModel {
                        event_id: Set(Uuid::new_v4()),
                        aggregate_type: Set("user_session".to_string()),
                        aggregate_id: Set(session_id),
                        event_type: Set("SessionRevoked".to_string()),
                        payload: Set(payload_value(SessionRevokedPayload {
                            session_id: session_id.to_string(),
                            user_id: user_id.to_string(),
                            revoked_at: revoked_at.to_rfc3339(),
                            revoke_reason: revoke_reason.clone(),
                        })?),
                        status: Set("pending".to_string()),
                        publish_attempts: Set(0),
                        occurred_at: Set(revoked_at.into()),
                        available_at: Set(revoked_at.into()),
                        claimed_by: Set(None),
                        claimed_at: Set(None),
                        published_at: Set(None),
                        last_error: Set(None),
                        created_at: Set(revoked_at.into()),
                    })
                    .exec(txn)
                    .await
                    .map_err(|e| {
                        error!(error = %e, "identity revoke session outbox insert failed");
                        Status::internal("internal server error")
                    })?;

                    Ok(Response::new(RevokeSessionResponse {
                        revoked: true,
                        revoked_at: Some(to_timestamp(revoked_at)),
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "identity revoke session transaction connection failure");
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

    fn test_service() -> Handler {
        Handler {
            connection: MockDatabase::new(DbBackend::Postgres).into_connection(),
            auth: AuthKeys::from_shared_secret(b"test-secret-key"),
        }
    }

    #[tokio::test]
    async fn revoke_session_rejects_invalid_session_id() {
        let error = test_service()
            .revoke_session(Request::new(relay_proto::identity::RevokeSessionRequest {
                session_id: "not-a-uuid".to_string(),
                revoke_reason: None,
            }))
            .await
            .expect_err("invalid session id should fail");

        assert_eq!(error.code(), tonic::Code::InvalidArgument);
        assert_eq!(error.message(), "invalid session_id");
    }
}
