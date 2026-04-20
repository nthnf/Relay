use relay_proto::workspace::{
    AcceptInvitationRequest, AcceptInvitationResponse, AddMemberRequest, AddMemberResponse,
    CreateChannelRequest, CreateChannelResponse, CreateInviteLinkRequest, CreateInviteLinkResponse,
    CreateWorkspaceRequest, CreateWorkspaceResponse, GetWorkspaceRequest, GetWorkspaceResponse,
    IssueInvitationRequest, IssueInvitationResponse, JoinWorkspaceByInviteLinkRequest,
    JoinWorkspaceByInviteLinkResponse, ListChannelsRequest, ListChannelsResponse,
    ListWorkspacesForUserRequest, ListWorkspacesForUserResponse, RemoveMemberRequest,
    RemoveMemberResponse, RevokeInviteLinkRequest, RevokeInviteLinkResponse,
    workspace_service_server::{WorkspaceService, WorkspaceServiceServer},
};
use sea_orm::DatabaseConnection;
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct Handler {
    pub(crate) connection: DatabaseConnection,
}

impl Handler {
    pub fn new(connection: DatabaseConnection) -> Self {
        Self { connection }
    }

    pub fn into_server(self) -> WorkspaceServiceServer<Self> {
        WorkspaceServiceServer::new(self)
    }
}

#[tonic::async_trait]
impl WorkspaceService for Handler {
    async fn create_workspace(
        &self,
        request: Request<CreateWorkspaceRequest>,
    ) -> Result<Response<CreateWorkspaceResponse>, Status> {
        self.create_workspace(request).await
    }

    async fn get_workspace(
        &self,
        request: Request<GetWorkspaceRequest>,
    ) -> Result<Response<GetWorkspaceResponse>, Status> {
        self.get_workspace(request).await
    }

    async fn list_workspaces_for_user(
        &self,
        request: Request<ListWorkspacesForUserRequest>,
    ) -> Result<Response<ListWorkspacesForUserResponse>, Status> {
        self.list_workspaces_for_user(request).await
    }

    async fn create_channel(
        &self,
        request: Request<CreateChannelRequest>,
    ) -> Result<Response<CreateChannelResponse>, Status> {
        self.create_channel(request).await
    }

    async fn list_channels(
        &self,
        request: Request<ListChannelsRequest>,
    ) -> Result<Response<ListChannelsResponse>, Status> {
        self.list_channels(request).await
    }

    async fn add_member(
        &self,
        request: Request<AddMemberRequest>,
    ) -> Result<Response<AddMemberResponse>, Status> {
        self.add_member(request).await
    }

    async fn remove_member(
        &self,
        request: Request<RemoveMemberRequest>,
    ) -> Result<Response<RemoveMemberResponse>, Status> {
        self.remove_member(request).await
    }

    async fn issue_invitation(
        &self,
        _request: Request<IssueInvitationRequest>,
    ) -> Result<Response<IssueInvitationResponse>, Status> {
        todo!()
    }

    async fn accept_invitation(
        &self,
        _request: Request<AcceptInvitationRequest>,
    ) -> Result<Response<AcceptInvitationResponse>, Status> {
        todo!()
    }

    async fn create_invite_link(
        &self,
        _request: Request<CreateInviteLinkRequest>,
    ) -> Result<Response<CreateInviteLinkResponse>, Status> {
        todo!()
    }

    async fn join_workspace_by_invite_link(
        &self,
        _request: Request<JoinWorkspaceByInviteLinkRequest>,
    ) -> Result<Response<JoinWorkspaceByInviteLinkResponse>, Status> {
        todo!()
    }

    async fn revoke_invite_link(
        &self,
        _request: Request<RevokeInviteLinkRequest>,
    ) -> Result<Response<RevokeInviteLinkResponse>, Status> {
        todo!()
    }
}
