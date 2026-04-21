use friendship::{
    db,
    entity::{friend_request, friendship_edge, user_block, user_snapshot},
    grpc::FriendshipServer,
};
use migration::{Migrator, MigratorTrait};
use relay_proto::friendship::friendship_service_client::FriendshipServiceClient;
use relay_proto::friendship::{
    AcceptFriendRequestRequest, BlockUserRequest, CreateFriendRequestRequest, ListFriendsRequest,
    ListPendingRequestsRequest, RejectFriendRequestRequest, RemoveFriendRequest,
    UnblockUserRequest,
};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{core::IntoContainerPort, runners::AsyncRunner},
};
use tonic::transport::Server;
use uuid::Uuid;

const ACTOR_USER_ID_METADATA: &str = "x-user-id";

#[tokio::test]
async fn create_friend_request_path() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let actor = Uuid::new_v4();
    let target = Uuid::new_v4();

    env.seed_user(actor, false).await?;
    env.seed_user(target, false).await?;

    let response = env
        .client
        .clone()
        .create_friend_request(actor_request(
            actor,
            CreateFriendRequestRequest {
                target_user_id: target.to_string(),
            },
        ))
        .await?
        .into_inner();

    assert_eq!(response.requester_user_id, actor.to_string());
    assert_eq!(response.addressee_user_id, target.to_string());
    assert_eq!(response.status, "pending");

    let row = friend_request::Entity::find_by_id(Uuid::parse_str(&response.friend_request_id)?)
        .one(&env.db)
        .await?
        .expect("friend request row");
    assert_eq!(row.status, "pending");

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn create_friend_request_rejects_unknown_target() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let actor = Uuid::new_v4();
    let target = Uuid::new_v4();

    let error = env
        .client
        .clone()
        .create_friend_request(actor_request(
            actor,
            CreateFriendRequestRequest {
                target_user_id: target.to_string(),
            },
        ))
        .await
        .expect_err("missing target should be rejected");

    assert_eq!(error.code(), tonic::Code::NotFound);
    assert_eq!(error.message(), "User not found");

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn accept_friend_request_path() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let actor = Uuid::new_v4();
    let requester = Uuid::new_v4();

    env.seed_user(actor, false).await?;
    env.seed_user(requester, false).await?;

    let created = env
        .client
        .clone()
        .create_friend_request(actor_request(
            requester,
            CreateFriendRequestRequest {
                target_user_id: actor.to_string(),
            },
        ))
        .await?
        .into_inner();

    let accepted = env
        .client
        .clone()
        .accept_friend_request(actor_request(
            actor,
            AcceptFriendRequestRequest {
                friend_request_id: created.friend_request_id.clone(),
            },
        ))
        .await?
        .into_inner();

    assert_eq!(accepted.friend_request_id, created.friend_request_id);
    assert_eq!(accepted.addressee_user_id, actor.to_string());
    assert_eq!(accepted.requester_user_id, requester.to_string());

    let edge_count = friendship_edge::Entity::find().all(&env.db).await?.len();
    assert_eq!(edge_count, 2);

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn reject_friend_request_path() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let actor = Uuid::new_v4();
    let requester = Uuid::new_v4();

    env.seed_user(actor, false).await?;
    env.seed_user(requester, false).await?;

    let created = env
        .client
        .clone()
        .create_friend_request(actor_request(
            requester,
            CreateFriendRequestRequest {
                target_user_id: actor.to_string(),
            },
        ))
        .await?
        .into_inner();

    let rejected = env
        .client
        .clone()
        .reject_friend_request(actor_request(
            actor,
            RejectFriendRequestRequest {
                friend_request_id: created.friend_request_id.clone(),
            },
        ))
        .await?
        .into_inner();

    assert_eq!(rejected.friend_request_id, created.friend_request_id);
    assert_eq!(rejected.status, "rejected");

    let row = friend_request::Entity::find_by_id(Uuid::parse_str(&created.friend_request_id)?)
        .one(&env.db)
        .await?
        .expect("friend request row");
    assert_eq!(row.status, "rejected");

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn remove_friend_path() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let actor = Uuid::new_v4();
    let friend = Uuid::new_v4();

    env.seed_user(actor, false).await?;
    env.seed_user(friend, false).await?;

    let created = env
        .client
        .clone()
        .create_friend_request(actor_request(
            actor,
            CreateFriendRequestRequest {
                target_user_id: friend.to_string(),
            },
        ))
        .await?
        .into_inner();

    env.client
        .clone()
        .accept_friend_request(actor_request(
            friend,
            AcceptFriendRequestRequest {
                friend_request_id: created.friend_request_id.clone(),
            },
        ))
        .await?;

    let removed = env
        .client
        .clone()
        .remove_friend(actor_request(
            actor,
            RemoveFriendRequest {
                friend_user_id: friend.to_string(),
            },
        ))
        .await?
        .into_inner();

    assert!(removed.removed);
    assert_eq!(friendship_edge::Entity::find().all(&env.db).await?.len(), 0);

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn block_user_path() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let blocker = Uuid::new_v4();
    let blocked = Uuid::new_v4();

    env.seed_user(blocker, false).await?;
    env.seed_user(blocked, false).await?;

    let created = env
        .client
        .clone()
        .create_friend_request(actor_request(
            blocked,
            CreateFriendRequestRequest {
                target_user_id: blocker.to_string(),
            },
        ))
        .await?
        .into_inner();

    let blocked_response = env
        .client
        .clone()
        .block_user(actor_request(
            blocker,
            BlockUserRequest {
                target_user_id: blocked.to_string(),
            },
        ))
        .await?
        .into_inner();

    assert!(blocked_response.blocked);
    assert!(!blocked_response.already_blocked);
    assert_eq!(user_block::Entity::find().all(&env.db).await?.len(), 1);

    let canceled = friend_request::Entity::find_by_id(Uuid::parse_str(&created.friend_request_id)?)
        .one(&env.db)
        .await?
        .expect("friend request row");
    assert_eq!(canceled.status, "canceled_by_block");

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn unblock_user_path() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let blocker = Uuid::new_v4();
    let blocked = Uuid::new_v4();

    env.seed_user(blocker, false).await?;
    env.seed_user(blocked, false).await?;

    env.client
        .clone()
        .block_user(actor_request(
            blocker,
            BlockUserRequest {
                target_user_id: blocked.to_string(),
            },
        ))
        .await?;

    let unblocked = env
        .client
        .clone()
        .unblock_user(actor_request(
            blocker,
            UnblockUserRequest {
                target_user_id: blocked.to_string(),
            },
        ))
        .await?
        .into_inner();

    assert!(unblocked.unblocked);
    assert_eq!(user_block::Entity::find().all(&env.db).await?.len(), 0);

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn list_friends_path() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let actor = Uuid::new_v4();
    let friend = Uuid::new_v4();

    env.seed_user(actor, false).await?;
    env.seed_user(friend, false).await?;

    let created = env
        .client
        .clone()
        .create_friend_request(actor_request(
            actor,
            CreateFriendRequestRequest {
                target_user_id: friend.to_string(),
            },
        ))
        .await?
        .into_inner();

    env.client
        .clone()
        .accept_friend_request(actor_request(
            friend,
            AcceptFriendRequestRequest {
                friend_request_id: created.friend_request_id,
            },
        ))
        .await?;

    let response = env
        .client
        .clone()
        .list_friends(actor_request(
            actor,
            ListFriendsRequest {
                page_size: Some(20),
                page_token: None,
            },
        ))
        .await?
        .into_inner();

    assert_eq!(response.friends.len(), 1);
    assert_eq!(response.friends[0].friend_user_id, friend.to_string());

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn list_pending_requests_path() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let requester = Uuid::new_v4();
    let addressee = Uuid::new_v4();

    env.seed_user(requester, false).await?;
    env.seed_user(addressee, false).await?;

    env.client
        .clone()
        .create_friend_request(actor_request(
            requester,
            CreateFriendRequestRequest {
                target_user_id: addressee.to_string(),
            },
        ))
        .await?;

    let response = env
        .client
        .clone()
        .list_pending_requests(actor_request(
            addressee,
            ListPendingRequestsRequest {
                direction: Some("incoming".to_string()),
                page_size: Some(20),
                page_token: None,
            },
        ))
        .await?
        .into_inner();

    assert_eq!(response.requests.len(), 1);
    assert_eq!(
        response.requests[0].requester_user_id,
        requester.to_string()
    );

    env.shutdown().await;
    Ok(())
}

struct TestEnv {
    _postgres: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    db: sea_orm::DatabaseConnection,
    client: FriendshipServiceClient<tonic::transport::Channel>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    server_task: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
}

impl TestEnv {
    async fn start() -> Result<Self, Box<dyn std::error::Error>> {
        let postgres = Postgres::default().start().await?;
        let postgres_host = postgres.get_host().await?;
        let postgres_port = postgres.get_host_port_ipv4(5432_u16.tcp()).await?;
        let database_url =
            format!("postgres://postgres:postgres@{postgres_host}:{postgres_port}/postgres");

        let db = db::connect(&database_url).await?;
        Migrator::up(&db, None).await?;

        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        drop(listener);

        let service = FriendshipServer::new(db.clone());
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        let server_task = tokio::spawn(async move {
            Server::builder()
                .add_service(service.into_server())
                .serve_with_shutdown(addr, async {
                    let _ = shutdown_rx.await;
                })
                .await
        });

        let client = connect_client(addr).await?;

        Ok(Self {
            _postgres: postgres,
            db,
            client,
            shutdown: Some(shutdown_tx),
            server_task,
        })
    }

    async fn shutdown(mut self) {
        if let Some(sender) = self.shutdown.take() {
            let _ = sender.send(());
        }

        let _ = self.server_task.await;
    }

    async fn seed_user(
        &self,
        user_id: Uuid,
        email_verified: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        user_snapshot::ActiveModel {
            user_id: Set(user_id),
            email_verified: Set(email_verified),
            username: Set(format!("user-{user_id}")),
            display_name: Set("Test User".to_string()),
            avatar_url: Set(None),
            created_at: Set(chrono::Utc::now().into()),
            updated_at: Set(chrono::Utc::now().into()),
        }
        .insert(&self.db)
        .await?;

        Ok(())
    }
}

async fn connect_client(
    addr: std::net::SocketAddr,
) -> Result<FriendshipServiceClient<tonic::transport::Channel>, Box<dyn std::error::Error>> {
    let endpoint = format!("http://{addr}");

    for _ in 0..20 {
        match FriendshipServiceClient::connect(endpoint.clone()).await {
            Ok(client) => return Ok(client),
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(50)).await,
        }
    }

    Ok(FriendshipServiceClient::connect(endpoint).await?)
}

fn actor_request<T>(actor: Uuid, request: T) -> tonic::Request<T> {
    let mut request = tonic::Request::new(request);
    request.metadata_mut().insert(
        ACTOR_USER_ID_METADATA,
        actor
            .to_string()
            .parse()
            .expect("uuid metadata should be valid"),
    );
    request
}
