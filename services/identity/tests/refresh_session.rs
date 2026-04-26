#[allow(dead_code)]
mod common;

use chrono::{Duration, Utc};
use common::{
    TestEnv, auth_keys, hash_token_for_test, insert_user_account, insert_user_profile,
    insert_user_session,
};
use identity::entity::user_session;
use relay_proto::identity::RefreshSessionRequest;
use sea_orm::EntityTrait;
use uuid::Uuid;

#[tokio::test]
async fn refresh_session_rotates_session_and_returns_new_tokens()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = Utc::now();
    let user_id = Uuid::new_v4();
    let old_session_id = Uuid::new_v4();
    let refresh_token = "refresh-token-value";

    insert_user_account(&env.db, user_id, "alice@example.com", Some(now), "active", now).await?;
    insert_user_profile(
        &env.db,
        user_id,
        "alice",
        "Alice",
        Some("https://cdn.example.com/alice.png"),
        now,
    )
    .await?;
    insert_user_session(
        &env.db,
        old_session_id,
        user_id,
        hash_token_for_test(refresh_token),
        now,
        now + Duration::days(7),
        None,
        None,
        None,
        None,
    )
    .await?;

    let response = env
        .client
        .clone()
        .refresh_session(RefreshSessionRequest {
            refresh_token: refresh_token.to_string(),
            client_instance_id: None,
        })
        .await?
        .into_inner();

    assert_eq!(response.user_id, user_id.to_string());
    assert_ne!(response.session_id, old_session_id.to_string());
    assert!(!response.access_token.is_empty());
    assert!(!response.refresh_token.is_empty());
    assert!(response.access_token_expires_at.is_some());
    assert!(response.refresh_token_expires_at.is_some());
    assert!(response.email_verified);
    assert_eq!(response.profile.as_ref().map(|p| p.username.as_str()), Some("alice"));

    let claims = auth_keys().verify_access_token(&response.access_token)?;
    assert_eq!(claims.user_id, user_id);
    assert_eq!(claims.session_id.to_string(), response.session_id);

    let sessions = user_session::Entity::find().all(&env.db).await?;
    assert_eq!(sessions.len(), 2);

    let old_session = sessions
        .iter()
        .find(|session| session.session_id == old_session_id)
        .expect("old session row");
    assert!(old_session.revoked_at.is_some());
    assert_eq!(old_session.revoke_reason.as_deref(), Some("rotated"));

    let new_session_id = Uuid::parse_str(&response.session_id)?;
    let new_session = sessions
        .iter()
        .find(|session| session.session_id == new_session_id)
        .expect("new session row");
    assert_eq!(new_session.refresh_token_hash, hash_token_for_test(&response.refresh_token));

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn refresh_session_rejects_invalid_refresh_token()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;

    let error = env
        .client
        .clone()
        .refresh_session(RefreshSessionRequest {
            refresh_token: "missing-refresh-token".to_string(),
            client_instance_id: None,
        })
        .await
        .expect_err("missing refresh token should fail");

    assert_eq!(error.code(), tonic::Code::Unauthenticated);
    assert_eq!(error.message(), "invalid refresh token");

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn refresh_session_rejects_revoked_refresh_token()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = Utc::now();
    let user_id = Uuid::new_v4();
    let session_id = Uuid::new_v4();
    let refresh_token = "revoked-refresh-token";

    insert_user_account(&env.db, user_id, "alice@example.com", Some(now), "active", now).await?;
    insert_user_profile(&env.db, user_id, "alice", "Alice", None, now).await?;
    insert_user_session(
        &env.db,
        session_id,
        user_id,
        hash_token_for_test(refresh_token),
        now,
        now + Duration::days(7),
        Some(now),
        Some("rotated"),
        None,
        None,
    )
    .await?;

    let error = env
        .client
        .clone()
        .refresh_session(RefreshSessionRequest {
            refresh_token: refresh_token.to_string(),
            client_instance_id: None,
        })
        .await
        .expect_err("revoked refresh token should fail");

    assert_eq!(error.code(), tonic::Code::Unauthenticated);
    assert_eq!(error.message(), "invalid refresh token");

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn refresh_session_rejects_expired_refresh_token()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = Utc::now();
    let user_id = Uuid::new_v4();
    let session_id = Uuid::new_v4();
    let refresh_token = "expired-refresh-token";

    insert_user_account(&env.db, user_id, "alice@example.com", Some(now), "active", now).await?;
    insert_user_profile(&env.db, user_id, "alice", "Alice", None, now).await?;
    insert_user_session(
        &env.db,
        session_id,
        user_id,
        hash_token_for_test(refresh_token),
        now,
        now - Duration::seconds(1),
        None,
        None,
        None,
        None,
    )
    .await?;

    let error = env
        .client
        .clone()
        .refresh_session(RefreshSessionRequest {
            refresh_token: refresh_token.to_string(),
            client_instance_id: None,
        })
        .await
        .expect_err("expired refresh token should fail");

    assert_eq!(error.code(), tonic::Code::Unauthenticated);
    assert_eq!(error.message(), "invalid refresh token");

    env.shutdown().await;
    Ok(())
}
