mod setup;

use friendship::entity::friend_request;
use sea_orm::{EntityTrait, Set};

use setup::{
    connect_friendship, list_pending_requests_request, FakeIdentityEnv, TestDbEnv,
};

#[tokio::test]
async fn lists_pending_requests_with_direction_and_cursor()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let actor_user_id = uuid::Uuid::new_v4();
    let incoming_user_1 = uuid::Uuid::new_v4();
    let incoming_user_2 = uuid::Uuid::new_v4();
    let outgoing_user = uuid::Uuid::new_v4();
    let request_id_1 = uuid::Uuid::new_v4();
    let request_id_2 = uuid::Uuid::new_v4();
    let request_id_3 = uuid::Uuid::new_v4();
    let (identity_env, identity) = FakeIdentityEnv::start([incoming_user_1, incoming_user_2, outgoing_user]).await?;
    let db_env = TestDbEnv::start().await?;
    let (friendship_env, mut client) = connect_friendship(db_env.db.clone(), identity).await?;

    let now = chrono::Utc::now();
    friend_request::Entity::insert_many([
        friend_request::ActiveModel {
            friend_request_id: Set(request_id_1),
            requester_user_id: Set(incoming_user_1),
            addressee_user_id: Set(actor_user_id),
            status: Set("pending".to_string()),
            created_at: Set((now - chrono::Duration::minutes(3)).into()),
            resolved_at: Set(None),
            resolution_reason: Set(None),
        },
        friend_request::ActiveModel {
            friend_request_id: Set(request_id_2),
            requester_user_id: Set(incoming_user_2),
            addressee_user_id: Set(actor_user_id),
            status: Set("pending".to_string()),
            created_at: Set((now - chrono::Duration::minutes(2)).into()),
            resolved_at: Set(None),
            resolution_reason: Set(None),
        },
        friend_request::ActiveModel {
            friend_request_id: Set(request_id_3),
            requester_user_id: Set(actor_user_id),
            addressee_user_id: Set(outgoing_user),
            status: Set("pending".to_string()),
            created_at: Set((now - chrono::Duration::minutes(1)).into()),
            resolved_at: Set(None),
            resolution_reason: Set(None),
        },
    ])
    .exec(&db_env.db)
    .await?;

    let first_page = client
        .list_pending_requests(list_pending_requests_request(
            actor_user_id,
            Some("incoming".to_string()),
            Some(1),
            None,
        ))
        .await?
        .into_inner();

    assert_eq!(first_page.requests.len(), 1);
    assert_eq!(first_page.requests[0].friend_request_id, request_id_2.to_string());
    let next_page_token = first_page.next_page_token.expect("next page token");

    let second_page = client
        .list_pending_requests(list_pending_requests_request(
            actor_user_id,
            Some("incoming".to_string()),
            Some(1),
            Some(next_page_token),
        ))
        .await?
        .into_inner();

    assert_eq!(second_page.requests.len(), 1);
    assert_eq!(second_page.requests[0].friend_request_id, request_id_1.to_string());
    assert!(second_page.next_page_token.is_none());

    friendship_env.shutdown().await;
    identity_env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn lists_pending_requests_rejects_bad_direction()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let actor_user_id = uuid::Uuid::new_v4();
    let (identity_env, identity) = FakeIdentityEnv::start([]).await?;
    let db_env = TestDbEnv::start().await?;
    let (friendship_env, mut client) = connect_friendship(db_env.db.clone(), identity).await?;

    let error = client
        .list_pending_requests(list_pending_requests_request(
            actor_user_id,
            Some("sideways".to_string()),
            Some(1),
            None,
        ))
        .await
        .expect_err("bad direction should fail");

    assert_eq!(error.code(), tonic::Code::InvalidArgument);
    assert_eq!(error.message(), "Invalid direction");

    friendship_env.shutdown().await;
    identity_env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn lists_pending_requests_outgoing_only()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let actor_user_id = uuid::Uuid::new_v4();
    let target_user_id = uuid::Uuid::new_v4();
    let request_id = uuid::Uuid::new_v4();
    let (identity_env, identity) = FakeIdentityEnv::start([target_user_id]).await?;
    let db_env = TestDbEnv::start().await?;
    let (friendship_env, mut client) = connect_friendship(db_env.db.clone(), identity).await?;

    friend_request::Entity::insert(friend_request::ActiveModel {
        friend_request_id: Set(request_id),
        requester_user_id: Set(actor_user_id),
        addressee_user_id: Set(target_user_id),
        status: Set("pending".to_string()),
        created_at: Set(chrono::Utc::now().into()),
        resolved_at: Set(None),
        resolution_reason: Set(None),
    })
    .exec(&db_env.db)
    .await?;

    let response = client
        .list_pending_requests(list_pending_requests_request(
            actor_user_id,
            Some("outgoing".to_string()),
            Some(20),
            None,
        ))
        .await?
        .into_inner();

    assert_eq!(response.requests.len(), 1);
    assert_eq!(response.requests[0].friend_request_id, request_id.to_string());
    assert!(response.next_page_token.is_none());

    friendship_env.shutdown().await;
    identity_env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn lists_pending_requests_all_direction_returns_both_sides()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let actor_user_id = uuid::Uuid::new_v4();
    let incoming_user = uuid::Uuid::new_v4();
    let outgoing_user = uuid::Uuid::new_v4();
    let incoming_request_id = uuid::Uuid::new_v4();
    let outgoing_request_id = uuid::Uuid::new_v4();
    let (identity_env, identity) = FakeIdentityEnv::start([incoming_user, outgoing_user]).await?;
    let db_env = TestDbEnv::start().await?;
    let (friendship_env, mut client) = connect_friendship(db_env.db.clone(), identity).await?;

    let now = chrono::Utc::now();
    friend_request::Entity::insert_many([
        friend_request::ActiveModel {
            friend_request_id: Set(incoming_request_id),
            requester_user_id: Set(incoming_user),
            addressee_user_id: Set(actor_user_id),
            status: Set("pending".to_string()),
            created_at: Set((now - chrono::Duration::minutes(2)).into()),
            resolved_at: Set(None),
            resolution_reason: Set(None),
        },
        friend_request::ActiveModel {
            friend_request_id: Set(outgoing_request_id),
            requester_user_id: Set(actor_user_id),
            addressee_user_id: Set(outgoing_user),
            status: Set("pending".to_string()),
            created_at: Set((now - chrono::Duration::minutes(1)).into()),
            resolved_at: Set(None),
            resolution_reason: Set(None),
        },
    ])
    .exec(&db_env.db)
    .await?;

    let response = client
        .list_pending_requests(list_pending_requests_request(
            actor_user_id,
            Some("all".to_string()),
            Some(20),
            None,
        ))
        .await?
        .into_inner();

    assert_eq!(response.requests.len(), 2);
    assert_eq!(response.requests[0].friend_request_id, outgoing_request_id.to_string());
    assert_eq!(response.requests[1].friend_request_id, incoming_request_id.to_string());
    assert!(response.next_page_token.is_none());

    friendship_env.shutdown().await;
    identity_env.shutdown().await;
    Ok(())
}
