use chrono::{Duration, Utc};
use envoy_types::ext_authz::v3::pb::{CheckRequest, CheckResponse, HttpResponse};
use envoy_types::pb::envoy::service::auth::v3::{
    AttributeContext,
    attribute_context::{HttpRequest, Request as AuthRequest},
};
use relay_proto::identity::{
    AuthenticatePasswordRequest, GetUserProfileRequest, GetUsersByIdsRequest,
    RedeemEmailVerificationTokenRequest, RefreshSessionRequest, RegisterUserRequest,
    ResendVerificationEmailRequest, RevokeSessionRequest, UpdateUserProfileRequest,
};
use sea_orm::{DatabaseConnection, DbBackend, DbErr, MockDatabase, MockExecResult};
use tonic::Request;
use uuid::Uuid;

use crate::auth::{hash_password, hash_token};
use crate::entity::{
    email_verification_token, outbox_event, user_account, user_credential_password, user_profile,
    user_session,
};

use relay_types::ACTOR_USER_ID_METADATA;

use super::handler::Handler;

fn test_service(db: DatabaseConnection) -> Handler {
    Handler::new(
        db,
        crate::auth::AuthKeys::from_shared_secret(b"test-secret-key"),
    )
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

fn revoke_request(session_id: &str, revoke_reason: Option<&str>) -> Request<RevokeSessionRequest> {
    Request::new(RevokeSessionRequest {
        session_id: session_id.to_string(),
        revoke_reason: revoke_reason.map(str::to_string),
    })
}

fn redeem_request(token: &str) -> Request<RedeemEmailVerificationTokenRequest> {
    Request::new(RedeemEmailVerificationTokenRequest {
        token: token.to_string(),
    })
}

fn resend_request(email: &str) -> Request<ResendVerificationEmailRequest> {
    Request::new(ResendVerificationEmailRequest {
        email: email.to_string(),
    })
}

fn update_profile_request(
    user_id: Option<Uuid>,
    display_name: &str,
    avatar_url: Option<&str>,
) -> Request<UpdateUserProfileRequest> {
    let mut request = Request::new(UpdateUserProfileRequest {
        display_name: display_name.to_string(),
        avatar_url: avatar_url.map(str::to_string),
    });

    if let Some(user_id) = user_id {
        request.metadata_mut().insert(
            ACTOR_USER_ID_METADATA,
            user_id
                .to_string()
                .parse()
                .expect("user id metadata should be valid"),
        );
    }

    request
}

fn get_user_profile_request(
    actor_user_id: Option<Uuid>,
    user_id: Option<Uuid>,
) -> Request<GetUserProfileRequest> {
    let mut request = Request::new(GetUserProfileRequest {
        user_id: user_id.map(|user_id| user_id.to_string()),
    });

    if let Some(actor_user_id) = actor_user_id {
        request.metadata_mut().insert(
            ACTOR_USER_ID_METADATA,
            actor_user_id
                .to_string()
                .parse()
                .expect("user id metadata should be valid"),
        );
    }

    request
}

fn get_users_by_ids_request(user_ids: &[Uuid]) -> Request<GetUsersByIdsRequest> {
    Request::new(GetUsersByIdsRequest {
        user_ids: user_ids.iter().map(ToString::to_string).collect(),
    })
}

fn check_request(token: &str) -> Request<CheckRequest> {
    Request::new(CheckRequest {
        attributes: Some(AttributeContext {
            request: Some(AuthRequest {
                http: Some(HttpRequest {
                    headers: std::iter::once((
                        "authorization".to_string(),
                        format!("Bearer {token}"),
                    ))
                    .collect(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }),
    })
}

fn access_token(user_id: Uuid, session_id: Uuid) -> String {
    crate::auth::AuthKeys::from_shared_secret(b"test-secret-key")
        .sign_access_token(crate::auth::AccessClaims {
            user_id,
            session_id,
        })
        .expect("token signing should succeed")
}

fn assert_denied_check(response: &CheckResponse, message: &str) {
    let status = response
        .status
        .as_ref()
        .expect("ext_authz response should include a status");

    assert_eq!(status.code, tonic::Code::Unauthenticated as i32);
    assert_eq!(status.message, message);
    assert!(response.http_response.is_none());
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

fn mock_user_credential_password(now: chrono::DateTime<Utc>) -> user_credential_password::Model {
    user_credential_password::Model {
        user_id: Uuid::new_v4(),
        password_hash: "$argon2id$v=19$m=19456,t=2,p=1$mock$mock".to_string(),
        password_updated_at: now.into(),
        failed_attempt_count: 0,
        created_at: now.into(),
        updated_at: now.into(),
    }
}

fn mock_email_verification_token(now: chrono::DateTime<Utc>) -> email_verification_token::Model {
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
            "Unique Constraint Violation: {username_constraint}",
            username_constraint = "uq-user-profile-username"
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
async fn check_accepts_active_session() {
    let now = Utc::now();
    let user = mock_verified_user_account(now);
    let expected_user_id = user.user_id.to_string();
    let session_id = Uuid::new_v4();
    let session = user_session::Model {
        session_id,
        user_id: user.user_id,
        refresh_token_hash: hash_token("refresh-token"),
        issued_at: now.into(),
        refresh_expires_at: (now + Duration::days(7)).into(),
        revoked_at: None,
        revoke_reason: None,
        replaced_by_session_id: None,
        client_instance_id: None,
        created_at: now.into(),
    };

    let token = access_token(user.user_id, session_id);

    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([[user]])
        .append_query_results([[session]])
        .into_connection();

    let response = test_service(db)
        .check(check_request(&token))
        .await
        .expect("token should validate")
        .into_inner();
    let expected_session_id = session_id.to_string();
    let status = response
        .status
        .clone()
        .expect("ext_authz response should include a status");
    let ok_response = match response.http_response {
        Some(HttpResponse::OkResponse(ok_response)) => ok_response,
        other => panic!("expected ok http response, got {other:?}"),
    };

    let headers = ok_response
        .headers
        .iter()
        .filter_map(|header| {
            header
                .header
                .as_ref()
                .map(|value| (value.key.as_str(), value.value.as_str()))
        })
        .collect::<std::collections::HashMap<_, _>>();

    assert_eq!(status.code, tonic::Code::Ok as i32);
    assert_eq!(status.message, "request is valid");
    assert_eq!(
        headers.get("x-user-id").copied(),
        Some(expected_user_id.as_str())
    );
    assert_eq!(
        headers.get("x-session-id").copied(),
        Some(expected_session_id.as_str())
    );
}

#[tokio::test]
async fn check_rejects_missing_bearer_token() {
    let response = test_service(MockDatabase::new(DbBackend::Postgres).into_connection())
        .check(Request::new(CheckRequest::default()))
        .await
        .expect("missing token should be handled as a denied response")
        .into_inner();

    assert_denied_check(&response, "missing bearer token");
}

#[tokio::test]
async fn check_rejects_unknown_user() {
    let token = access_token(Uuid::new_v4(), Uuid::new_v4());
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([Vec::<user_account::Model>::new()])
        .into_connection();

    let response = test_service(db)
        .check(check_request(&token))
        .await
        .expect("unknown user should be handled as a denied response")
        .into_inner();

    assert_denied_check(&response, "unknown user");
}

#[tokio::test]
async fn check_rejects_inactive_account() {
    let now = Utc::now();
    let user = user_account::Model {
        account_status: "disabled".to_string(),
        ..mock_verified_user_account(now)
    };
    let token = access_token(user.user_id, Uuid::new_v4());
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([[user]])
        .into_connection();

    let response = test_service(db)
        .check(check_request(&token))
        .await
        .expect("inactive account should be handled as a denied response")
        .into_inner();

    assert_denied_check(&response, "account is not active");
}

#[tokio::test]
async fn check_rejects_unknown_session() {
    let now = Utc::now();
    let user = mock_verified_user_account(now);
    let session_id = Uuid::new_v4();
    let token = access_token(user.user_id, session_id);
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([[user]])
        .append_query_results([Vec::<user_session::Model>::new()])
        .into_connection();

    let response = test_service(db)
        .check(check_request(&token))
        .await
        .expect("unknown session should be handled as a denied response")
        .into_inner();

    assert_denied_check(&response, "unknown session");
}

#[tokio::test]
async fn check_rejects_revoked_session() {
    let now = Utc::now();
    let user = mock_verified_user_account(now);
    let session_id = Uuid::new_v4();
    let token = access_token(user.user_id, session_id);
    let session = user_session::Model {
        session_id,
        user_id: user.user_id,
        refresh_token_hash: hash_token("refresh-token"),
        issued_at: now.into(),
        refresh_expires_at: (now + Duration::days(7)).into(),
        revoked_at: Some(now.into()),
        revoke_reason: Some("logout".to_string()),
        replaced_by_session_id: None,
        client_instance_id: None,
        created_at: now.into(),
    };
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([[user]])
        .append_query_results([[session]])
        .into_connection();

    let response = test_service(db)
        .check(check_request(&token))
        .await
        .expect("revoked session should be handled as a denied response")
        .into_inner();

    assert_denied_check(&response, "session revoked");
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
    let inserted_session = mock_refresh_session(now, user.user_id, "new-refresh-hash".to_string());

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

#[tokio::test]
async fn revoke_session_updates_session_and_writes_outbox() {
    let now = Utc::now();
    let session = mock_refresh_session(now, Uuid::new_v4(), "refresh-hash".to_string());
    let updated_session = user_session::Model {
        revoked_at: Some(now.into()),
        revoke_reason: Some("logout".to_string()),
        ..session.clone()
    };
    let outbox = mock_outbox_event(now, "SessionRevoked");
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([[session.clone()]])
        .append_query_results([[updated_session]])
        .append_query_results([[outbox]])
        .append_exec_results([mock_exec_result(), mock_exec_result()])
        .into_connection();

    let response = test_service(db.clone())
        .revoke_session(revoke_request(&session.session_id.to_string(), None))
        .await
        .expect("revoke should succeed")
        .into_inner();

    assert!(response.revoked);
    assert!(response.revoked_at.is_some());

    let statement_dump = db
        .into_transaction_log()
        .iter()
        .flat_map(|transaction| transaction.statements().iter())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(statement_dump.contains("UPDATE \"user_session\""));
    assert!(statement_dump.contains("SessionRevoked"));
    assert!(statement_dump.contains("logout"));
}

#[tokio::test]
async fn revoke_session_is_idempotent_for_already_revoked_session() {
    let now = Utc::now();
    let session = user_session::Model {
        revoked_at: Some(now.into()),
        revoke_reason: Some("logout".to_string()),
        ..mock_refresh_session(now, Uuid::new_v4(), "refresh-hash".to_string())
    };
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([[session.clone()]])
        .into_connection();

    let response = test_service(db.clone())
        .revoke_session(revoke_request(
            &session.session_id.to_string(),
            Some("logout"),
        ))
        .await
        .expect("already revoked session should still succeed")
        .into_inner();

    assert!(response.revoked);
    assert!(response.revoked_at.is_some());

    let statement_dump = db
        .into_transaction_log()
        .iter()
        .flat_map(|transaction| transaction.statements().iter())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(!statement_dump.contains("UPDATE \"user_session\""));
    assert!(!statement_dump.contains("SessionRevoked"));
}

#[tokio::test]
async fn revoke_session_rejects_invalid_session_id() {
    let db = MockDatabase::new(DbBackend::Postgres).into_connection();

    let error = test_service(db)
        .revoke_session(revoke_request("not-a-uuid", None))
        .await
        .expect_err("invalid session id should fail");

    assert_eq!(error.code(), tonic::Code::InvalidArgument);
    assert_eq!(error.message(), "invalid session_id");
}

#[tokio::test]
async fn redeem_email_verification_token_creates_first_session_and_outbox() {
    let now = Utc::now();
    let user = mock_user_account(now);
    let token = "verification-token-value";
    let existing_token = email_verification_token::Model {
        token_id: Uuid::new_v4(),
        user_id: user.user_id,
        token_hash: hash_token(token),
        expires_at: (now + Duration::hours(1)).into(),
        consumed_at: None,
        created_at: now.into(),
    };
    let updated_account = user_account::Model {
        email_verified_at: Some(now.into()),
        ..user.clone()
    };
    let updated_token = email_verification_token::Model {
        consumed_at: Some(now.into()),
        ..existing_token.clone()
    };
    let profile = user_profile::Model {
        user_id: user.user_id,
        username: "alice".to_string(),
        display_name: "Alice".to_string(),
        avatar_url: Some("https://cdn.example.com/alice.png".to_string()),
        created_at: now.into(),
        updated_at: now.into(),
    };
    let inserted_session = mock_refresh_session(now, user.user_id, "new-refresh-hash".to_string());
    let outbox = mock_outbox_event(now, "UserEmailVerified");

    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([[existing_token]])
        .append_query_results([[user.clone()]])
        .append_query_results([[profile.clone()]])
        .append_query_results([[updated_account]])
        .append_query_results([[updated_token]])
        .append_query_results([[inserted_session]])
        .append_query_results([[outbox]])
        .append_exec_results([
            mock_exec_result(),
            mock_exec_result(),
            mock_exec_result(),
            mock_exec_result(),
        ])
        .into_connection();

    let service = test_service(db.clone());
    let response = service
        .redeem_email_verification_token(redeem_request(token))
        .await
        .expect("redeem should succeed")
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

    assert!(statement_dump.contains("UPDATE \"user_account\""));
    assert!(statement_dump.contains("UPDATE \"email_verification_token\""));
    assert!(statement_dump.contains("INSERT INTO \"user_session\""));
    assert!(statement_dump.contains("UserEmailVerified"));
}

#[tokio::test]
async fn redeem_email_verification_token_rejects_unknown_token() {
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([Vec::<email_verification_token::Model>::new()])
        .into_connection();

    let error = test_service(db)
        .redeem_email_verification_token(redeem_request("missing-token"))
        .await
        .expect_err("unknown token should fail");

    assert_eq!(error.code(), tonic::Code::NotFound);
    assert_eq!(error.message(), "invalid verification token");
}

#[tokio::test]
async fn redeem_email_verification_token_rejects_consumed_token() {
    let now = Utc::now();
    let token = "consumed-token";
    let existing_token = email_verification_token::Model {
        token_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        token_hash: hash_token(token),
        expires_at: (now + Duration::hours(1)).into(),
        consumed_at: Some(now.into()),
        created_at: now.into(),
    };
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([[existing_token]])
        .into_connection();

    let error = test_service(db)
        .redeem_email_verification_token(redeem_request(token))
        .await
        .expect_err("consumed token should fail");

    assert_eq!(error.code(), tonic::Code::NotFound);
    assert_eq!(error.message(), "invalid verification token");
}

#[tokio::test]
async fn redeem_email_verification_token_rejects_expired_token() {
    let now = Utc::now();
    let token = "expired-token";
    let existing_token = email_verification_token::Model {
        token_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        token_hash: hash_token(token),
        expires_at: (now - Duration::seconds(1)).into(),
        consumed_at: None,
        created_at: now.into(),
    };
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([[existing_token]])
        .into_connection();

    let error = test_service(db)
        .redeem_email_verification_token(redeem_request(token))
        .await
        .expect_err("expired token should fail");

    assert_eq!(error.code(), tonic::Code::NotFound);
    assert_eq!(error.message(), "invalid verification token");
}

#[tokio::test]
async fn resend_verification_email_accepts_unknown_email_without_side_effects() {
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([Vec::<user_account::Model>::new()])
        .into_connection();

    let response = test_service(db.clone())
        .resend_verification_email(resend_request("missing@example.com"))
        .await
        .expect("unknown email should still be accepted")
        .into_inner();

    assert!(response.accepted);

    let statement_dump = db
        .into_transaction_log()
        .iter()
        .flat_map(|transaction| transaction.statements().iter())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(!statement_dump.contains("BEGIN"));
    assert!(!statement_dump.contains("INSERT INTO \"outbox_event\""));
}

#[tokio::test]
async fn resend_verification_email_accepts_verified_user_without_side_effects() {
    let now = Utc::now();
    let user = mock_verified_user_account(now);
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([[user]])
        .into_connection();

    let response = test_service(db.clone())
        .resend_verification_email(resend_request("alice@example.com"))
        .await
        .expect("verified user should still be accepted")
        .into_inner();

    assert!(response.accepted);

    let statement_dump = db
        .into_transaction_log()
        .iter()
        .flat_map(|transaction| transaction.statements().iter())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(!statement_dump.contains("BEGIN"));
    assert!(!statement_dump.contains("INSERT INTO \"outbox_event\""));
}

#[tokio::test]
async fn resend_verification_email_replaces_active_tokens_and_writes_event() {
    let now = Utc::now();
    let user = mock_user_account(now);
    let existing_token = email_verification_token::Model {
        token_id: Uuid::new_v4(),
        user_id: user.user_id,
        token_hash: "existing-token-hash".to_string(),
        expires_at: (now + Duration::hours(1)).into(),
        consumed_at: None,
        created_at: now.into(),
    };
    let consumed_token = email_verification_token::Model {
        consumed_at: Some(now.into()),
        ..existing_token.clone()
    };
    let inserted_token = email_verification_token::Model {
        token_id: Uuid::new_v4(),
        user_id: user.user_id,
        token_hash: "new-token-hash".to_string(),
        expires_at: (now + Duration::hours(6)).into(),
        consumed_at: None,
        created_at: now.into(),
    };
    let outbox = mock_outbox_event(now, "VerificationEmailRequested");

    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([[user.clone()]])
        .append_query_results([[existing_token]])
        .append_query_results([[consumed_token]])
        .append_query_results([[inserted_token]])
        .append_query_results([[outbox]])
        .append_exec_results([mock_exec_result(), mock_exec_result(), mock_exec_result()])
        .into_connection();

    let response = test_service(db.clone())
        .resend_verification_email(resend_request(&user.email))
        .await
        .expect("unverified user should trigger resend")
        .into_inner();

    assert!(response.accepted);

    let statement_dump = db
        .into_transaction_log()
        .iter()
        .flat_map(|transaction| transaction.statements().iter())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(statement_dump.contains("UPDATE \"email_verification_token\""));
    assert!(statement_dump.contains("INSERT INTO \"email_verification_token\""));
    assert!(statement_dump.contains("VerificationEmailRequested"));
    assert!(statement_dump.contains("resend_verification"));
}

#[tokio::test]
async fn update_user_profile_requires_actor_context() {
    let db = MockDatabase::new(DbBackend::Postgres).into_connection();

    let error = test_service(db)
        .update_user_profile(update_profile_request(None, "Alice Updated", None))
        .await
        .expect_err("missing actor metadata should fail");

    assert_eq!(error.code(), tonic::Code::Unauthenticated);
    assert_eq!(error.message(), "missing authenticated actor context");
}

#[tokio::test]
async fn update_user_profile_clears_avatar_and_writes_outbox() {
    let now = Utc::now();
    let user_id = Uuid::new_v4();
    let profile = user_profile::Model {
        user_id,
        username: "alice".to_string(),
        display_name: "Alice".to_string(),
        avatar_url: Some("https://cdn.example.com/alice.png".to_string()),
        created_at: now.into(),
        updated_at: now.into(),
    };
    let updated_profile = user_profile::Model {
        display_name: "Alice Updated".to_string(),
        avatar_url: None,
        ..profile.clone()
    };
    let outbox = mock_outbox_event(now, "UserProfileUpdated");

    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([[profile]])
        .append_query_results([[updated_profile]])
        .append_query_results([[outbox]])
        .append_exec_results([mock_exec_result(), mock_exec_result()])
        .into_connection();

    let response = test_service(db.clone())
        .update_user_profile(update_profile_request(
            Some(user_id),
            "Alice Updated",
            Some("   "),
        ))
        .await
        .expect("profile update should succeed")
        .into_inner();

    assert_eq!(response.user_id, user_id.to_string());
    assert_eq!(response.username, "alice");
    assert_eq!(response.display_name, "Alice Updated");
    assert_eq!(response.avatar_url, None);
    assert!(response.updated_at.is_some());

    let statement_dump = db
        .into_transaction_log()
        .iter()
        .flat_map(|transaction| transaction.statements().iter())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(statement_dump.contains("UPDATE \"user_profile\""));
    assert!(statement_dump.contains("UserProfileUpdated"));
    assert!(statement_dump.contains("Alice Updated"));
}

#[tokio::test]
async fn get_user_profile_rejects_cross_user_lookup_on_actor_route() {
    let actor_user_id = Uuid::new_v4();
    let other_user_id = Uuid::new_v4();
    let db = MockDatabase::new(DbBackend::Postgres).into_connection();

    let error = test_service(db)
        .get_user_profile(get_user_profile_request(
            Some(actor_user_id),
            Some(other_user_id),
        ))
        .await
        .expect_err("cross-user profile lookup should be denied");

    assert_eq!(error.code(), tonic::Code::PermissionDenied);
    assert_eq!(
        error.message(),
        "cross-user profile lookup is not allowed on this route"
    );
}

#[tokio::test]
async fn get_users_by_ids_returns_profiles_from_single_batch_lookup() {
    let now = Utc::now();
    let first = user_profile::Model {
        user_id: Uuid::new_v4(),
        username: "alice".to_string(),
        display_name: "Alice".to_string(),
        avatar_url: Some("https://cdn.example.com/alice.png".to_string()),
        created_at: now.into(),
        updated_at: now.into(),
    };
    let second = user_profile::Model {
        user_id: Uuid::new_v4(),
        username: "bob".to_string(),
        display_name: "Bob".to_string(),
        avatar_url: None,
        created_at: now.into(),
        updated_at: now.into(),
    };
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([[first.clone(), second.clone()]])
        .into_connection();

    let response = test_service(db.clone())
        .get_users_by_ids(get_users_by_ids_request(&[first.user_id, second.user_id]))
        .await
        .expect("batched profile lookup should succeed")
        .into_inner();

    assert_eq!(response.users.len(), 2);
    assert_eq!(response.users[0].user_id, first.user_id.to_string());
    assert_eq!(response.users[0].username, "alice");
    assert_eq!(response.users[1].user_id, second.user_id.to_string());
    assert_eq!(response.users[1].username, "bob");

    let transaction_log = db.into_transaction_log();
    assert_eq!(transaction_log.len(), 1);
}
