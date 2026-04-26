use relay_proto::bootstrap::{
    GetAppBootstrapRequest, GetAppBootstrapResponse, GetDmBootstrapRequest, GetDmBootstrapResponse,
    GetWorkspaceBootstrapRequest, GetWorkspaceBootstrapResponse,
    bootstrap_service_server::{BootstrapService, BootstrapServiceServer},
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

    pub fn into_server(self) -> BootstrapServiceServer<Self> {
        BootstrapServiceServer::new(self)
    }
}

#[tonic::async_trait]
impl BootstrapService for Handler {
    async fn get_app_bootstrap(
        &self,
        request: Request<GetAppBootstrapRequest>,
    ) -> Result<Response<GetAppBootstrapResponse>, Status> {
        self.get_app_bootstrap(request).await
    }

    async fn get_workspace_bootstrap(
        &self,
        request: Request<GetWorkspaceBootstrapRequest>,
    ) -> Result<Response<GetWorkspaceBootstrapResponse>, Status> {
        self.get_workspace_bootstrap(request).await
    }

    async fn get_dm_bootstrap(
        &self,
        request: Request<GetDmBootstrapRequest>,
    ) -> Result<Response<GetDmBootstrapResponse>, Status> {
        self.get_dm_bootstrap(request).await
    }
}
