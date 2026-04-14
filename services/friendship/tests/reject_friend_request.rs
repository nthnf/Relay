mod setup;

use friendship::entity::{friend_request, outbox_event};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set};

use setup::{connect_friendship, reject_request, FakeIdentityEnv, TestDbEnv};

#[tokio::test]
async fn rejects_pending_request_and_writes_outbox()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let requester_user_id = uuid::Uuid::new_v4();
    let addressee_user_id = uuid::Uuid::new_v4();
    let friend_request_id = uuid::Uuid::new_v4();
    let (identity_env, identity) = FakeIdentityEnv::start([]).await?;
    let db_env = TestDbEnv::start().await?;
    let (friendship_env, mut client) = connect_friendship(db_env.db.clone(), identity).await?;

    friend_request::Entity::insert(friend_request::ActiveModel {
        friend_request_id: Set(friend_request_id),
        requester_user_id: Set(requester_user_id),
        addressee_user_id: Set(addressee_user_id),
        status: Set("pending".to_string()),
        created_at: Set(chrono::Utc::now().into()),
        resolved_at: Set(None),
        resolution_reason: Set(None),
    })
    .exec(&db_env.db)
    .await?;

    let response = client
        .reject_friend_request(reject_request(addressee_user_id, friend_request_id))
        .await?
        .into_inner();

    assert_eq!(response.friend_request_id, friend_request_id.to_string());
    assert_eq!(response.status, "rejected");
    assert!(response.resolved_at.is_some());

    let stored_request = friend_request::Entity::find_by_id(friend_request_id)
        .one(&db_env.db)
        .await?
        .expect("friend request row");
    assert_eq!(stored_request.status, "rejected");

    let outbox_row = outbox_event::Entity::find()
        .filter(outbox_event::Column::AggregateId.eq(friend_request_id))
        .one(&db_env.db)
        .await?
        .expect("outbox row");
    assert_eq!(outbox_row.event_type, "FriendRequestRejected");

    friendship_env.shutdown().await;
    identity_env.shutdown().await;
    Ok(())
}
