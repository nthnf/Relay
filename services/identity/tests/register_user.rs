#[allow(dead_code)]
mod common;

use common::{TestEnv, insert_user_account, insert_user_profile};
use identity::entity::{outbox_event, user_account, user_profile};
use relay_proto::identity::RegisterUserRequest;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

#[tokio::test]
async fn register_user_persists_identity_state_and_outbox_events()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;

    let response = env
        .client
        .clone()
        .register_user(RegisterUserRequest {
            email: "user1@example.com".to_string(),
            password: "correct horse battery staple".to_string(),
            username: "user1".to_string(),
            display_name: "User One".to_string(),
            avatar_url: Some("https://example.com/avatar.png".to_string()),
        })
        .await?
        .into_inner();

    assert_eq!(response.email, "user1@example.com");
    assert!(!response.user_id.is_empty());
    assert!(response.verification_email_requested_at.is_some());

    let account = user_account::Entity::find()
        .filter(user_account::Column::EmailNormalized.eq("user1@example.com"))
        .one(&env.db)
        .await?
        .expect("user account row");
    assert_eq!(account.email, "user1@example.com");
    assert_eq!(account.user_id.to_string(), response.user_id);

    let profile = user_profile::Entity::find_by_id(account.user_id)
        .one(&env.db)
        .await?
        .expect("user profile row");
    assert_eq!(profile.username, "user1");
    assert_eq!(profile.display_name, "User One");

    let outbox_rows = outbox_event::Entity::find()
        .filter(outbox_event::Column::AggregateId.eq(account.user_id))
        .all(&env.db)
        .await?;
    assert_eq!(outbox_rows.len(), 2);
    assert!(
        outbox_rows
            .iter()
            .any(|row| row.event_type == "UserRegistered")
    );
    assert!(
        outbox_rows
            .iter()
            .any(|row| row.event_type == "VerificationEmailRequested")
    );

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn register_user_returns_already_exists_for_duplicate_email()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = chrono::Utc::now();
    let user_id = Uuid::new_v4();

    insert_user_account(&env.db, user_id, "alice@example.com", None, "active", now).await?;

    let error = env
        .client
        .clone()
        .register_user(RegisterUserRequest {
            email: "ALICE@example.com".to_string(),
            password: "plain-password".to_string(),
            username: "alice2".to_string(),
            display_name: "Alice Two".to_string(),
            avatar_url: None,
        })
        .await
        .expect_err("duplicate email should be rejected");

    assert_eq!(error.code(), tonic::Code::AlreadyExists);
    assert_eq!(error.message(), "A user with this email already exists");

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn register_user_returns_already_exists_for_duplicate_username_constraint()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = chrono::Utc::now();
    let existing_user_id = Uuid::new_v4();

    insert_user_account(
        &env.db,
        existing_user_id,
        "existing@example.com",
        None,
        "active",
        now,
    )
    .await?;
    insert_user_profile(
        &env.db,
        existing_user_id,
        "alice",
        "Existing Alice",
        None,
        now,
    )
    .await?;

    let error = env
        .client
        .clone()
        .register_user(RegisterUserRequest {
            email: "new@example.com".to_string(),
            password: "plain-password".to_string(),
            username: "alice".to_string(),
            display_name: "Alice Two".to_string(),
            avatar_url: None,
        })
        .await
        .expect_err("duplicate username should be rejected");

    assert_eq!(error.code(), tonic::Code::AlreadyExists);
    assert_eq!(error.message(), "email or username already exists");

    let accounts = user_account::Entity::find().all(&env.db).await?;
    assert_eq!(accounts.len(), 1);

    env.shutdown().await;
    Ok(())
}
