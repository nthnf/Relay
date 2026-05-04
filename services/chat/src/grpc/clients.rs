use relay_proto::{
    realtime::realtime_service_client::RealtimeServiceClient,
    workspace::workspace_service_client::WorkspaceServiceClient,
};
use std::time::Duration;
use tokio::time::sleep;
use tonic::transport::Channel;
use tracing::warn;

use crate::config::Config;

const CONNECT_RETRY_DELAY: Duration = Duration::from_secs(2);
const CONNECT_MAX_RETRIES: usize = 30;

#[derive(Clone)]
pub struct Clients {
    pub workspace: WorkspaceServiceClient<Channel>,
    pub realtime: RealtimeServiceClient<Channel>,
}

impl Clients {
    pub async fn connect(config: &Config) -> Result<Self, tonic::transport::Error> {
        let workspace = connect_workspace(&config.workspace_service_url).await?;
        let realtime = connect_realtime(&config.realtime_service_url).await?;

        Ok(Self {
            workspace,
            realtime,
        })
    }
}

async fn connect_workspace(
    url: &str,
) -> Result<WorkspaceServiceClient<Channel>, tonic::transport::Error> {
    let mut last_error = None;

    for attempt in 1..=CONNECT_MAX_RETRIES {
        match WorkspaceServiceClient::connect(url.to_owned()).await {
            Ok(client) => return Ok(client),
            Err(error) => {
                warn!(attempt, error = %error, "workspace grpc connection failed; retrying");
                last_error = Some(error);
                sleep(CONNECT_RETRY_DELAY).await;
            }
        }
    }

    Err(last_error.expect("workspace grpc connection should have been attempted"))
}

async fn connect_realtime(
    url: &str,
) -> Result<RealtimeServiceClient<Channel>, tonic::transport::Error> {
    let mut last_error = None;

    for attempt in 1..=CONNECT_MAX_RETRIES {
        match RealtimeServiceClient::connect(url.to_owned()).await {
            Ok(client) => return Ok(client),
            Err(error) => {
                warn!(attempt, error = %error, "realtime grpc connection failed; retrying");
                last_error = Some(error);
                sleep(CONNECT_RETRY_DELAY).await;
            }
        }
    }

    Err(last_error.expect("realtime grpc connection should have been attempted"))
}
