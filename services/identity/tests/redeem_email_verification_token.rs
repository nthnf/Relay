#[allow(dead_code)]
mod common;

use chrono::{Duration, Utc};
use common::{
    TestEnv, auth_keys, hash_token_for_test, insert_email_verification_token,
    insert_user_account, insert_user_profile,
};
use identity::entity::{email_verification_token, outbox_event, user_account, user_session};
use relay_proto::identity::RedeemEmailVerificationTokenRequest;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

#[tokio::test]
async fn redeem_email_verification_token_creates_first_session_and_outbox()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = Utc::now();
    let user_id = Uuid::new_v4();
    let token = "verification-token-value";
    let token_id = Uuid::new_v4();

    insert_user_account(&env.db, user_id, "alice@example.com", None, "active", now).await?;
    insert_user_profile(
        &env.db,
        user_id,
        "alice",
        "Alice",
        Some("https://cdn.example.com/alice.png"),
        now,
    )
    .await?;
    insert_email_verification_token(
        &env.db,
        token_id,
        user_id,
        hash_token_for_test(token),
        now + Duration::hours(1),
        None,
        now,
    )
    .await?;

    let response = env
        .client
        .clone()
        .redeem_email_verification_token(RedeemEmailVerificationTokenRequest {
            token: token.to_string(),
        })
        .await?
        .into_inner();

    assert_eq!(response.user_id, user_id.to_string());
    assert!(!response.session_id.is_empty());
    assert!(!response.access_token.is_empty());
    assert!(!response.refresh_token.is_empty());
    assert!(response.access_token_expires_at.is_some());
    assert!(response.refresh_token_expires_at.is_some());
    assert!(response.email_verified);
    assert_eq!(response.profile.as_ref().map(|p| p.username.as_str()), Some("alice"));

    let claims = auth_keys().verify_access_token(&response.access_token)?;
    assert_eq!(claims.user_id, user_id);

    let account = user_account::Entity::find_by_id(user_id)
        .one(&env.db)
        .await?
        .expect("account row");
    assert!(account.email_verified_at.is_some());

    let stored_token = email_verification_token::Entity::find_by_id(token_id)
        .one(&env.db)
        .await?
        .expect("token row");
    assert!(stored_token.consumed_at.is_some());

    let sessions = user_session::Entity::find().all(&env.db).await?;
    assert_eq!(sessions.len(), 1);

    let outbox = outbox_event::Entity::find()
        .filter(outbox_event::Column::AggregateId.eq(user_id))
        .all(&env.db)
        .await?;
    assert_eq!(outbox.len(), 1);
    assert_eq!(outbox[0].event_type, "UserEmailVerified");

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn redeem_email_verification_token_rejects_unknown_token()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;

    let error = env
        .client
        .clone()
        .redeem_email_verification_token(RedeemEmailVerificationTokenRequest {
            token: "missing-token".to_string(),
        })
        .await
        .expect_err("unknown token should fail");

    assert_eq!(error.code(), tonic::Code::NotFound);
    assert_eq!(error.message(), "invalid verification token");

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn redeem_email_verification_token_rejects_consumed_token()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = Utc::now();
    let user_id = Uuid::new_v4();
    let token = "consumed-token";

    insert_user_account(&env.db, user_id, "alice@example.com", None, "active", now).await?;
    insert_email_verification_token(
        &env.db,
        Uuid::new_v4(),
        user_id,
        hash_token_for_test(token),
        now + Duration::hours(1),
        Some(now),
        now,
    )
    .await?;

    let error = env
        .client
        .clone()
        .redeem_email_verification_token(RedeemEmailVerificationTokenRequest {
            token: token.to_string(),
        })
        .await
        .expect_err("consumed token should fail");

    assert_eq!(error.code(), tonic::Code::NotFound);
    assert_eq!(error.message(), "invalid verification token");

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn redeem_email_verification_token_rejects_expired_token()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = Utc::now();
    let user_id = Uuid::new_v4();
    let token = "expired-token";

    insert_user_account(&env.db, user_id, "alice@example.com", None, "active", now).await?;
    insert_email_verification_token(
        &env.db,
        Uuid::new_v4(),
        user_id,
        hash_token_for_test(token),
        now - Duration::seconds(1),
        None,
        now,
    )
    .await?;

    let error = env
        .client
        .clone()
        .redeem_email_verification_token(RedeemEmailVerificationTokenRequest {
            token: token.to_string(),
        })
        .await
        .expect_err("expired token should fail");

    assert_eq!(error.code(), tonic::Code::NotFound);
    assert_eq!(error.message(), "invalid verification token");

    env.shutdown().await;
    Ok(())
}
