use relay_proto::{
    realtime::realtime_service_client::RealtimeServiceClient,
    workspace::workspace_service_client::WorkspaceServiceClient,
};
use tonic::transport::Channel;

use crate::config::Config;

#[derive(Clone)]
pub struct Clients {
    pub workspace: WorkspaceServiceClient<Channel>,
    pub realtime: RealtimeServiceClient<Channel>,
}

impl Clients {
    pub async fn connect(config: &Config) -> Result<Self, tonic::transport::Error> {
        let workspace =
            WorkspaceServiceClient::connect(config.workspace_service_url.clone()).await?;
        let realtime = RealtimeServiceClient::connect(config.realtime_service_url.clone()).await?;

        Ok(Self {
            workspace,
            realtime,
        })
    }
}
