#[allow(dead_code)]
mod common;

use chrono::Utc;
use common::{
    TestEnv, count_outbox_events, hash_token_for_test, insert_user_account, insert_user_session,
};
use identity::entity::user_session;
use relay_proto::identity::RevokeSessionRequest;
use sea_orm::EntityTrait;
use uuid::Uuid;

#[tokio::test]
async fn revoke_session_updates_session_and_writes_outbox()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = Utc::now();
    let session_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    insert_user_account(&env.db, user_id, "alice@example.com", None, "active", now).await?;

    insert_user_session(
        &env.db,
        session_id,
        user_id,
        hash_token_for_test("refresh-hash"),
        now,
        now + chrono::Duration::days(7),
        None,
        None,
        None,
        None,
    )
    .await?;

    let response = env
        .client
        .clone()
        .revoke_session(RevokeSessionRequest {
            session_id: session_id.to_string(),
            revoke_reason: None,
        })
        .await?
        .into_inner();

    assert!(response.revoked);
    assert!(response.revoked_at.is_some());

    let session = user_session::Entity::find_by_id(session_id)
        .one(&env.db)
        .await?
        .expect("session row");
    assert!(session.revoked_at.is_some());
    assert_eq!(session.revoke_reason.as_deref(), Some("logout"));

    let outbox = count_outbox_events(&env.db, session_id).await?;
    assert_eq!(outbox.len(), 1);
    assert_eq!(outbox[0].event_type, "SessionRevoked");

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn revoke_session_is_idempotent_for_already_revoked_session()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = Utc::now();
    let session_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    insert_user_account(&env.db, user_id, "alice@example.com", None, "active", now).await?;

    insert_user_session(
        &env.db,
        session_id,
        user_id,
        hash_token_for_test("refresh-hash"),
        now,
        now + chrono::Duration::days(7),
        Some(now),
        Some("logout"),
        None,
        None,
    )
    .await?;

    let response = env
        .client
        .clone()
        .revoke_session(RevokeSessionRequest {
            session_id: session_id.to_string(),
            revoke_reason: Some("logout".to_string()),
        })
        .await?
        .into_inner();

    assert!(response.revoked);
    assert!(response.revoked_at.is_some());

    let outbox = count_outbox_events(&env.db, session_id).await?;
    assert!(outbox.is_empty());

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn revoke_session_rejects_invalid_session_id()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;

    let error = env
        .client
        .clone()
        .revoke_session(RevokeSessionRequest {
            session_id: "not-a-uuid".to_string(),
            revoke_reason: None,
        })
        .await
        .expect_err("invalid session id should fail");

    assert_eq!(error.code(), tonic::Code::InvalidArgument);
    assert_eq!(error.message(), "invalid session_id");

    env.shutdown().await;
    Ok(())
}
