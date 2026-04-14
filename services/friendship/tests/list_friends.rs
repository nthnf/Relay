mod setup;

use friendship::entity::{friend_request, friendship_edge};
use sea_orm::{EntityTrait, Set};

use setup::{connect_friendship, list_friends_request, FakeIdentityEnv, TestDbEnv};

#[tokio::test]
async fn lists_friends_with_cursor_pagination()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let actor_user_id = uuid::Uuid::new_v4();
    let friend_user_1 = uuid::Uuid::new_v4();
    let friend_user_2 = uuid::Uuid::new_v4();
    let friend_user_3 = uuid::Uuid::new_v4();
    let friend_request_id_1 = uuid::Uuid::new_v4();
    let friend_request_id_2 = uuid::Uuid::new_v4();
    let friend_request_id_3 = uuid::Uuid::new_v4();
    let (identity_env, identity) = FakeIdentityEnv::start([friend_user_1, friend_user_2, friend_user_3]).await?;
    let db_env = TestDbEnv::start().await?;
    let (friendship_env, mut client) = connect_friendship(db_env.db.clone(), identity).await?;

    let now = chrono::Utc::now();
    friend_request::Entity::insert_many([
        friend_request::ActiveModel {
            friend_request_id: Set(friend_request_id_1),
            requester_user_id: Set(friend_user_1),
            addressee_user_id: Set(actor_user_id),
            status: Set("accepted".to_string()),
            created_at: Set((now - chrono::Duration::minutes(3)).into()),
            resolved_at: Set(Some((now - chrono::Duration::minutes(3)).into())),
            resolution_reason: Set(Some("accepted".to_string())),
        },
        friend_request::ActiveModel {
            friend_request_id: Set(friend_request_id_2),
            requester_user_id: Set(friend_user_2),
            addressee_user_id: Set(actor_user_id),
            status: Set("accepted".to_string()),
            created_at: Set((now - chrono::Duration::minutes(2)).into()),
            resolved_at: Set(Some((now - chrono::Duration::minutes(2)).into())),
            resolution_reason: Set(Some("accepted".to_string())),
        },
        friend_request::ActiveModel {
            friend_request_id: Set(friend_request_id_3),
            requester_user_id: Set(friend_user_3),
            addressee_user_id: Set(actor_user_id),
            status: Set("accepted".to_string()),
            created_at: Set((now - chrono::Duration::minutes(1)).into()),
            resolved_at: Set(Some((now - chrono::Duration::minutes(1)).into())),
            resolution_reason: Set(Some("accepted".to_string())),
        },
    ])
    .exec(&db_env.db)
    .await?;

    friendship_edge::Entity::insert_many([
        friendship_edge::ActiveModel {
            user_id: Set(actor_user_id),
            friend_user_id: Set(friend_user_1),
            friend_request_id: Set(friend_request_id_1),
            accepted_at: Set((now - chrono::Duration::minutes(3)).into()),
            created_at: Set((now - chrono::Duration::minutes(3)).into()),
        },
        friendship_edge::ActiveModel {
            user_id: Set(actor_user_id),
            friend_user_id: Set(friend_user_2),
            friend_request_id: Set(friend_request_id_2),
            accepted_at: Set((now - chrono::Duration::minutes(2)).into()),
            created_at: Set((now - chrono::Duration::minutes(2)).into()),
        },
        friendship_edge::ActiveModel {
            user_id: Set(actor_user_id),
            friend_user_id: Set(friend_user_3),
            friend_request_id: Set(friend_request_id_3),
            accepted_at: Set((now - chrono::Duration::minutes(1)).into()),
            created_at: Set((now - chrono::Duration::minutes(1)).into()),
        },
    ])
    .exec(&db_env.db)
    .await?;

    let first_page = client
        .list_friends(list_friends_request(actor_user_id, Some(2), None))
        .await?
        .into_inner();

    assert_eq!(first_page.friends.len(), 2);
    assert_eq!(first_page.friends[0].friend_user_id, friend_user_3.to_string());
    assert_eq!(first_page.friends[1].friend_user_id, friend_user_2.to_string());
    let next_page_token = first_page.next_page_token.expect("next page token");

    let second_page = client
        .list_friends(list_friends_request(actor_user_id, Some(2), Some(next_page_token)))
        .await?
        .into_inner();

    assert_eq!(second_page.friends.len(), 1);
    assert_eq!(second_page.friends[0].friend_user_id, friend_user_1.to_string());
    assert!(second_page.next_page_token.is_none());

    friendship_env.shutdown().await;
    identity_env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn lists_friends_rejects_bad_page_token()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let actor_user_id = uuid::Uuid::new_v4();
    let (identity_env, identity) = FakeIdentityEnv::start([]).await?;
    let db_env = TestDbEnv::start().await?;
    let (friendship_env, mut client) = connect_friendship(db_env.db.clone(), identity).await?;

    let error = client
        .list_friends(list_friends_request(actor_user_id, Some(2), Some("bad-token".to_string())))
        .await
        .expect_err("bad page token should fail");

    assert_eq!(error.code(), tonic::Code::InvalidArgument);
    assert_eq!(error.message(), "Invalid page token");

    friendship_env.shutdown().await;
    identity_env.shutdown().await;
    Ok(())
}
