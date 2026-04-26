extern crate chat as chat_crate;

mod setup;

use chat_crate::entity::{
    chat_message, conversation, conversation_read_cursor, dm_pair, outbox_event, user_snapshot,
};
use chat_crate::grpc::ChatServer;
use migration::{Migrator, MigratorTrait};
use relay_proto::chat::MarkConversationReadRequest;
use relay_proto::chat::chat_service_client::ChatServiceClient;
use sea_orm::{ColumnTrait, Database, EntityTrait, QueryFilter};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{core::IntoContainerPort, runners::AsyncRunner},
};
use tonic::{Code, Request, metadata::MetadataValue, transport::Server};
use uuid::Uuid;

const ACTOR_USER_ID_METADATA: &str = "x-user-id";

#[tokio::test]
async fn mark_conversation_read_inserts_cursor_and_outbox_for_dm()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let actor_user_id = Uuid::new_v4();
    let peer_user_id = Uuid::new_v4();
    let dm_pair_id = Uuid::new_v4();
    let conversation_id = Uuid::new_v4();

    insert_user_snapshot(&env.db, actor_user_id).await?;
    insert_user_snapshot(&env.db, peer_user_id).await?;
    insert_dm_pair(&env.db, dm_pair_id, actor_user_id, peer_user_id).await?;
    insert_conversation(&env.db, conversation_id, Some(dm_pair_id), None, "dm").await?;
    insert_message(&env.db, conversation_id, actor_user_id, 1, "one").await?;
    insert_message(&env.db, conversation_id, peer_user_id, 2, "two").await?;

    let response = env
        .client
        .clone()
        .mark_conversation_read(actor_request(
            actor_user_id,
            MarkConversationReadRequest {
                conversation_id: conversation_id.to_string(),
                last_read_conversation_message_seq: 2,
            },
        ))
        .await?
        .into_inner();

    assert!(response.updated);
    assert_eq!(response.conversation_id, conversation_id.to_string());
    assert_eq!(response.last_read_conversation_message_seq, 2);
    assert!(response.read_at.is_some());

    let cursor = conversation_read_cursor::Entity::find_by_id((actor_user_id, conversation_id))
        .one(&env.db)
        .await?
        .expect("cursor row");
    assert_eq!(cursor.last_read_conversation_message_seq, 2);

    let outbox_rows: Vec<outbox_event::Model> = outbox_event::Entity::find()
        .filter(outbox_event::Column::EventType.eq("ConversationReadCursorUpdated"))
        .all(&env.db)
        .await?;
    assert_eq!(outbox_rows.len(), 1);

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn mark_conversation_read_keeps_cursor_monotonic() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let actor_user_id = Uuid::new_v4();
    let peer_user_id = Uuid::new_v4();
    let dm_pair_id = Uuid::new_v4();
    let conversation_id = Uuid::new_v4();

    insert_user_snapshot(&env.db, actor_user_id).await?;
    insert_user_snapshot(&env.db, peer_user_id).await?;
    insert_dm_pair(&env.db, dm_pair_id, actor_user_id, peer_user_id).await?;
    insert_conversation(&env.db, conversation_id, Some(dm_pair_id), None, "dm").await?;
    insert_message(&env.db, conversation_id, actor_user_id, 1, "one").await?;
    insert_message(&env.db, conversation_id, peer_user_id, 2, "two").await?;
    insert_read_cursor(&env.db, actor_user_id, conversation_id, 2).await?;

    let response = env
        .client
        .clone()
        .mark_conversation_read(actor_request(
            actor_user_id,
            MarkConversationReadRequest {
                conversation_id: conversation_id.to_string(),
                last_read_conversation_message_seq: 1,
            },
        ))
        .await?
        .into_inner();

    assert!(!response.updated);
    assert_eq!(response.last_read_conversation_message_seq, 2);

    let cursor = conversation_read_cursor::Entity::find_by_id((actor_user_id, conversation_id))
        .one(&env.db)
        .await?
        .expect("cursor row");
    assert_eq!(cursor.last_read_conversation_message_seq, 2);

    let outbox_rows: Vec<outbox_event::Model> = outbox_event::Entity::find()
        .filter(outbox_event::Column::EventType.eq("ConversationReadCursorUpdated"))
        .all(&env.db)
        .await?;
    assert_eq!(outbox_rows.len(), 0);

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn mark_conversation_read_rejects_seq_ahead_of_latest_message()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let actor_user_id = Uuid::new_v4();
    let peer_user_id = Uuid::new_v4();
    let dm_pair_id = Uuid::new_v4();
    let conversation_id = Uuid::new_v4();

    insert_user_snapshot(&env.db, actor_user_id).await?;
    insert_user_snapshot(&env.db, peer_user_id).await?;
    insert_dm_pair(&env.db, dm_pair_id, actor_user_id, peer_user_id).await?;
    insert_conversation(&env.db, conversation_id, Some(dm_pair_id), None, "dm").await?;
    insert_message(&env.db, conversation_id, actor_user_id, 1, "one").await?;

    let err = env
        .client
        .clone()
        .mark_conversation_read(actor_request(
            actor_user_id,
            MarkConversationReadRequest {
                conversation_id: conversation_id.to_string(),
                last_read_conversation_message_seq: 2,
            },
        ))
        .await
        .unwrap_err();

    assert_eq!(err.code(), Code::InvalidArgument);

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

async fn insert_dm_pair(
    db: &sea_orm::DatabaseConnection,
    dm_pair_id: Uuid,
    actor_user_id: Uuid,
    peer_user_id: Uuid,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = chrono::Utc::now();
    let (low_user_id, high_user_id) = if actor_user_id < peer_user_id {
        (actor_user_id, peer_user_id)
    } else {
        (peer_user_id, actor_user_id)
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
    conversation_message_seq: i64,
    body: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = chrono::Utc::now();
    chat_crate::entity::chat_message::Entity::insert(chat_message::ActiveModel {
        message_id: sea_orm::Set(Uuid::new_v4()),
        conversation_id: sea_orm::Set(conversation_id),
        author_user_id: sea_orm::Set(author_user_id),
        client_message_id: sea_orm::Set(None),
        conversation_message_seq: sea_orm::Set(conversation_message_seq),
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

async fn insert_read_cursor(
    db: &sea_orm::DatabaseConnection,
    user_id: Uuid,
    conversation_id: Uuid,
    last_read_conversation_message_seq: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = chrono::Utc::now();
    chat_crate::entity::conversation_read_cursor::Entity::insert(
        conversation_read_cursor::ActiveModel {
            user_id: sea_orm::Set(user_id),
            conversation_id: sea_orm::Set(conversation_id),
            last_read_conversation_message_seq: sea_orm::Set(last_read_conversation_message_seq),
            read_at: sea_orm::Set(now.into()),
            updated_at: sea_orm::Set(now.into()),
        },
    )
    .exec(db)
    .await?;

    Ok(())
}
