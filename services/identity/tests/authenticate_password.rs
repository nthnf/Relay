#[allow(dead_code)]
mod common;

use chrono::Utc;
use common::{
    TestEnv, auth_keys, hash_password_for_test, hash_token_for_test, insert_password_credential,
    insert_user_account, insert_user_profile,
};
use identity::entity::user_session;
use relay_proto::identity::AuthenticatePasswordRequest;
use sea_orm::EntityTrait;
use uuid::Uuid;

#[tokio::test]
async fn authenticate_password_returns_tokens_and_creates_session()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = Utc::now();
    let user_id = Uuid::new_v4();
    let password = "plain-password";
    let password_hash = hash_password_for_test(password);

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
    insert_password_credential(&env.db, user_id, password_hash, now).await?;

    let response = env
        .client
        .clone()
        .authenticate_password(AuthenticatePasswordRequest {
            email: "Alice@Example.com".to_string(),
            password: password.to_string(),
            client_instance_id: None,
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
    assert_eq!(claims.session_id.to_string(), response.session_id);

    let sessions = user_session::Entity::find().all(&env.db).await?;
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].user_id, user_id);
    assert_eq!(sessions[0].refresh_token_hash, hash_token_for_test(&response.refresh_token));

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn authenticate_password_rejects_invalid_credentials()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = Utc::now();
    let user_id = Uuid::new_v4();

    insert_user_account(&env.db, user_id, "alice@example.com", Some(now), "active", now).await?;
    insert_password_credential(
        &env.db,
        user_id,
        hash_password_for_test("different-password"),
        now,
    )
    .await?;

    let error = env
        .client
        .clone()
        .authenticate_password(AuthenticatePasswordRequest {
            email: "Alice@Example.com".to_string(),
            password: "plain-password".to_string(),
            client_instance_id: None,
        })
        .await
        .expect_err("authentication should reject invalid password");

    assert_eq!(error.code(), tonic::Code::Unauthenticated);
    assert_eq!(error.message(), "invalid credentials");

    let sessions = user_session::Entity::find().all(&env.db).await?;
    assert!(sessions.is_empty());

    env.shutdown().await;
    Ok(())
}
