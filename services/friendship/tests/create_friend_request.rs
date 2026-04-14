mod setup;

use friendship::entity::{friend_request, outbox_event};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use setup::{FakeIdentityEnv, TestDbEnv, connect_friendship, create_request};

#[tokio::test]
async fn create_friend_request_persists_rows_and_returns_record()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let actor_user_id = uuid::Uuid::new_v4();
    let target_user_id = uuid::Uuid::new_v4();
    let (identity_env, identity) = FakeIdentityEnv::start([target_user_id]).await?;
    let db_env = TestDbEnv::start().await?;
    let (friendship_env, mut client) = connect_friendship(db_env.db.clone(), identity).await?;

    let response = client
        .create_friend_request(create_request(actor_user_id, target_user_id))
        .await?
        .into_inner();

    assert_eq!(response.requester_user_id, actor_user_id.to_string());
    assert_eq!(response.addressee_user_id, target_user_id.to_string());
    assert_eq!(response.status, "pending");
    assert!(response.created_at.is_some());

    let request_row = friend_request::Entity::find()
        .filter(friend_request::Column::RequesterUserId.eq(actor_user_id))
        .filter(friend_request::Column::AddresseeUserId.eq(target_user_id))
        .one(&db_env.db)
        .await?
        .expect("friend request row");
    assert_eq!(request_row.status, "pending");

    let outbox_row = outbox_event::Entity::find()
        .filter(outbox_event::Column::AggregateType.eq("friend_request"))
        .one(&db_env.db)
        .await?
        .expect("outbox row");
    assert_eq!(outbox_row.event_type, "FriendRequestCreated");

    friendship_env.shutdown().await;
    identity_env.shutdown().await;
    Ok(())
}
