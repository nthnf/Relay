mod setup;

use friendship::entity::{friend_request, friendship_edge, outbox_event};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set};

use setup::{connect_friendship, remove_request, FakeIdentityEnv, TestDbEnv};

#[tokio::test]
async fn removes_friendship_and_writes_outbox()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let actor_user_id = uuid::Uuid::new_v4();
    let friend_user_id = uuid::Uuid::new_v4();
    let friend_request_id = uuid::Uuid::new_v4();
    let (identity_env, identity) = FakeIdentityEnv::start([]).await?;
    let db_env = TestDbEnv::start().await?;
    let (friendship_env, mut client) = connect_friendship(db_env.db.clone(), identity).await?;

    friend_request::Entity::insert(friend_request::ActiveModel {
        friend_request_id: Set(friend_request_id),
        requester_user_id: Set(actor_user_id),
        addressee_user_id: Set(friend_user_id),
        status: Set("accepted".to_string()),
        created_at: Set(chrono::Utc::now().into()),
        resolved_at: Set(Some(chrono::Utc::now().into())),
        resolution_reason: Set(Some("accepted".to_string())),
    })
    .exec(&db_env.db)
    .await?;

    friendship_edge::Entity::insert(friendship_edge::ActiveModel {
        user_id: Set(actor_user_id),
        friend_user_id: Set(friend_user_id),
        friend_request_id: Set(friend_request_id),
        accepted_at: Set(chrono::Utc::now().into()),
        created_at: Set(chrono::Utc::now().into()),
    })
    .exec(&db_env.db)
    .await?;

    friendship_edge::Entity::insert(friendship_edge::ActiveModel {
        user_id: Set(friend_user_id),
        friend_user_id: Set(actor_user_id),
        friend_request_id: Set(friend_request_id),
        accepted_at: Set(chrono::Utc::now().into()),
        created_at: Set(chrono::Utc::now().into()),
    })
    .exec(&db_env.db)
    .await?;

    let response = client
        .remove_friend(remove_request(actor_user_id, friend_user_id))
        .await?
        .into_inner();

    assert!(response.removed);
    assert!(response.removed_at.is_some());

    assert!(
        friendship_edge::Entity::find()
            .filter(friendship_edge::Column::UserId.eq(actor_user_id))
            .filter(friendship_edge::Column::FriendUserId.eq(friend_user_id))
            .one(&db_env.db)
            .await?
            .is_none()
    );
    assert!(
        friendship_edge::Entity::find()
            .filter(friendship_edge::Column::UserId.eq(friend_user_id))
            .filter(friendship_edge::Column::FriendUserId.eq(actor_user_id))
            .one(&db_env.db)
            .await?
            .is_none()
    );

    let outbox_row = outbox_event::Entity::find()
        .filter(outbox_event::Column::AggregateType.eq("friendship"))
        .one(&db_env.db)
        .await?
        .expect("outbox row");
    assert_eq!(outbox_row.event_type, "FriendshipRemoved");

    friendship_env.shutdown().await;
    identity_env.shutdown().await;
    Ok(())
}
