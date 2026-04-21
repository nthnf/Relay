use relay_proto::chat::chat_service_server::{ChatService, ChatServiceServer};
use sea_orm::DatabaseConnection;

#[derive(Clone)]
pub struct Handler {
    pub(crate) connection: DatabaseConnection,
}

impl Handler {
    pub fn new(connection: DatabaseConnection) -> Self {
        Self { connection }
    }

    pub fn into_server(self) -> ChatServiceServer<Self> {
        ChatServiceServer::new(self)
    }
}
