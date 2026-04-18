use crate::store::Store;
use relay_proto::realtime::realtime_service_server::{RealtimeService, RealtimeServiceServer};
use std::sync::Arc;
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct Handler {
    pub(crate) store: Arc<Store>,
}

impl Handler {
    pub fn new(store: Arc<Store>) -> Self {
        Self { store }
    }

    pub fn into_server(self) -> RealtimeServiceServer<Self> {
        RealtimeServiceServer::new(self)
    }
}

#[tonic::async_trait]
impl RealtimeService for Handler {
    async fn deliver_message(
        &self,
        request: Request<relay_proto::realtime::DeliverMessageRequest>,
    ) -> Result<Response<relay_proto::realtime::DeliverMessageResponse>, Status> {
        self.deliver_message(request).await
    }

    async fn disconnect_actor_sessions(
        &self,
        request: Request<relay_proto::realtime::DisconnectActorSessionsRequest>,
    ) -> Result<Response<relay_proto::realtime::DisconnectActorSessionsResponse>, Status> {
        self.disconnect_actor_sessions(request).await
    }
}
