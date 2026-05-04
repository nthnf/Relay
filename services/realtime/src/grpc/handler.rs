use crate::{redis::RedisStore, store::Store};
use relay_proto::realtime::realtime_service_server::{RealtimeService, RealtimeServiceServer};
use std::sync::Arc;
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct Handler {
    pub(crate) store: Arc<Store>,
    pub(crate) redis: Option<Arc<RedisStore>>,
}

impl Handler {
    pub fn new(store: Arc<Store>) -> Self {
        Self { store, redis: None }
    }

    pub fn with_redis(store: Arc<Store>, redis: Arc<RedisStore>) -> Self {
        Self {
            store,
            redis: Some(redis),
        }
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

    async fn get_user_presence(
        &self,
        request: Request<relay_proto::realtime::GetUserPresenceRequest>,
    ) -> Result<Response<relay_proto::realtime::GetUserPresenceResponse>, Status> {
        self.get_user_presence(request).await
    }
}
