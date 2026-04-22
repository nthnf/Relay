use chat::grpc::clients::Clients;
use relay_proto::realtime::realtime_service_client::RealtimeServiceClient;
use relay_proto::realtime::realtime_service_server::{RealtimeService, RealtimeServiceServer};
use relay_proto::realtime::{
    DeliverMessageRequest, DeliverMessageResponse, DisconnectActorSessionsRequest,
    DisconnectActorSessionsResponse,
};
use relay_proto::workspace::workspace_service_client::WorkspaceServiceClient;
use relay_proto::workspace::workspace_service_server::{WorkspaceService, WorkspaceServiceServer};
use relay_proto::workspace::{
    AcceptInvitationRequest, AcceptInvitationResponse, AddMemberRequest, AddMemberResponse,
    AuthorizeChannelActionRequest, AuthorizeChannelActionResponse, CreateChannelRequest,
    CreateChannelResponse, CreateInviteLinkRequest, CreateInviteLinkResponse,
    CreateWorkspaceRequest, CreateWorkspaceResponse, GetWorkspaceRequest, GetWorkspaceResponse,
    IssueInvitationRequest, IssueInvitationResponse, JoinWorkspaceByInviteLinkRequest,
    JoinWorkspaceByInviteLinkResponse, ListChannelsRequest, ListChannelsResponse,
    ListWorkspacesForUserRequest, ListWorkspacesForUserResponse, RemoveMemberRequest,
    RemoveMemberResponse, RevokeInviteLinkRequest, RevokeInviteLinkResponse,
};
use tonic::{Request, Response, Status, transport::Server};

pub struct MockServers {
    shutdown: Vec<tokio::sync::oneshot::Sender<()>>,
    tasks: Vec<tokio::task::JoinHandle<Result<(), tonic::transport::Error>>>,
}

impl MockServers {
    pub async fn shutdown(mut self) {
        for sender in self.shutdown.drain(..) {
            let _ = sender.send(());
        }

        for task in self.tasks.drain(..) {
            let _ = task.await;
        }
    }
}

pub async fn start_clients() -> Result<(Clients, MockServers), Box<dyn std::error::Error>> {
    let (workspace_url, workspace_task, workspace_shutdown) = start_workspace_mock().await?;
    let (realtime_url, realtime_task, realtime_shutdown) = start_realtime_mock().await?;

    let workspace = connect_workspace_client(&workspace_url).await?;
    let realtime = connect_realtime_client(&realtime_url).await?;

    Ok((
        Clients { workspace, realtime },
        MockServers {
            shutdown: vec![workspace_shutdown, realtime_shutdown],
            tasks: vec![workspace_task, realtime_task],
        },
    ))
}

async fn start_workspace_mock(
) -> Result<
    (
        String,
        tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
        tokio::sync::oneshot::Sender<()>,
    ),
    Box<dyn std::error::Error>,
> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    drop(listener);

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let server_task = tokio::spawn(async move {
        Server::builder()
            .add_service(WorkspaceServiceServer::new(MockWorkspaceService))
            .serve_with_shutdown(addr, async {
                let _ = shutdown_rx.await;
            })
            .await
    });

    Ok((format!("http://{addr}"), server_task, shutdown_tx))
}

async fn start_realtime_mock(
) -> Result<
    (
        String,
        tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
        tokio::sync::oneshot::Sender<()>,
    ),
    Box<dyn std::error::Error>,
> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    drop(listener);

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let server_task = tokio::spawn(async move {
        Server::builder()
            .add_service(RealtimeServiceServer::new(MockRealtimeService))
            .serve_with_shutdown(addr, async {
                let _ = shutdown_rx.await;
            })
            .await
    });

    Ok((format!("http://{addr}"), server_task, shutdown_tx))
}

async fn connect_workspace_client(
    url: &str,
) -> Result<WorkspaceServiceClient<tonic::transport::Channel>, Box<dyn std::error::Error>> {
    for _ in 0..20 {
        match WorkspaceServiceClient::connect(url.to_string()).await {
            Ok(client) => return Ok(client),
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(50)).await,
        }
    }

    Ok(WorkspaceServiceClient::connect(url.to_string()).await?)
}

async fn connect_realtime_client(
    url: &str,
) -> Result<RealtimeServiceClient<tonic::transport::Channel>, Box<dyn std::error::Error>> {
    for _ in 0..20 {
        match RealtimeServiceClient::connect(url.to_string()).await {
            Ok(client) => return Ok(client),
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(50)).await,
        }
    }

    Ok(RealtimeServiceClient::connect(url.to_string()).await?)
}

#[derive(Clone, Default)]
struct MockWorkspaceService;

#[tonic::async_trait]
impl WorkspaceService for MockWorkspaceService {
    async fn create_workspace(
        &self,
        _request: Request<CreateWorkspaceRequest>,
    ) -> Result<Response<CreateWorkspaceResponse>, Status> {
        Err(Status::unimplemented("mock"))
    }

    async fn get_workspace(
        &self,
        _request: Request<GetWorkspaceRequest>,
    ) -> Result<Response<GetWorkspaceResponse>, Status> {
        Err(Status::unimplemented("mock"))
    }

    async fn list_workspaces_for_user(
        &self,
        _request: Request<ListWorkspacesForUserRequest>,
    ) -> Result<Response<ListWorkspacesForUserResponse>, Status> {
        Err(Status::unimplemented("mock"))
    }

    async fn authorize_channel_action(
        &self,
        _request: Request<AuthorizeChannelActionRequest>,
    ) -> Result<Response<AuthorizeChannelActionResponse>, Status> {
        Ok(Response::new(AuthorizeChannelActionResponse { allowed: true }))
    }

    async fn create_channel(
        &self,
        _request: Request<CreateChannelRequest>,
    ) -> Result<Response<CreateChannelResponse>, Status> {
        Err(Status::unimplemented("mock"))
    }

    async fn list_channels(
        &self,
        _request: Request<ListChannelsRequest>,
    ) -> Result<Response<ListChannelsResponse>, Status> {
        Err(Status::unimplemented("mock"))
    }

    async fn add_member(
        &self,
        _request: Request<AddMemberRequest>,
    ) -> Result<Response<AddMemberResponse>, Status> {
        Err(Status::unimplemented("mock"))
    }

    async fn remove_member(
        &self,
        _request: Request<RemoveMemberRequest>,
    ) -> Result<Response<RemoveMemberResponse>, Status> {
        Err(Status::unimplemented("mock"))
    }

    async fn issue_invitation(
        &self,
        _request: Request<IssueInvitationRequest>,
    ) -> Result<Response<IssueInvitationResponse>, Status> {
        Err(Status::unimplemented("mock"))
    }

    async fn accept_invitation(
        &self,
        _request: Request<AcceptInvitationRequest>,
    ) -> Result<Response<AcceptInvitationResponse>, Status> {
        Err(Status::unimplemented("mock"))
    }

    async fn create_invite_link(
        &self,
        _request: Request<CreateInviteLinkRequest>,
    ) -> Result<Response<CreateInviteLinkResponse>, Status> {
        Err(Status::unimplemented("mock"))
    }

    async fn join_workspace_by_invite_link(
        &self,
        _request: Request<JoinWorkspaceByInviteLinkRequest>,
    ) -> Result<Response<JoinWorkspaceByInviteLinkResponse>, Status> {
        Err(Status::unimplemented("mock"))
    }

    async fn revoke_invite_link(
        &self,
        _request: Request<RevokeInviteLinkRequest>,
    ) -> Result<Response<RevokeInviteLinkResponse>, Status> {
        Err(Status::unimplemented("mock"))
    }
}

#[derive(Clone, Default)]
struct MockRealtimeService;

#[tonic::async_trait]
impl RealtimeService for MockRealtimeService {
    async fn deliver_message(
        &self,
        _request: Request<DeliverMessageRequest>,
    ) -> Result<Response<DeliverMessageResponse>, Status> {
        Ok(Response::new(DeliverMessageResponse {
            accepted: true,
            attempted_recipient_count: 0,
            delivered_session_count: 0,
        }))
    }

    async fn disconnect_actor_sessions(
        &self,
        _request: Request<DisconnectActorSessionsRequest>,
    ) -> Result<Response<DisconnectActorSessionsResponse>, Status> {
        Err(Status::unimplemented("mock"))
    }
}
