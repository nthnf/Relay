use std::{collections::HashSet, sync::Arc};

use friendship::{
    db,
    grpc::{FriendshipServer, client::IdentityClient},
};
use migration::{Migrator, MigratorTrait};
use relay_proto::friendship::friendship_service_client::FriendshipServiceClient;
use relay_proto::friendship::{
    AcceptFriendRequestRequest, BlockUserRequest, CreateFriendRequestRequest, ListFriendsRequest,
    ListPendingRequestsRequest, RejectFriendRequestRequest, RemoveFriendRequest,
    UnblockUserRequest,
};
use relay_proto::identity::identity_service_server::{IdentityService, IdentityServiceServer};
use relay_proto::identity::{
    AuthenticatePasswordRequest, GetUserProfileRequest, GetUserProfileResponse,
    GetUsersByIdsRequest, GetUsersByIdsResponse, RedeemEmailVerificationTokenRequest,
    RefreshSessionRequest, RegisterUserRequest, RegisterUserResponse,
    ResendVerificationEmailRequest, ResendVerificationEmailResponse, RevokeSessionRequest,
    RevokeSessionResponse, TokenPairResponse, UpdateUserProfileRequest, UpdateUserProfileResponse,
    UserProfile,
};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{core::IntoContainerPort, runners::AsyncRunner},
};
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub const ACTOR_USER_ID_METADATA: &str = "x-user-id";

#[derive(Clone)]
pub struct FakeIdentityService {
    known_users: Arc<HashSet<Uuid>>,
}

impl FakeIdentityService {
    pub fn new(known_users: impl IntoIterator<Item = Uuid>) -> Self {
        Self {
            known_users: Arc::new(known_users.into_iter().collect()),
        }
    }
}

#[tonic::async_trait]
impl IdentityService for FakeIdentityService {
    async fn register_user(
        &self,
        _request: Request<RegisterUserRequest>,
    ) -> Result<Response<RegisterUserResponse>, Status> {
        Err(Status::unimplemented("not used"))
    }

    async fn authenticate_password(
        &self,
        _request: Request<AuthenticatePasswordRequest>,
    ) -> Result<Response<TokenPairResponse>, Status> {
        Err(Status::unimplemented("not used"))
    }

    async fn refresh_session(
        &self,
        _request: Request<RefreshSessionRequest>,
    ) -> Result<Response<TokenPairResponse>, Status> {
        Err(Status::unimplemented("not used"))
    }

    async fn revoke_session(
        &self,
        _request: Request<RevokeSessionRequest>,
    ) -> Result<Response<RevokeSessionResponse>, Status> {
        Err(Status::unimplemented("not used"))
    }

    async fn redeem_email_verification_token(
        &self,
        _request: Request<RedeemEmailVerificationTokenRequest>,
    ) -> Result<Response<TokenPairResponse>, Status> {
        Err(Status::unimplemented("not used"))
    }

    async fn resend_verification_email(
        &self,
        _request: Request<ResendVerificationEmailRequest>,
    ) -> Result<Response<ResendVerificationEmailResponse>, Status> {
        Err(Status::unimplemented("not used"))
    }

    async fn update_user_profile(
        &self,
        _request: Request<UpdateUserProfileRequest>,
    ) -> Result<Response<UpdateUserProfileResponse>, Status> {
        Err(Status::unimplemented("not used"))
    }

    async fn get_user_profile(
        &self,
        _request: Request<GetUserProfileRequest>,
    ) -> Result<Response<GetUserProfileResponse>, Status> {
        Err(Status::unimplemented("not used"))
    }

    async fn get_users_by_ids(
        &self,
        request: Request<GetUsersByIdsRequest>,
    ) -> Result<Response<GetUsersByIdsResponse>, Status> {
        let users = request
            .into_inner()
            .user_ids
            .into_iter()
            .filter_map(|user_id| {
                Uuid::parse_str(&user_id).ok().and_then(|parsed_user_id| {
                    if self.known_users.contains(&parsed_user_id) {
                        Some(UserProfile {
                            user_id: user_id.clone(),
                            username: format!("user-{user_id}"),
                            display_name: format!("User {user_id}"),
                            avatar_url: None,
                        })
                    } else {
                        None
                    }
                })
            })
            .collect();

        Ok(Response::new(GetUsersByIdsResponse { users }))
    }
}

pub struct FakeIdentityEnv {
    _server_task: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl FakeIdentityEnv {
    pub async fn start(
        known_users: impl IntoIterator<Item = Uuid>,
    ) -> Result<(Self, IdentityClient), Box<dyn std::error::Error + Send + Sync>> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        drop(listener);

        let service = FakeIdentityService::new(known_users);
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        let server_task = tokio::spawn(async move {
            Server::builder()
                .add_service(IdentityServiceServer::new(service))
                .serve_with_shutdown(addr, async {
                    let _ = shutdown_rx.await;
                })
                .await
        });

        let endpoint = format!("http://{addr}");
        let mut client = None;
        let mut last_error: Option<Box<dyn std::error::Error + Send + Sync>> = None;
        for _ in 0..20 {
            match IdentityClient::connect(endpoint.clone()).await {
                Ok(connected) => {
                    client = Some(connected);
                    break;
                }
                Err(error) => {
                    last_error = Some(error);
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
            }
        }

        let client = match client {
            Some(client) => client,
            None => {
                return Err(last_error.expect("connect loop should set error"));
            }
        };

        Ok((
            Self {
                _server_task: server_task,
                shutdown: Some(shutdown_tx),
            },
            client,
        ))
    }

    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }

        let _ = self._server_task.await;
    }
}

pub struct TestDbEnv {
    _postgres: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    pub db: sea_orm::DatabaseConnection,
}

impl TestDbEnv {
    pub async fn start() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let postgres = Postgres::default().start().await?;
        let postgres_host = postgres.get_host().await?;
        let postgres_port = postgres.get_host_port_ipv4(5432.tcp()).await?;
        let database_url =
            format!("postgres://postgres:postgres@{postgres_host}:{postgres_port}/postgres");

        let db = db::connect(&database_url).await?;
        Migrator::up(&db, None).await?;

        Ok(Self {
            _postgres: postgres,
            db,
        })
    }
}

pub struct FriendshipEnv {
    _server_task: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl FriendshipEnv {
    pub async fn start(
        db: sea_orm::DatabaseConnection,
        identity: IdentityClient,
    ) -> Result<
        (Self, FriendshipServiceClient<tonic::transport::Channel>),
        Box<dyn std::error::Error + Send + Sync>,
    > {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        drop(listener);

        let service = FriendshipServer::new(db, identity);
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        let server_task = tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(service.into_server())
                .serve_with_shutdown(addr, async {
                    let _ = shutdown_rx.await;
                })
                .await
        });

        let endpoint = format!("http://{addr}");
        let mut client = None;
        let mut last_error: Option<tonic::transport::Error> = None;
        for _ in 0..20 {
            match FriendshipServiceClient::connect(endpoint.clone()).await {
                Ok(connected) => {
                    client = Some(connected);
                    break;
                }
                Err(error) => {
                    last_error = Some(error);
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
            }
        }

        let client = match client {
            Some(client) => client,
            None => {
                return Err(Box::new(last_error.expect("connect loop should set error"))
                    as Box<dyn std::error::Error + Send + Sync>);
            }
        };

        Ok((
            Self {
                _server_task: server_task,
                shutdown: Some(shutdown_tx),
            },
            client,
        ))
    }

    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }

        let _ = self._server_task.await;
    }
}

#[allow(dead_code)]
pub fn create_request(
    actor_user_id: Uuid,
    target_user_id: Uuid,
) -> tonic::Request<CreateFriendRequestRequest> {
    let mut request = tonic::Request::new(CreateFriendRequestRequest {
        target_user_id: target_user_id.to_string(),
    });
    request.metadata_mut().insert(
        ACTOR_USER_ID_METADATA,
        actor_user_id
            .to_string()
            .parse()
            .expect("uuid metadata should be valid"),
    );
    request
}

#[allow(dead_code)]
pub fn accept_request(
    actor_user_id: Uuid,
    friend_request_id: Uuid,
) -> tonic::Request<AcceptFriendRequestRequest> {
    let mut request = tonic::Request::new(AcceptFriendRequestRequest {
        friend_request_id: friend_request_id.to_string(),
    });
    request.metadata_mut().insert(
        ACTOR_USER_ID_METADATA,
        actor_user_id
            .to_string()
            .parse()
            .expect("uuid metadata should be valid"),
    );
    request
}

#[allow(dead_code)]
pub fn reject_request(
    actor_user_id: Uuid,
    friend_request_id: Uuid,
) -> tonic::Request<RejectFriendRequestRequest> {
    let mut request = tonic::Request::new(RejectFriendRequestRequest {
        friend_request_id: friend_request_id.to_string(),
    });
    request.metadata_mut().insert(
        ACTOR_USER_ID_METADATA,
        actor_user_id
            .to_string()
            .parse()
            .expect("uuid metadata should be valid"),
    );
    request
}

#[allow(dead_code)]
pub fn remove_request(
    actor_user_id: Uuid,
    friend_user_id: Uuid,
) -> tonic::Request<RemoveFriendRequest> {
    let mut request = tonic::Request::new(RemoveFriendRequest {
        friend_user_id: friend_user_id.to_string(),
    });
    request.metadata_mut().insert(
        ACTOR_USER_ID_METADATA,
        actor_user_id
            .to_string()
            .parse()
            .expect("uuid metadata should be valid"),
    );
    request
}

#[allow(dead_code)]
pub fn block_request(
    actor_user_id: Uuid,
    target_user_id: Uuid,
) -> tonic::Request<BlockUserRequest> {
    let mut request = tonic::Request::new(BlockUserRequest {
        target_user_id: target_user_id.to_string(),
    });
    request.metadata_mut().insert(
        ACTOR_USER_ID_METADATA,
        actor_user_id
            .to_string()
            .parse()
            .expect("uuid metadata should be valid"),
    );
    request
}

#[allow(dead_code)]
pub fn unblock_request(
    actor_user_id: Uuid,
    target_user_id: Uuid,
) -> tonic::Request<UnblockUserRequest> {
    let mut request = tonic::Request::new(UnblockUserRequest {
        target_user_id: target_user_id.to_string(),
    });
    request.metadata_mut().insert(
        ACTOR_USER_ID_METADATA,
        actor_user_id
            .to_string()
            .parse()
            .expect("uuid metadata should be valid"),
    );
    request
}

#[allow(dead_code)]
pub fn list_friends_request(
    actor_user_id: Uuid,
    page_size: Option<i32>,
    page_token: Option<String>,
) -> tonic::Request<ListFriendsRequest> {
    let mut request = tonic::Request::new(ListFriendsRequest {
        page_size,
        page_token,
    });
    request.metadata_mut().insert(
        ACTOR_USER_ID_METADATA,
        actor_user_id
            .to_string()
            .parse()
            .expect("uuid metadata should be valid"),
    );
    request
}

#[allow(dead_code)]
pub fn list_pending_requests_request(
    actor_user_id: Uuid,
    direction: Option<String>,
    page_size: Option<i32>,
    page_token: Option<String>,
) -> tonic::Request<ListPendingRequestsRequest> {
    let mut request = tonic::Request::new(ListPendingRequestsRequest {
        direction,
        page_size,
        page_token,
    });
    request.metadata_mut().insert(
        ACTOR_USER_ID_METADATA,
        actor_user_id
            .to_string()
            .parse()
            .expect("uuid metadata should be valid"),
    );
    request
}

pub async fn connect_friendship(
    db: sea_orm::DatabaseConnection,
    identity: IdentityClient,
) -> Result<
    (
        FriendshipEnv,
        FriendshipServiceClient<tonic::transport::Channel>,
    ),
    Box<dyn std::error::Error + Send + Sync>,
> {
    FriendshipEnv::start(db, identity).await
}
