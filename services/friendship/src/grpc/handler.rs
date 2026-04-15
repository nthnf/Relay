use relay_proto::friendship::friendship_service_server::{
    FriendshipService, FriendshipServiceServer,
};
use sea_orm::DatabaseConnection;
use tonic::{Request, Status};

#[derive(Clone)]
pub struct Handler {
    pub(crate) connection: DatabaseConnection,
}

impl Handler {
    pub fn new(connection: DatabaseConnection) -> Self {
        Self { connection }
    }

    pub fn into_server(self) -> FriendshipServiceServer<Self> {
        FriendshipServiceServer::new(self)
    }
}

#[tonic::async_trait]
impl FriendshipService for Handler {
    async fn create_friend_request(
        &self,
        request: Request<relay_proto::friendship::CreateFriendRequestRequest>,
    ) -> Result<tonic::Response<relay_proto::friendship::FriendRequestRecord>, Status> {
        self.create_friend_request(request).await
    }

    async fn accept_friend_request(
        &self,
        request: Request<relay_proto::friendship::AcceptFriendRequestRequest>,
    ) -> Result<tonic::Response<relay_proto::friendship::AcceptFriendRequestResponse>, Status> {
        self.accept_friend_request(request).await
    }

    async fn reject_friend_request(
        &self,
        request: Request<relay_proto::friendship::RejectFriendRequestRequest>,
    ) -> Result<tonic::Response<relay_proto::friendship::RejectFriendRequestResponse>, Status> {
        self.reject_friend_request(request).await
    }

    async fn remove_friend(
        &self,
        request: Request<relay_proto::friendship::RemoveFriendRequest>,
    ) -> Result<tonic::Response<relay_proto::friendship::RemoveFriendResponse>, Status> {
        self.remove_friend(request).await
    }

    async fn block_user(
        &self,
        request: Request<relay_proto::friendship::BlockUserRequest>,
    ) -> Result<tonic::Response<relay_proto::friendship::BlockUserResponse>, Status> {
        self.block_user(request).await
    }

    async fn unblock_user(
        &self,
        request: Request<relay_proto::friendship::UnblockUserRequest>,
    ) -> Result<tonic::Response<relay_proto::friendship::UnblockUserResponse>, Status> {
        self.unblock_user(request).await
    }

    async fn list_friends(
        &self,
        request: Request<relay_proto::friendship::ListFriendsRequest>,
    ) -> Result<tonic::Response<relay_proto::friendship::ListFriendsResponse>, Status> {
        self.list_friends(request).await
    }

    async fn list_pending_requests(
        &self,
        request: Request<relay_proto::friendship::ListPendingRequestsRequest>,
    ) -> Result<tonic::Response<relay_proto::friendship::ListPendingRequestsResponse>, Status> {
        self.list_pending_requests(request).await
    }
}
