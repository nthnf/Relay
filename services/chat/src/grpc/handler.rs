use super::clients::Clients;
use relay_proto::chat::{
    CreateConversationRequest, CreateConversationResponse, CreateMessageRequest,
    CreateMessageResponse, DeleteMessageRequest, DeleteMessageResponse, EditMessageRequest,
    EditMessageResponse, ListConversationMessagesRequest, ListMessagesResponse,
    chat_service_server::{ChatService, ChatServiceServer},
};
use sea_orm::DatabaseConnection;
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct Handler {
    pub(crate) connection: DatabaseConnection,
    pub(crate) clients: Clients,
}

impl Handler {
    pub fn new(connection: DatabaseConnection, clients: Clients) -> Self {
        Self { connection, clients }
    }

    pub fn with_clients(connection: DatabaseConnection, clients: Clients) -> Self {
        Self::new(connection, clients)
    }

    pub fn into_server(self) -> ChatServiceServer<Self> {
        ChatServiceServer::new(self)
    }
}

#[tonic::async_trait]
impl ChatService for Handler {
    async fn create_message(
        &self,
        request: Request<CreateMessageRequest>,
    ) -> Result<Response<CreateMessageResponse>, Status> {
        self.create_message(request).await
    }

    async fn edit_message(
        &self,
        request: Request<EditMessageRequest>,
    ) -> Result<Response<EditMessageResponse>, Status> {
        self.edit_message(request).await
    }

    async fn delete_message(
        &self,
        request: Request<DeleteMessageRequest>,
    ) -> Result<Response<DeleteMessageResponse>, Status> {
        self.delete_message(request).await
    }

    async fn list_conversation_messages(
        &self,
        request: Request<ListConversationMessagesRequest>,
    ) -> Result<Response<ListMessagesResponse>, Status> {
        self.list_conversation_messages(request).await
    }

    async fn create_conversation(
        &self,
        request: Request<CreateConversationRequest>,
    ) -> Result<Response<CreateConversationResponse>, Status> {
        self.create_conversation(request).await
    }
}
