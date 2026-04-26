use envoy_types::ext_authz::v3::pb::{CheckRequest, CheckResponse};
use envoy_types::ext_authz::v3::{CheckRequestExt, CheckResponseExt, OkHttpResponseBuilder};
use sea_orm::EntityTrait;
use tonic::{Request, Response, Status};
use tracing::error;

use crate::auth::{ACCESS_TOKEN_VALIDITY, TokenAuthError};
use crate::entity::{user_account, user_session};

use super::handler::Handler;

impl Handler {
    pub(super) async fn check(
        &self,
        request: Request<CheckRequest>,
    ) -> Result<Response<CheckResponse>, Status> {
        let request = request.into_inner();
        let headers = request.get_client_headers().cloned().unwrap_or_default();

        let access_token = headers.iter().find_map(|(key, value)| {
            key.eq_ignore_ascii_case("authorization").then(|| {
                value
                    .strip_prefix("Bearer ")
                    .or_else(|| value.strip_prefix("bearer "))
            })
        }).flatten();

        let Some(access_token) = access_token else {
            return Ok(Response::new(deny("missing bearer token")));
        };

        let claims = match self.auth.verify_access_token(access_token) {
            Ok(claims) => claims,
            Err(TokenAuthError::Jwt(err)) => {
                return Ok(Response::new(deny(&err.to_string())));
            }
        };

        let user = user_account::Entity::find_by_id(claims.user_id)
            .one(&self.connection)
            .await
            .map_err(|e| {
                error!(error = %e, "identity check user lookup failed");
                Status::internal("internal server error")
            })?;

        let Some(user) = user else {
            return Ok(Response::new(deny("unknown user")));
        };

        if user.account_status != "active" {
            return Ok(Response::new(deny("account is not active")));
        }

        let session = user_session::Entity::find_by_id(claims.session_id)
            .one(&self.connection)
            .await
            .map_err(|e| {
                error!(error = %e, "identity check session lookup failed");
                Status::internal("internal server error")
            })?;

        let Some(session) = session else {
            return Ok(Response::new(deny("unknown session")));
        };

        if session.user_id != user.user_id || session.revoked_at.is_some() {
            return Ok(Response::new(deny("session revoked")));
        }

        let access_token_expires_at = chrono::Utc::now()
            + chrono::Duration::from_std(ACCESS_TOKEN_VALIDITY).map_err(|e| {
                tracing::error!(error = %e, "access token validity conversion failed");
                tonic::Status::internal("internal server error")
            })?;

        let mut ok_response = OkHttpResponseBuilder::new();
        ok_response
            .add_header("x-user-id", user.user_id.to_string(), None, false)
            .add_header("x-session-id", session.session_id.to_string(), None, false)
            .add_header(
                "x-email-verified",
                user.email_verified_at.is_some().to_string(),
                None,
                false,
            )
            .add_header(
                "x-access-token-expires-at",
                access_token_expires_at.to_rfc3339(),
                None,
                false,
            );

        let mut response = CheckResponse::with_status(Status::ok("request is valid"));
        response.set_http_response(ok_response);

        Ok(Response::new(response))
    }
}

fn deny(message: &str) -> CheckResponse {
    CheckResponse::with_status(Status::unauthenticated(message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthKeys;
    use envoy_types::ext_authz::v3::pb::CheckRequest;
    use envoy_types::pb::envoy::service::auth::v3::{
        AttributeContext,
        attribute_context::{HttpRequest, Request as AuthRequest},
    };
    use sea_orm::{DbBackend, MockDatabase};
    use tonic::Request;

    fn test_service() -> Handler {
        Handler {
            connection: MockDatabase::new(DbBackend::Postgres).into_connection(),
            auth: AuthKeys::from_shared_secret(b"test-secret-key"),
        }
    }

    #[tokio::test]
    async fn check_rejects_missing_bearer_token() {
        let response = test_service()
            .check(Request::new(CheckRequest {
                attributes: Some(AttributeContext {
                    request: Some(AuthRequest {
                        http: Some(HttpRequest {
                            headers: std::collections::HashMap::new(),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
            }))
            .await
            .expect("missing token should be handled as a denied response")
            .into_inner();

        let status = response.status.as_ref().expect("status");
        assert_eq!(status.code, tonic::Code::Unauthenticated as i32);
        assert_eq!(status.message, "missing bearer token");
        assert!(response.http_response.is_none());
    }
}
