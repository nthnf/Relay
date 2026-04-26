#[allow(dead_code)]
mod common;

use chrono::Utc;
use common::{TestEnv, insert_user_account, insert_user_profile};
use relay_proto::identity::GetUsersByIdsRequest;
use uuid::Uuid;

#[tokio::test]
async fn get_users_by_ids_returns_profiles_from_single_batch_lookup()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = Utc::now();
    let first_user_id = Uuid::new_v4();
    let second_user_id = Uuid::new_v4();

    insert_user_account(
        &env.db,
        first_user_id,
        "alice@example.com",
        None,
        "active",
        now,
    )
    .await?;
    insert_user_account(
        &env.db,
        second_user_id,
        "bob@example.com",
        None,
        "active",
        now,
    )
    .await?;
    insert_user_profile(
        &env.db,
        first_user_id,
        "alice",
        "Alice",
        Some("https://cdn.example.com/alice.png"),
        now,
    )
    .await?;
    insert_user_profile(&env.db, second_user_id, "bob", "Bob", None, now).await?;

    let response = env
        .client
        .clone()
        .get_users_by_ids(GetUsersByIdsRequest {
            user_ids: vec![first_user_id.to_string(), second_user_id.to_string()],
        })
        .await?
        .into_inner();

    assert_eq!(response.users.len(), 2);
    assert_eq!(response.users[0].user_id, first_user_id.to_string());
    assert_eq!(response.users[0].username, "alice");
    assert_eq!(response.users[1].user_id, second_user_id.to_string());
    assert_eq!(response.users[1].username, "bob");

    env.shutdown().await;
    Ok(())
}
