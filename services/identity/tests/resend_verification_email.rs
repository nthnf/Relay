#[allow(dead_code)]
mod common;

use chrono::{Duration, Utc};
use common::{
    TestEnv, count_outbox_events, hash_token_for_test, insert_email_verification_token,
    insert_user_account,
};
use identity::entity::email_verification_token;
use relay_proto::identity::ResendVerificationEmailRequest;
use sea_orm::EntityTrait;
use uuid::Uuid;

#[tokio::test]
async fn resend_verification_email_accepts_unknown_email_without_side_effects()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;

    let response = env
        .client
        .clone()
        .resend_verification_email(ResendVerificationEmailRequest {
            email: "missing@example.com".to_string(),
        })
        .await?
        .into_inner();

    assert!(response.accepted);

    let outbox = count_outbox_events(&env.db, uuid::Uuid::nil()).await?;
    assert!(outbox.is_empty());

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn resend_verification_email_accepts_verified_user_without_side_effects()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = Utc::now();
    let user_id = Uuid::new_v4();

    insert_user_account(&env.db, user_id, "alice@example.com", Some(now), "active", now).await?;

    let response = env
        .client
        .clone()
        .resend_verification_email(ResendVerificationEmailRequest {
            email: "alice@example.com".to_string(),
        })
        .await?
        .into_inner();

    assert!(response.accepted);

    let outbox = count_outbox_events(&env.db, user_id).await?;
    assert!(outbox.is_empty());

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn resend_verification_email_replaces_active_tokens_and_writes_event()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = Utc::now();
    let user_id = Uuid::new_v4();
    let first_token_id = Uuid::new_v4();

    insert_user_account(&env.db, user_id, "alice@example.com", None, "active", now).await?;
    insert_email_verification_token(
        &env.db,
        first_token_id,
        user_id,
        hash_token_for_test("existing-token-hash"),
        now + Duration::hours(1),
        None,
        now,
    )
    .await?;

    let response = env
        .client
        .clone()
        .resend_verification_email(ResendVerificationEmailRequest {
            email: "alice@example.com".to_string(),
        })
        .await?
        .into_inner();

    assert!(response.accepted);

    let tokens = email_verification_token::Entity::find().all(&env.db).await?;
    assert_eq!(tokens.len(), 2);
    assert!(tokens.iter().any(|token| token.consumed_at.is_some()));
    assert!(tokens.iter().any(|token| token.consumed_at.is_none()));

    let outbox = count_outbox_events(&env.db, user_id).await?;
    assert_eq!(outbox.len(), 1);
    assert_eq!(outbox[0].event_type, "VerificationEmailRequested");

    env.shutdown().await;
    Ok(())
}
