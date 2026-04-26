#[allow(dead_code)]
mod common;

use chrono::Utc;
use common::{TestEnv, insert_user_account, insert_user_profile};
use identity::entity::{outbox_event, user_profile};
use relay_proto::identity::UpdateUserProfileRequest;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use tonic::Request;
use uuid::Uuid;

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
            relay_types::ACTOR_USER_ID_METADATA,
            user_id.to_string().parse().expect("user id metadata should be valid"),
        );
    }

    request
}

#[tokio::test]
async fn update_user_profile_clears_avatar_and_writes_outbox()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = Utc::now();
    let user_id = Uuid::new_v4();

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

    let response = env
        .client
        .clone()
        .update_user_profile(update_profile_request(
            Some(user_id),
            "Alice Updated",
            Some("   "),
        ))
        .await?
        .into_inner();

    assert_eq!(response.user_id, user_id.to_string());
    assert_eq!(response.username, "alice");
    assert_eq!(response.display_name, "Alice Updated");
    assert_eq!(response.avatar_url, None);
    assert!(response.updated_at.is_some());

    let profile = user_profile::Entity::find_by_id(user_id)
        .one(&env.db)
        .await?
        .expect("profile row");
    assert_eq!(profile.display_name, "Alice Updated");
    assert_eq!(profile.avatar_url, None);

    let outbox = outbox_event::Entity::find()
        .filter(outbox_event::Column::AggregateId.eq(user_id))
        .all(&env.db)
        .await?;
    assert_eq!(outbox.len(), 1);
    assert_eq!(outbox[0].event_type, "UserProfileUpdated");

    env.shutdown().await;
    Ok(())
}
