use chrono::{Duration, Utc};
use relay_proto::identity::identity_service_server::{IdentityService, IdentityServiceServer};
use relay_proto::identity::{
    AuthenticatePasswordRequest, GetUserProfileRequest, GetUserProfileResponse,
    GetUsersByIdsRequest, GetUsersByIdsResponse, RedeemEmailVerificationTokenRequest,
    RefreshSessionRequest, RegisterUserRequest, RegisterUserResponse,
    ResendVerificationEmailRequest, ResendVerificationEmailResponse, RevokeSessionRequest,
    RevokeSessionResponse, TokenPairResponse, UpdateUserProfileRequest, UpdateUserProfileResponse,
};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter, Set, SqlErr,
    TransactionError, TransactionTrait,
};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::auth::{ACCESS_TOKEN_VALIDITY, AuthKeys, hash_password, hash_token, verify_password};
use crate::entity::{
    email_verification_token, outbox_event, user_account, user_credential_password, user_profile,
    user_session,
};

const EMAIL_NORMALIZED_CONSTRAINT: &str = "uq-user-account-email-normalized";
const USERNAME_CONSTRAINT: &str = "uq-user-profile-username";

pub struct IdentityServer {
    connection: DatabaseConnection,
    auth: AuthKeys,
}

impl IdentityServer {
    pub fn new(connection: DatabaseConnection, auth: AuthKeys) -> Self {
        Self { connection, auth }
    }

    pub fn into_server(self) -> IdentityServiceServer<Self> {
        IdentityServiceServer::new(self)
    }

    fn unimplemented<T>(&self, name: &'static str) -> Result<Response<T>, Status> {
        let _ = (&self.connection, &self.auth);
        Err(Status::unimplemented(name))
    }
}

fn to_timestamp(dt: chrono::DateTime<Utc>) -> prost_types::Timestamp {
    prost_types::Timestamp {
        seconds: dt.timestamp(),
        nanos: dt.timestamp_subsec_nanos() as i32,
    }
}

#[tonic::async_trait]
impl IdentityService for IdentityServer {
    async fn register_user(
        &self,
        request: Request<RegisterUserRequest>,
    ) -> Result<Response<RegisterUserResponse>, Status> {
        let RegisterUserRequest {
            email,
            password,
            username,
            display_name,
            avatar_url,
        } = request.into_inner();

        let email_normalized = email.to_lowercase();

        let existing_user = user_account::Entity::find()
            .filter(user_account::Column::EmailNormalized.eq(&email_normalized))
            .one(&self.connection)
            .await
            .map_err(|e| {
                error!(error = %e, "identity registration lookup failed");
                Status::internal("internal server error")
            })?;

        if existing_user.is_some() {
            return Err(Status::already_exists(
                "A user with this email already exists",
            ));
        }

        let user_id = Uuid::new_v4();
        let now = Utc::now();
        let verification_token = Uuid::new_v4().to_string();
        let verification_token_hash = hash_token(&verification_token);
        let verification_token_id = Uuid::new_v4();

        let response = self
            .connection
            .transaction::<_, Response<RegisterUserResponse>, Status>(|txn| {
                Box::pin(async move {
                    let account = user_account::ActiveModel {
                        user_id: Set(user_id),
                        email: Set(email.clone()),
                        email_normalized: Set(email_normalized.clone()),
                        email_verified_at: Set(None),
                        account_status: Set("active".to_string()),
                        created_at: Set(now.into()),
                        updated_at: Set(now.into()),
                    };
                    user_account::Entity::insert(account)
                        .exec(txn)
                        .await
                        .map_err(|e| match e.sql_err() {
                            Some(SqlErr::UniqueConstraintViolation(message))
                                if message.contains(EMAIL_NORMALIZED_CONSTRAINT)
                                    || message.contains(USERNAME_CONSTRAINT) =>
                            {
                                Status::already_exists("email or username already exists")
                            }
                            _ => {
                                let message = e.to_string();
                                if message.contains(EMAIL_NORMALIZED_CONSTRAINT)
                                    || message.contains(USERNAME_CONSTRAINT)
                                {
                                    Status::already_exists("email or username already exists")
                                } else {
                                    error!(error = %e, "identity registration account insert failed");
                                    Status::internal("internal server error")
                                }
                            }
                        })?;

                    let profile = user_profile::ActiveModel {
                        user_id: Set(user_id),
                        username: Set(username.clone()),
                        display_name: Set(display_name.clone()),
                        avatar_url: Set(avatar_url.clone()),
                        created_at: Set(now.into()),
                        updated_at: Set(now.into()),
                    };
                    user_profile::Entity::insert(profile)
                        .exec(txn)
                        .await
                        .map_err(|e| match e.sql_err() {
                            Some(SqlErr::UniqueConstraintViolation(message))
                                if message.contains(EMAIL_NORMALIZED_CONSTRAINT)
                                    || message.contains(USERNAME_CONSTRAINT) =>
                            {
                                Status::already_exists("email or username already exists")
                            }
                            _ => {
                                let message = e.to_string();
                                if message.contains(EMAIL_NORMALIZED_CONSTRAINT)
                                    || message.contains(USERNAME_CONSTRAINT)
                                {
                                    Status::already_exists("email or username already exists")
                                } else {
                                    error!(error = %e, "identity registration profile insert failed");
                                    Status::internal("internal server error")
                                }
                            }
                        })?;

                    let credential = user_credential_password::ActiveModel {
                        user_id: Set(user_id),
                        password_hash: Set(hash_password(&password).map_err(|e| {
                            error!(error = %e, "identity password hashing failed");
                            Status::internal("internal server error")
                        })?),
                        password_updated_at: Set(now.into()),
                        failed_attempt_count: Set(0),
                        created_at: Set(now.into()),
                        updated_at: Set(now.into()),
                    };
                    user_credential_password::Entity::insert(credential)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "identity registration credential insert failed");
                            Status::internal("internal server error")
                        })?;

                    let verification_token_model = email_verification_token::ActiveModel {
                        token_id: Set(verification_token_id),
                        token_hash: Set(verification_token_hash.clone()),
                        user_id: Set(user_id),
                        created_at: Set(now.into()),
                        expires_at: Set((now + chrono::Duration::hours(6)).into()),
                        consumed_at: Set(None),
                    };
                    email_verification_token::Entity::insert(verification_token_model)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "identity registration verification token insert failed");
                            Status::internal("internal server error")
                        })?;

                    let user_registered_event = outbox_event::ActiveModel {
                        event_id: Set(Uuid::new_v4()),
                        aggregate_type: Set("user_account".to_string()),
                        aggregate_id: Set(user_id),
                        event_type: Set("UserRegistered".to_string()),
                        created_at: Set(now.into()),
                        available_at: Set(now.into()),
                        occurred_at: Set(now.into()),
                        claimed_at: Set(None),
                        claimed_by: Set(None),
                        published_at: Set(None),
                        last_error: Set(None),
                        publish_attempts: Set(0),
                        status: Set("pending".to_string()),
                        payload: Set(serde_json::json!({
                            "user_id": user_id.to_string(),
                            "email": email.clone(),
                            "email_verified": false,
                            "username": username.clone(),
                            "display_name": display_name.clone(),
                            "avatar_url": avatar_url.clone(),
                            "registered_at": now.to_rfc3339(),
                        })),
                    };
                    outbox_event::Entity::insert(user_registered_event)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "identity registration UserRegistered outbox insert failed");
                            Status::internal("internal server error")
                        })?;

                    let email_verification_request = outbox_event::ActiveModel {
                        event_id: Set(Uuid::new_v4()),
                        aggregate_type: Set("user_account".to_string()),
                        aggregate_id: Set(user_id),
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
                        payload: Set(serde_json::json!({
                            "user_id": user_id.to_string(),
                            "email": email.clone(),
                            "verification_token": verification_token.clone(),
                            "verification_token_expires_at": (now + chrono::Duration::hours(6)).to_rfc3339(),
                            "verification_token_id": verification_token_id.to_string(),
                            "reason": "registration",
                            "requested_at": now.to_rfc3339(),
                        })),
                    };
                    outbox_event::Entity::insert(email_verification_request)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "identity registration VerificationEmailRequested outbox insert failed");
                            Status::internal("internal server error")
                        })?;

                    Ok(Response::new(RegisterUserResponse {
                        user_id: user_id.to_string(),
                        email,
                        verification_email_requested_at: Some(to_timestamp(now)),
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "identity registration transaction connection failure");
                    Status::internal("internal server error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }

    async fn authenticate_password(
        &self,
        request: Request<AuthenticatePasswordRequest>,
    ) -> Result<Response<TokenPairResponse>, Status> {
        let AuthenticatePasswordRequest {
            email, password, ..
        } = request.into_inner();
        let email_normalized = email.to_lowercase();

        // Check if user exists and is eligible for authentication
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
            return Err(Status::unauthenticated("invalid refresh token"));
        }

        // Credential Verification
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

        // Token & Session
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

    async fn refresh_session(
        &self,
        request: Request<RefreshSessionRequest>,
    ) -> Result<Response<TokenPairResponse>, Status> {
        let RefreshSessionRequest { refresh_token, .. } = request.into_inner();

        // Check if refresh token exists and is eligible for refresh
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

        // Check if user is still eligible for authentication
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

        // Revoke current session and issue new tokens and session
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

        let response = self.connection
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

    async fn revoke_session(
        &self,
        request: Request<RevokeSessionRequest>,
    ) -> Result<Response<RevokeSessionResponse>, Status> {
        todo!(
            "implement session revocation by setting revoked_at and revoke_reason on the session record"
        );
    }

    async fn redeem_email_verification_token(
        &self,
        _request: Request<RedeemEmailVerificationTokenRequest>,
    ) -> Result<Response<TokenPairResponse>, Status> {
        self.unimplemented("redeem_email_verification_token")
    }

    async fn resend_verification_email(
        &self,
        _request: Request<ResendVerificationEmailRequest>,
    ) -> Result<Response<ResendVerificationEmailResponse>, Status> {
        self.unimplemented("resend_verification_email")
    }

    async fn update_user_profile(
        &self,
        _request: Request<UpdateUserProfileRequest>,
    ) -> Result<Response<UpdateUserProfileResponse>, Status> {
        self.unimplemented("update_user_profile")
    }

    async fn get_user_profile(
        &self,
        _request: Request<GetUserProfileRequest>,
    ) -> Result<Response<GetUserProfileResponse>, Status> {
        self.unimplemented("get_user_profile")
    }

    async fn get_users_by_ids(
        &self,
        _request: Request<GetUsersByIdsRequest>,
    ) -> Result<Response<GetUsersByIdsResponse>, Status> {
        self.unimplemented("get_users_by_ids")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DbBackend, DbErr, MockDatabase, MockExecResult};

    fn test_service(db: DatabaseConnection) -> IdentityServer {
        IdentityServer::new(db, AuthKeys::from_shared_secret(b"test-secret-key"))
    }

    fn register_request() -> Request<RegisterUserRequest> {
        Request::new(RegisterUserRequest {
            email: "alice@example.com".to_string(),
            password: "plain-password".to_string(),
            username: "alice".to_string(),
            display_name: "Alice".to_string(),
            avatar_url: Some("https://cdn.example.com/alice.png".to_string()),
        })
    }

    fn mock_exec_result() -> MockExecResult {
        MockExecResult {
            last_insert_id: 0,
            rows_affected: 1,
        }
    }

    fn authenticate_request() -> Request<AuthenticatePasswordRequest> {
        Request::new(AuthenticatePasswordRequest {
            email: "Alice@Example.com".to_string(),
            password: "plain-password".to_string(),
            client_instance_id: None,
        })
    }

    fn refresh_request(token: &str) -> Request<RefreshSessionRequest> {
        Request::new(RefreshSessionRequest {
            refresh_token: token.to_string(),
            client_instance_id: None,
        })
    }

    fn mock_user_account(now: chrono::DateTime<Utc>) -> user_account::Model {
        user_account::Model {
            user_id: Uuid::new_v4(),
            email: "alice@example.com".to_string(),
            email_normalized: "alice@example.com".to_string(),
            email_verified_at: None,
            account_status: "active".to_string(),
            created_at: now.into(),
            updated_at: now.into(),
        }
    }

    fn mock_verified_user_account(now: chrono::DateTime<Utc>) -> user_account::Model {
        user_account::Model {
            email_verified_at: Some(now.into()),
            ..mock_user_account(now)
        }
    }

    fn mock_refresh_session(
        now: chrono::DateTime<Utc>,
        user_id: Uuid,
        refresh_token_hash: String,
    ) -> user_session::Model {
        user_session::Model {
            session_id: Uuid::new_v4(),
            user_id,
            refresh_token_hash,
            issued_at: now.into(),
            refresh_expires_at: (now + Duration::days(7)).into(),
            revoked_at: None,
            revoke_reason: None,
            replaced_by_session_id: None,
            client_instance_id: None,
            created_at: now.into(),
        }
    }

    fn mock_user_profile(now: chrono::DateTime<Utc>) -> user_profile::Model {
        user_profile::Model {
            user_id: Uuid::new_v4(),
            username: "alice".to_string(),
            display_name: "Alice".to_string(),
            avatar_url: Some("https://cdn.example.com/alice.png".to_string()),
            created_at: now.into(),
            updated_at: now.into(),
        }
    }

    fn mock_user_credential_password(
        now: chrono::DateTime<Utc>,
    ) -> user_credential_password::Model {
        user_credential_password::Model {
            user_id: Uuid::new_v4(),
            password_hash: "$argon2id$v=19$m=19456,t=2,p=1$mock$mock".to_string(),
            password_updated_at: now.into(),
            failed_attempt_count: 0,
            created_at: now.into(),
            updated_at: now.into(),
        }
    }

    fn mock_email_verification_token(
        now: chrono::DateTime<Utc>,
    ) -> email_verification_token::Model {
        email_verification_token::Model {
            token_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_hash: "mock-token-hash".to_string(),
            expires_at: now.into(),
            consumed_at: None,
            created_at: now.into(),
        }
    }

    fn mock_outbox_event(now: chrono::DateTime<Utc>, event_type: &str) -> outbox_event::Model {
        outbox_event::Model {
            event_id: Uuid::new_v4(),
            aggregate_type: "user_account".to_string(),
            aggregate_id: Uuid::new_v4(),
            event_type: event_type.to_string(),
            payload: serde_json::json!({ "event_type": event_type }),
            status: "pending".to_string(),
            publish_attempts: 0,
            occurred_at: now.into(),
            available_at: now.into(),
            claimed_by: None,
            claimed_at: None,
            published_at: None,
            last_error: None,
            created_at: now.into(),
        }
    }

    #[tokio::test]
    async fn register_user_returns_already_exists_for_duplicate_email() {
        let now = Utc::now();
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([[mock_user_account(now)]])
            .into_connection();

        let error = test_service(db)
            .register_user(register_request())
            .await
            .expect_err("duplicate email should be rejected");

        assert_eq!(error.code(), tonic::Code::AlreadyExists);
        assert_eq!(error.message(), "A user with this email already exists");
    }

    #[tokio::test]
    async fn register_user_returns_already_exists_for_duplicate_username_constraint() {
        let now = Utc::now();
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([Vec::<user_account::Model>::new()])
            .append_query_results([[mock_user_account(now)]])
            .append_query_errors([DbErr::Custom(format!(
                "Unique Constraint Violation: {USERNAME_CONSTRAINT}"
            ))])
            .append_exec_results([mock_exec_result(), mock_exec_result()])
            .into_connection();

        let error = test_service(db)
            .register_user(register_request())
            .await
            .expect_err("duplicate username should be rejected");

        assert_eq!(error.code(), tonic::Code::AlreadyExists);
        assert_eq!(error.message(), "email or username already exists");
    }

    #[tokio::test]
    async fn register_user_writes_two_outbox_events_and_returns_confirmation() {
        let now = Utc::now();
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([Vec::<user_account::Model>::new()])
            .append_query_results([[mock_user_account(now)]])
            .append_query_results([[mock_user_profile(now)]])
            .append_query_results([[mock_user_credential_password(now)]])
            .append_query_results([[mock_email_verification_token(now)]])
            .append_query_results([[mock_outbox_event(now, "UserRegistered")]])
            .append_query_results([[mock_outbox_event(now, "VerificationEmailRequested")]])
            .append_exec_results([
                mock_exec_result(),
                mock_exec_result(),
                mock_exec_result(),
                mock_exec_result(),
                mock_exec_result(),
                mock_exec_result(),
            ])
            .into_connection();

        let service = test_service(db.clone());
        let response = service
            .register_user(register_request())
            .await
            .expect("registration should succeed")
            .into_inner();

        assert_eq!(response.email, "alice@example.com");
        assert!(!response.user_id.is_empty());
        assert!(response.verification_email_requested_at.is_some());

        let transaction_log = db.into_transaction_log();
        assert_eq!(
            transaction_log.len(),
            2,
            "registration should log the precheck and one transaction"
        );

        let statement_dump = transaction_log
            .iter()
            .flat_map(|transaction| transaction.statements().iter())
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(statement_dump.contains("UserRegistered"));
        assert!(statement_dump.contains("VerificationEmailRequested"));
        assert!(statement_dump.contains("user_account"));
        assert!(statement_dump.contains("verification_token"));
    }

    #[tokio::test]
    async fn authenticate_password_returns_tokens_and_creates_session() {
        let now = Utc::now();
        let user = mock_verified_user_account(now);
        let password_hash = hash_password("plain-password").expect("hashing should succeed");
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([[user.clone()]])
            .append_query_results([[user_credential_password::Model {
                user_id: user.user_id,
                password_hash,
                password_updated_at: now.into(),
                failed_attempt_count: 0,
                created_at: now.into(),
                updated_at: now.into(),
            }]])
            .append_query_results([[user_profile::Model {
                user_id: user.user_id,
                username: "alice".to_string(),
                display_name: "Alice".to_string(),
                avatar_url: Some("https://cdn.example.com/alice.png".to_string()),
                created_at: now.into(),
                updated_at: now.into(),
            }]])
            .append_query_results([[user_session::Model {
                session_id: Uuid::new_v4(),
                user_id: user.user_id,
                refresh_token_hash: "mock-refresh-hash".to_string(),
                issued_at: now.into(),
                refresh_expires_at: (now + Duration::days(7)).into(),
                revoked_at: None,
                revoke_reason: None,
                replaced_by_session_id: None,
                client_instance_id: None,
                created_at: now.into(),
            }]])
            .append_exec_results([mock_exec_result()])
            .into_connection();

        let response = test_service(db.clone())
            .authenticate_password(authenticate_request())
            .await
            .expect("authentication should succeed")
            .into_inner();

        assert_eq!(response.user_id, user.user_id.to_string());
        assert!(!response.session_id.is_empty());
        assert!(!response.access_token.is_empty());
        assert!(!response.refresh_token.is_empty());
        assert!(response.access_token_expires_at.is_some());
        assert!(response.refresh_token_expires_at.is_some());
        assert!(response.email_verified);
        assert_eq!(
            response.profile.as_ref().map(|p| p.username.as_str()),
            Some("alice")
        );

        let statement_dump = db
            .into_transaction_log()
            .iter()
            .flat_map(|transaction| transaction.statements().iter())
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(statement_dump.contains("email_normalized"));
        assert!(statement_dump.contains("INSERT INTO \"user_session\""));
    }

    #[tokio::test]
    async fn authenticate_password_rejects_invalid_credentials() {
        let now = Utc::now();
        let user = mock_verified_user_account(now);
        let password_hash = hash_password("different-password").expect("hashing should succeed");
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([[user]])
            .append_query_results([[user_credential_password::Model {
                user_id: Uuid::new_v4(),
                password_hash,
                password_updated_at: now.into(),
                failed_attempt_count: 0,
                created_at: now.into(),
                updated_at: now.into(),
            }]])
            .into_connection();

        let error = test_service(db.clone())
            .authenticate_password(authenticate_request())
            .await
            .expect_err("authentication should reject invalid password");

        assert_eq!(error.code(), tonic::Code::Unauthenticated);
        assert_eq!(error.message(), "invalid credentials");

        let statement_dump = db
            .into_transaction_log()
            .iter()
            .flat_map(|transaction| transaction.statements().iter())
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(!statement_dump.contains("INSERT INTO \"user_session\""));
    }

    #[tokio::test]
    async fn refresh_session_rotates_session_and_returns_new_tokens() {
        let now = Utc::now();
        let user = mock_verified_user_account(now);
        let refresh_token = "refresh-token-value";
        let old_session = mock_refresh_session(now, user.user_id, hash_token(refresh_token));
        let profile = user_profile::Model {
            user_id: user.user_id,
            username: "alice".to_string(),
            display_name: "Alice".to_string(),
            avatar_url: Some("https://cdn.example.com/alice.png".to_string()),
            created_at: now.into(),
            updated_at: now.into(),
        };
        let rotated_session = user_session::Model {
            revoked_at: Some(now.into()),
            revoke_reason: Some("rotated".to_string()),
            replaced_by_session_id: Some(Uuid::new_v4()),
            ..old_session.clone()
        };
        let inserted_session =
            mock_refresh_session(now, user.user_id, "new-refresh-hash".to_string());

        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([[old_session.clone()]])
            .append_query_results([[user.clone()]])
            .append_query_results([[profile.clone()]])
            .append_query_results([[rotated_session]])
            .append_query_results([[inserted_session]])
            .append_exec_results([mock_exec_result(), mock_exec_result()])
            .into_connection();

        let service = test_service(db.clone());
        let response = service
            .refresh_session(refresh_request(refresh_token))
            .await
            .expect("refresh should succeed")
            .into_inner();

        assert_eq!(response.user_id, user.user_id.to_string());
        assert_ne!(response.session_id, old_session.session_id.to_string());
        assert!(!response.access_token.is_empty());
        assert!(!response.refresh_token.is_empty());
        assert!(response.access_token_expires_at.is_some());
        assert!(response.refresh_token_expires_at.is_some());
        assert!(response.email_verified);
        assert_eq!(
            response.profile.as_ref().map(|p| p.username.as_str()),
            Some("alice")
        );

        let claims = service
            .auth
            .verify_access_token(&response.access_token)
            .expect("access token should verify");
        assert_eq!(claims.user_id, user.user_id);
        assert_eq!(claims.session_id.to_string(), response.session_id);

        let statement_dump = db
            .into_transaction_log()
            .iter()
            .flat_map(|transaction| transaction.statements().iter())
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(statement_dump.contains("UPDATE \"user_session\""));
        assert!(statement_dump.contains("rotated"));
        assert!(statement_dump.contains("INSERT INTO \"user_session\""));
    }

    #[tokio::test]
    async fn refresh_session_rejects_invalid_refresh_token() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([Vec::<user_session::Model>::new()])
            .into_connection();

        let error = test_service(db)
            .refresh_session(refresh_request("missing-refresh-token"))
            .await
            .expect_err("missing refresh token should fail");

        assert_eq!(error.code(), tonic::Code::Unauthenticated);
        assert_eq!(error.message(), "invalid refresh token");
    }

    #[tokio::test]
    async fn refresh_session_rejects_revoked_refresh_token() {
        let now = Utc::now();
        let refresh_token = "revoked-refresh-token";
        let session = user_session::Model {
            revoked_at: Some(now.into()),
            revoke_reason: Some("rotated".to_string()),
            ..mock_refresh_session(now, Uuid::new_v4(), hash_token(refresh_token))
        };
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([[session]])
            .into_connection();

        let error = test_service(db.clone())
            .refresh_session(refresh_request(refresh_token))
            .await
            .expect_err("revoked refresh token should fail");

        assert_eq!(error.code(), tonic::Code::Unauthenticated);
        assert_eq!(error.message(), "invalid refresh token");

        let statement_dump = db
            .into_transaction_log()
            .iter()
            .flat_map(|transaction| transaction.statements().iter())
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(!statement_dump.contains("UPDATE \"user_session\""));
        assert!(!statement_dump.contains("INSERT INTO \"user_session\""));
    }

    #[tokio::test]
    async fn refresh_session_rejects_expired_refresh_token() {
        let now = Utc::now();
        let refresh_token = "expired-refresh-token";
        let session = user_session::Model {
            refresh_expires_at: (now - Duration::seconds(1)).into(),
            ..mock_refresh_session(now, Uuid::new_v4(), hash_token(refresh_token))
        };
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([[session]])
            .into_connection();

        let error = test_service(db.clone())
            .refresh_session(refresh_request(refresh_token))
            .await
            .expect_err("expired refresh token should fail");

        assert_eq!(error.code(), tonic::Code::Unauthenticated);
        assert_eq!(error.message(), "invalid refresh token");

        let statement_dump = db
            .into_transaction_log()
            .iter()
            .flat_map(|transaction| transaction.statements().iter())
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(!statement_dump.contains("UPDATE \"user_session\""));
        assert!(!statement_dump.contains("INSERT INTO \"user_session\""));
    }
}
