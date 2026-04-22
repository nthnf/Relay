extern crate chat as chat_crate;

mod setup;

use relay_proto::chat::chat_service_client::ChatServiceClient;
use relay_proto::chat::ListConversationMessagesRequest;
use sea_orm::{Database, EntityTrait};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{core::IntoContainerPort, runners::AsyncRunner},
};
use tonic::{Code, Request, metadata::MetadataValue, transport::Server};
use uuid::Uuid;

use chat_crate::entity::{
    chat_message, conversation, dm_pair, user_snapshot, workspace_channel_snapshot,
    workspace_snapshot,
};
use chat_crate::grpc::ChatServer;
use migration::{Migrator, MigratorTrait};

const ACTOR_USER_ID_METADATA: &str = "x-user-id";

#[tokio::test]
async fn list_conversation_messages_paginates_channel_messages()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let actor_user_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    let conversation_id = Uuid::new_v4();

    insert_user_snapshot(&env.db, actor_user_id).await?;
    insert_workspace_snapshot(&env.db, workspace_id).await?;
    insert_workspace_channel_snapshot(&env.db, channel_id, workspace_id).await?;
    insert_conversation(&env.db, conversation_id, None, Some(channel_id), "channel").await?;
    insert_message(&env.db, conversation_id, actor_user_id, 1, "one").await?;
    insert_message(&env.db, conversation_id, actor_user_id, 2, "two").await?;
    insert_message(&env.db, conversation_id, actor_user_id, 3, "three").await?;

    let response = env
        .client
        .clone()
        .list_conversation_messages(actor_request(
            actor_user_id,
            ListConversationMessagesRequest {
                conversation_id: conversation_id.to_string(),
                page_size: Some(2),
                before_conversation_message_seq: None,
            },
        ))
        .await?
        .into_inner();

    assert_eq!(response.messages.len(), 2);
    assert_eq!(response.messages[0].conversation_message_seq, 3);
    assert_eq!(response.messages[0].body, "three");
    assert_eq!(response.messages[1].conversation_message_seq, 2);
    assert_eq!(response.messages[1].body, "two");
    assert_eq!(response.next_before_conversation_message_seq, Some(2));

    let next = env
        .client
        .clone()
        .list_conversation_messages(actor_request(
            actor_user_id,
            ListConversationMessagesRequest {
                conversation_id: conversation_id.to_string(),
                page_size: Some(2),
                before_conversation_message_seq: response.next_before_conversation_message_seq,
            },
        ))
        .await?
        .into_inner();

    assert_eq!(next.messages.len(), 1);
    assert_eq!(next.messages[0].conversation_message_seq, 1);
    assert_eq!(next.messages[0].body, "one");
    assert_eq!(next.next_before_conversation_message_seq, None);

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn list_conversation_messages_rejects_non_dm_member()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let actor_user_id = Uuid::new_v4();
    let peer_user_id = Uuid::new_v4();
    let outsider_user_id = Uuid::new_v4();
    let dm_pair_id = Uuid::new_v4();
    let conversation_id = Uuid::new_v4();

    insert_user_snapshot(&env.db, actor_user_id).await?;
    insert_user_snapshot(&env.db, peer_user_id).await?;
    insert_user_snapshot(&env.db, outsider_user_id).await?;
    insert_dm_pair(&env.db, dm_pair_id, actor_user_id, peer_user_id).await?;
    insert_conversation(&env.db, conversation_id, Some(dm_pair_id), None, "dm").await?;
    insert_message(&env.db, conversation_id, actor_user_id, 1, "hi").await?;

    let err = env
        .client
        .clone()
        .list_conversation_messages(actor_request(
            outsider_user_id,
            ListConversationMessagesRequest {
                conversation_id: conversation_id.to_string(),
                page_size: Some(20),
                before_conversation_message_seq: None,
            },
        ))
        .await
        .unwrap_err();

    assert_eq!(err.code(), Code::PermissionDenied);

    env.shutdown().await;
    Ok(())
}

struct TestEnv {
    _postgres: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    db: sea_orm::DatabaseConnection,
    client: ChatServiceClient<tonic::transport::Channel>,
    mocks: setup::MockServers,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    server_task: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
}

impl TestEnv {
    async fn start() -> Result<Self, Box<dyn std::error::Error>> {
        let postgres = Postgres::default().start().await?;
        let postgres_host = postgres.get_host().await?;
        let postgres_port = postgres.get_host_port_ipv4(5432.tcp()).await?;
        let database_url =
            format!("postgres://postgres:postgres@{postgres_host}:{postgres_port}/postgres");

        let db = Database::connect(&database_url).await?;
        Migrator::up(&db, None).await?;

        let (clients, mocks) = setup::start_clients().await?;

        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        drop(listener);

        let handler = ChatServer::with_clients(db.clone(), clients);
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        let server_task = tokio::spawn(async move {
            Server::builder()
                .add_service(handler.into_server())
                .serve_with_shutdown(addr, async {
                    let _ = shutdown_rx.await;
                })
                .await
        });

        let client = connect_client(addr).await?;

        Ok(Self {
            _postgres: postgres,
            db,
            client,
            mocks,
            shutdown: Some(shutdown_tx),
            server_task,
        })
    }

    async fn shutdown(mut self) {
        if let Some(sender) = self.shutdown.take() {
            let _ = sender.send(());
        }

        let _ = self.server_task.await;
        self.mocks.shutdown().await;
    }
}

async fn connect_client(
    addr: std::net::SocketAddr,
) -> Result<ChatServiceClient<tonic::transport::Channel>, Box<dyn std::error::Error>> {
    let endpoint = format!("http://{addr}");

    for _ in 0..20 {
        match ChatServiceClient::connect(endpoint.clone()).await {
            Ok(client) => return Ok(client),
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(50)).await,
        }
    }

    Ok(ChatServiceClient::connect(endpoint).await?)
}

fn actor_request<T>(user_id: Uuid, request: T) -> Request<T> {
    let mut request = Request::new(request);
    request.metadata_mut().insert(
        ACTOR_USER_ID_METADATA,
        MetadataValue::try_from(user_id.to_string()).expect("metadata"),
    );
    request
}

async fn insert_user_snapshot(
    db: &sea_orm::DatabaseConnection,
    user_id: Uuid,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = chrono::Utc::now();
    chat_crate::entity::user_snapshot::Entity::insert(user_snapshot::ActiveModel {
        user_id: sea_orm::Set(user_id),
        created_at: sea_orm::Set(now.into()),
        updated_at: sea_orm::Set(now.into()),
    })
    .exec(db)
    .await?;

    Ok(())
}

async fn insert_workspace_snapshot(
    db: &sea_orm::DatabaseConnection,
    workspace_id: Uuid,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = chrono::Utc::now();
    chat_crate::entity::workspace_snapshot::Entity::insert(workspace_snapshot::ActiveModel {
        workspace_id: sea_orm::Set(workspace_id),
        created_at: sea_orm::Set(now.into()),
        updated_at: sea_orm::Set(now.into()),
    })
    .exec(db)
    .await?;

    Ok(())
}

async fn insert_workspace_channel_snapshot(
    db: &sea_orm::DatabaseConnection,
    channel_id: Uuid,
    workspace_id: Uuid,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = chrono::Utc::now();
    chat_crate::entity::workspace_channel_snapshot::Entity::insert(
        workspace_channel_snapshot::ActiveModel {
            workspace_channel_id: sea_orm::Set(channel_id),
            workspace_id: sea_orm::Set(workspace_id),
            channel_kind: sea_orm::Set("text".to_string()),
            created_at: sea_orm::Set(now.into()),
            updated_at: sea_orm::Set(now.into()),
        },
    )
    .exec(db)
    .await?;

    Ok(())
}

async fn insert_dm_pair(
    db: &sea_orm::DatabaseConnection,
    dm_pair_id: Uuid,
    user_a: Uuid,
    user_b: Uuid,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = chrono::Utc::now();
    let (low_user_id, high_user_id) = if user_a < user_b {
        (user_a, user_b)
    } else {
        (user_b, user_a)
    };

    chat_crate::entity::dm_pair::Entity::insert(dm_pair::ActiveModel {
        id: sea_orm::Set(dm_pair_id),
        low_user_id: sea_orm::Set(low_user_id),
        high_user_id: sea_orm::Set(high_user_id),
        created_at: sea_orm::Set(now.into()),
    })
    .exec(db)
    .await?;

    Ok(())
}

async fn insert_conversation(
    db: &sea_orm::DatabaseConnection,
    conversation_id: Uuid,
    dm_pair_id: Option<Uuid>,
    workspace_channel_id: Option<Uuid>,
    target_type: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = chrono::Utc::now();
    chat_crate::entity::conversation::Entity::insert(conversation::ActiveModel {
        id: sea_orm::Set(conversation_id),
        target_type: sea_orm::Set(target_type.to_string()),
        dm_pair_id: sea_orm::Set(dm_pair_id),
        workspace_channel_id: sea_orm::Set(workspace_channel_id),
        created_at: sea_orm::Set(now.into()),
    })
    .exec(db)
    .await?;

    Ok(())
}

async fn insert_message(
    db: &sea_orm::DatabaseConnection,
    conversation_id: Uuid,
    author_user_id: Uuid,
    seq: i64,
    body: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = chrono::Utc::now();
    chat_crate::entity::chat_message::Entity::insert(chat_message::ActiveModel {
        message_id: sea_orm::Set(Uuid::new_v4()),
        conversation_id: sea_orm::Set(conversation_id),
        author_user_id: sea_orm::Set(author_user_id),
        client_message_id: sea_orm::Set(None),
        conversation_message_seq: sea_orm::Set(seq),
        body: sea_orm::Set(body.to_string()),
        message_status: sea_orm::Set("active".to_string()),
        created_at: sea_orm::Set(now.into()),
        updated_at: sea_orm::Set(now.into()),
        deleted_at: sea_orm::Set(None),
        deleted_by_user_id: sea_orm::Set(None),
        last_edited_at: sea_orm::Set(None),
        last_edited_by_user_id: sea_orm::Set(None),
    })
    .exec(db)
    .await?;

    Ok(())
}
