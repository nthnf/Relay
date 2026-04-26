use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bootstrap::{
    amqp::AmqpHandler,
    db,
    entity::{
        compose_queue, conversation_message_state, conversation_read_state, conversation_snapshot,
        dm_pair_snapshot, friend_request_snapshot, user_snapshot, workspace_channel_snapshot,
        workspace_member_snapshot, workspace_snapshot,
    },
};
use lapin::{
    BasicProperties, Confirmation, Connection, ConnectionProperties,
    options::{BasicPublishOptions, ConfirmSelectOptions, QueueDeclareOptions},
    types::{FieldTable, ShortString},
};
use migration::{Migrator, MigratorTrait};
use relay_amqp::AmqpSubscriber;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use testcontainers_modules::{
    postgres::Postgres,
    rabbitmq::RabbitMq,
    testcontainers::{core::IntoContainerPort, runners::AsyncRunner},
};
use uuid::Uuid;

#[tokio::test]
async fn consumes_amqp_events_and_updates_bootstrap_projections()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let prefix = unique_name("bootstrap-amqp-test");
    let tasks = start_subscribers(&env, &prefix).await?;

    let user_id = Uuid::new_v4();
    let author_user_id = Uuid::new_v4();
    let new_member_user_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    let conversation_id = Uuid::new_v4();
    let dm_pair_id = Uuid::new_v4();
    let dm_conversation_id = Uuid::new_v4();
    let friend_request_event_id = Uuid::new_v4();
    let now = chrono::Utc::now().to_rfc3339();

    publish_event(
        &env.amqp_addr,
        "identity.UserRegistered",
        json!({
            "user_id": user_id,
            "email": "user1@example.com",
            "email_verified": false,
            "username": "user1",
            "display_name": "User One",
            "avatar_url": null,
            "registered_at": now,
        }),
    )
    .await?;

    publish_event(
        &env.amqp_addr,
        "identity.UserRegistered",
        json!({
            "user_id": author_user_id,
            "email": "author@example.com",
            "email_verified": false,
            "username": "author",
            "display_name": "Author User",
            "avatar_url": null,
            "registered_at": now,
        }),
    )
    .await?;

    wait_until("author user snapshot", || async {
        user_snapshot::Entity::find_by_id(author_user_id)
            .one(&env.db)
            .await
            .map(|row| row.is_some())
    })
    .await?;

    wait_until("user snapshot", || async {
        user_snapshot::Entity::find_by_id(user_id)
            .one(&env.db)
            .await
            .map(|row| row.is_some())
    })
    .await?;

    publish_event_with_message_id(
        &env.amqp_addr,
        "friendship.FriendRequestCreated",
        friend_request_event_id,
        json!({
            "friend_request_id": Uuid::new_v4(),
            "requester_user_id": author_user_id,
            "addressee_user_id": user_id,
            "status": "pending",
            "created_at": now,
        }),
    )
    .await?;

    publish_event_with_message_id(
        &env.amqp_addr,
        "friendship.FriendRequestCreated",
        friend_request_event_id,
        json!({
            "friend_request_id": Uuid::new_v4(),
            "requester_user_id": author_user_id,
            "addressee_user_id": user_id,
            "status": "pending",
            "created_at": now,
        }),
    )
    .await?;

    wait_until("friend request snapshot", || async {
        let rows = friend_request_snapshot::Entity::find()
            .filter(friend_request_snapshot::Column::AddresseeUserId.eq(user_id))
            .all(&env.db)
            .await?;
        let queue = compose_queue::Entity::find()
            .filter(compose_queue::Column::ComposeKind.eq("user_app"))
            .filter(compose_queue::Column::UserId.eq(user_id))
            .one(&env.db)
            .await?;

        Ok(rows.len() == 1
            && rows[0].status == "pending"
            && queue.is_some_and(|row| row.status == "claimed"))
    })
    .await?;

    publish_event(
        &env.amqp_addr,
        "workspace.WorkspaceCreated",
        json!({
            "workspace_id": workspace_id,
            "name": "Relay HQ",
            "owner_user_id": user_id,
            "created_at": now,
            "initial_member_user_id": user_id,
        }),
    )
    .await?;

    publish_event(
        &env.amqp_addr,
        "workspace.WorkspaceChannelCreated",
        json!({
            "channel_id": channel_id,
            "workspace_id": workspace_id,
            "name": "general",
            "channel_kind": "text",
            "position": 1,
            "created_by_user_id": user_id,
            "created_at": now,
        }),
    )
    .await?;

    wait_until("workspace and channel snapshots", || async {
        let workspace = workspace_snapshot::Entity::find_by_id(workspace_id)
            .one(&env.db)
            .await?;
        let channel = workspace_channel_snapshot::Entity::find_by_id(channel_id)
            .one(&env.db)
            .await?;

        Ok(workspace.is_some_and(|row| row.name == "Relay HQ")
            && channel.is_some_and(|row| row.name == "general"))
    })
    .await?;

    publish_event(
        &env.amqp_addr,
        "workspace.WorkspaceMemberAdded",
        json!({
            "workspace_id": workspace_id,
            "user_id": new_member_user_id,
            "joined_at": now,
            "added_by_user_id": user_id,
            "source": "test",
        }),
    )
    .await?;

    wait_until("new member snapshot", || async {
        let member =
            workspace_member_snapshot::Entity::find_by_id((workspace_id, new_member_user_id))
                .one(&env.db)
                .await?;
        let queue = compose_queue::Entity::find()
            .filter(compose_queue::Column::ComposeKind.eq("workspace_channel"))
            .filter(compose_queue::Column::UserId.eq(new_member_user_id))
            .filter(compose_queue::Column::WorkspaceId.eq(workspace_id))
            .one(&env.db)
            .await?;

        Ok(member.is_some_and(|row| row.status == "active")
            && queue.is_some_and(|row| row.status == "claimed"))
    })
    .await?;

    publish_event(
        &env.amqp_addr,
        "chat.ConversationCreated",
        json!({
            "conversation_id": conversation_id,
            "target_type": "workspace_channel",
            "dm_pair_id": null,
            "workspace_channel_id": channel_id,
            "created_at": now,
        }),
    )
    .await?;

    wait_until("conversation snapshot", || async {
        let row = conversation_snapshot::Entity::find_by_id(conversation_id)
            .one(&env.db)
            .await?;
        let queue = compose_queue::Entity::find()
            .filter(compose_queue::Column::ComposeKind.eq("workspace_channel"))
            .filter(compose_queue::Column::ConversationId.eq(conversation_id))
            .one(&env.db)
            .await?;

        Ok(
            row.is_some_and(|row| row.workspace_channel_id == Some(channel_id))
                && queue.is_some_and(|row| row.status == "claimed"),
        )
    })
    .await?;

    publish_event(
        &env.amqp_addr,
        "chat.MessageCreated",
        json!({
            "delivery_id": Uuid::new_v4(),
            "message_id": Uuid::new_v4(),
            "conversation_id": conversation_id,
            "target_type": "workspace_channel",
            "workspace_id": workspace_id,
            "workspace_channel_id": channel_id,
            "author_user_id": author_user_id,
            "conversation_message_seq": 1,
            "body": "hello",
            "created_at": now,
        }),
    )
    .await?;

    wait_until("message state", || async {
        let row = conversation_message_state::Entity::find_by_id(conversation_id)
            .one(&env.db)
            .await?;

        Ok(row.is_some_and(|row| {
            row.last_message_seq == Some(1) && row.last_message_preview.as_deref() == Some("hello")
        }))
    })
    .await?;

    publish_event(
        &env.amqp_addr,
        "chat.ConversationReadCursorUpdated",
        json!({
            "conversation_id": conversation_id,
            "target_type": "workspace_channel",
            "workspace_channel_id": channel_id,
            "user_id": user_id,
            "last_read_conversation_message_seq": 1,
            "read_at": now,
        }),
    )
    .await?;

    wait_until("read cursor state", || async {
        let row = conversation_read_state::Entity::find_by_id((conversation_id, user_id))
            .one(&env.db)
            .await?;

        Ok(row.is_some_and(|row| row.last_read_conversation_message_seq == 1))
    })
    .await?;

    let (low_user_id, high_user_id) = if user_id < author_user_id {
        (user_id, author_user_id)
    } else {
        (author_user_id, user_id)
    };
    publish_event(
        &env.amqp_addr,
        "chat.DmPairCreated",
        json!({
            "dm_pair_id": dm_pair_id,
            "low_user_id": low_user_id,
            "high_user_id": high_user_id,
            "created_at": now,
        }),
    )
    .await?;

    wait_until("dm pair snapshot", || async {
        dm_pair_snapshot::Entity::find_by_id(dm_pair_id)
            .one(&env.db)
            .await
            .map(|row| row.is_some())
    })
    .await?;

    publish_event(
        &env.amqp_addr,
        "chat.ConversationCreated",
        json!({
            "conversation_id": dm_conversation_id,
            "target_type": "dm",
            "dm_pair_id": dm_pair_id,
            "workspace_channel_id": null,
            "created_at": now,
        }),
    )
    .await?;

    wait_until("dm conversation snapshot", || async {
        let row = conversation_snapshot::Entity::find_by_id(dm_conversation_id)
            .one(&env.db)
            .await?;
        let queue = compose_queue::Entity::find()
            .filter(compose_queue::Column::ComposeKind.eq("dm"))
            .filter(compose_queue::Column::DmPairId.eq(dm_pair_id))
            .filter(compose_queue::Column::ConversationId.eq(dm_conversation_id))
            .one(&env.db)
            .await?;

        Ok(row.is_some_and(|row| row.dm_pair_id == Some(dm_pair_id))
            && queue.is_some_and(|row| row.status == "claimed"))
    })
    .await?;

    for task in tasks {
        task.abort();
        let _ = task.await;
    }

    Ok(())
}

struct TestEnv {
    _postgres: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    _rabbitmq: testcontainers_modules::testcontainers::ContainerAsync<RabbitMq>,
    db: sea_orm::DatabaseConnection,
    amqp_addr: String,
}

impl TestEnv {
    async fn start() -> Result<Self, Box<dyn std::error::Error>> {
        let postgres = Postgres::default().start().await?;
        let postgres_host = postgres.get_host().await?;
        let postgres_port = postgres.get_host_port_ipv4(5432.tcp()).await?;
        let database_url =
            format!("postgres://postgres:postgres@{postgres_host}:{postgres_port}/postgres");
        let db = db::connect(&database_url).await?;
        Migrator::up(&db, None).await?;

        let rabbitmq = RabbitMq::default().start().await?;
        let rabbitmq_host = rabbitmq.get_host().await?;
        let rabbitmq_port = rabbitmq.get_host_port_ipv4(5672.tcp()).await?;
        let amqp_addr = format!("amqp://{rabbitmq_host}:{rabbitmq_port}/%2f");

        Ok(Self {
            _postgres: postgres,
            _rabbitmq: rabbitmq,
            db,
            amqp_addr,
        })
    }
}

async fn start_subscribers(
    env: &TestEnv,
    prefix: &str,
) -> Result<
    Vec<tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>>,
    Box<dyn std::error::Error>,
> {
    let bindings = ["identity.*", "friendship.*", "workspace.*", "chat.*"];
    let mut tasks = Vec::new();

    for binding in bindings {
        let queue = format!("{prefix}.{binding}");
        let consumer = format!("{prefix}.{binding}.consumer");
        let amqp_addr = env.amqp_addr.clone();
        let db = env.db.clone();
        let queue_for_task = queue.clone();

        let task = tokio::spawn(async move {
            AmqpSubscriber::topic(
                "bootstrap-test",
                queue_for_task,
                consumer,
                "relay.events",
                binding,
            )
            .handle(AmqpHandler::new(db))
            .run(&amqp_addr)
            .await
        });

        wait_for_queue(&env.amqp_addr, &queue).await?;
        tasks.push(task);
    }

    Ok(tasks)
}

async fn wait_for_queue(
    amqp_addr: &str,
    queue_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let connection = Connection::connect(amqp_addr, ConnectionProperties::default()).await?;
    let deadline = Instant::now() + Duration::from_secs(10);

    loop {
        let channel = connection.create_channel().await?;
        let result = channel
            .queue_declare(
                queue_name.into(),
                QueueDeclareOptions {
                    passive: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await;

        match result {
            Ok(_) => return Ok(()),
            Err(error) if Instant::now() < deadline => {
                let _ = error;
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(error) => return Err(Box::new(error)),
        }
    }
}

async fn publish_event(
    amqp_addr: &str,
    routing_key: &str,
    payload: serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    publish_event_with_message_id(amqp_addr, routing_key, Uuid::new_v4(), payload).await
}

async fn publish_event_with_message_id(
    amqp_addr: &str,
    routing_key: &str,
    message_id: Uuid,
    payload: serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let connection = Connection::connect(amqp_addr, ConnectionProperties::default()).await?;
    let channel = connection.create_channel().await?;
    let payload = serde_json::to_vec(&payload)?;

    channel
        .confirm_select(ConfirmSelectOptions::default())
        .await?;
    let confirm = channel
        .basic_publish(
            "relay.events".into(),
            routing_key.into(),
            BasicPublishOptions::default(),
            payload.as_slice(),
            BasicProperties::default()
                .with_content_type(ShortString::from("application/json"))
                .with_message_id(ShortString::from(message_id.to_string())),
        )
        .await?;

    match confirm.await? {
        Confirmation::Ack(_) => Ok(()),
        Confirmation::Nack(_) => Err("broker returned nack for test publish".into()),
        Confirmation::NotRequested => Err("publisher confirms were not enabled".into()),
    }
}

async fn wait_until<F, Fut>(label: &str, mut condition: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<bool, sea_orm::DbErr>>,
{
    let deadline = Instant::now() + Duration::from_secs(10);

    loop {
        if condition().await? {
            return Ok(());
        }

        if Instant::now() >= deadline {
            return Err(format!("timed out waiting for {label}").into());
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn unique_name(prefix: &str) -> String {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    format!("{prefix}.{suffix}")
}
