mod setup;

use friendship::entity::{friend_request, friendship_edge, outbox_event, user_block};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set};

use setup::{block_request, connect_friendship, FakeIdentityEnv, TestDbEnv};

#[tokio::test]
async fn blocks_user_cancels_request_and_removes_friendship()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let actor_user_id = uuid::Uuid::new_v4();
    let target_user_id = uuid::Uuid::new_v4();
    let friend_request_id = uuid::Uuid::new_v4();
    let (identity_env, identity) = FakeIdentityEnv::start([target_user_id]).await?;
    let db_env = TestDbEnv::start().await?;
    let (friendship_env, mut client) = connect_friendship(db_env.db.clone(), identity).await?;

    friend_request::Entity::insert(friend_request::ActiveModel {
        friend_request_id: Set(friend_request_id),
        requester_user_id: Set(actor_user_id),
        addressee_user_id: Set(target_user_id),
        status: Set("pending".to_string()),
        created_at: Set(chrono::Utc::now().into()),
        resolved_at: Set(None),
        resolution_reason: Set(None),
    })
    .exec(&db_env.db)
    .await?;

    friendship_edge::Entity::insert(friendship_edge::ActiveModel {
        user_id: Set(actor_user_id),
        friend_user_id: Set(target_user_id),
        friend_request_id: Set(friend_request_id),
        accepted_at: Set(chrono::Utc::now().into()),
        created_at: Set(chrono::Utc::now().into()),
    })
    .exec(&db_env.db)
    .await?;

    friendship_edge::Entity::insert(friendship_edge::ActiveModel {
        user_id: Set(target_user_id),
        friend_user_id: Set(actor_user_id),
        friend_request_id: Set(friend_request_id),
        accepted_at: Set(chrono::Utc::now().into()),
        created_at: Set(chrono::Utc::now().into()),
    })
    .exec(&db_env.db)
    .await?;

    let response = client
        .block_user(block_request(actor_user_id, target_user_id))
        .await?
        .into_inner();

    assert!(response.blocked);
    assert!(!response.already_blocked);

    let block = user_block::Entity::find()
        .filter(user_block::Column::BlockerUserId.eq(actor_user_id))
        .filter(user_block::Column::BlockedUserId.eq(target_user_id))
        .one(&db_env.db)
        .await?
        .expect("block row");
    assert_eq!(block.reason, None);

    let stored_request = friend_request::Entity::find_by_id(friend_request_id)
        .one(&db_env.db)
        .await?
        .expect("friend request row");
    assert_eq!(stored_request.status, "canceled_by_block");

    assert!(
        friendship_edge::Entity::find()
            .filter(friendship_edge::Column::UserId.eq(actor_user_id))
            .filter(friendship_edge::Column::FriendUserId.eq(target_user_id))
            .one(&db_env.db)
            .await?
            .is_none()
    );
    assert!(
        friendship_edge::Entity::find()
            .filter(friendship_edge::Column::UserId.eq(target_user_id))
            .filter(friendship_edge::Column::FriendUserId.eq(actor_user_id))
            .one(&db_env.db)
            .await?
            .is_none()
    );

    assert!(
        outbox_event::Entity::find()
            .filter(outbox_event::Column::EventType.eq("FriendRequestCanceledByBlock"))
            .one(&db_env.db)
            .await?
            .is_some()
    );

    friendship_env.shutdown().await;
    identity_env.shutdown().await;
    Ok(())
}
