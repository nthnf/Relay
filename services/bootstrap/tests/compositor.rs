use bootstrap::{
    compositor::Compositor,
    db,
    entity::{
        compose_queue, conversation_message_state, conversation_read_state, conversation_snapshot,
        dm_pair_snapshot, dm_projection, friend_request_snapshot, user_app_projection,
        user_snapshot, workspace_channel_projection, workspace_channel_snapshot,
        workspace_member_snapshot, workspace_projection, workspace_snapshot,
    },
};
use migration::{Migrator, MigratorTrait};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{core::IntoContainerPort, runners::AsyncRunner},
};
use uuid::Uuid;

#[tokio::test]
async fn compositor_builds_user_app_projection_from_source_snapshots()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = chrono::Utc::now();
    let user_id = Uuid::new_v4();
    let requester_user_id = Uuid::new_v4();

    user_snapshot::ActiveModel {
        user_id: Set(user_id),
        username: Set("viewer".to_string()),
        display_name: Set("Viewer".to_string()),
        avatar_url: Set(None),
        updated_at: Set(now.into()),
    }
    .insert(&env.db)
    .await?;
    friend_request_snapshot::ActiveModel {
        friend_request_id: Set(Uuid::new_v4()),
        requester_user_id: Set(requester_user_id),
        addressee_user_id: Set(user_id),
        status: Set("pending".to_string()),
        updated_at: Set(now.into()),
    }
    .insert(&env.db)
    .await?;
    insert_queue(&env.db, "user_app", Some(user_id), None, None, None, None).await?;

    Compositor::new(env.db.clone()).run_once().await?;

    let projection = user_app_projection::Entity::find_by_id(user_id)
        .one(&env.db)
        .await?
        .ok_or("missing user app projection")?;
    assert_eq!(projection.username, "viewer");
    assert_eq!(projection.pending_friend_request_count, 1);
    assert!(queue_is_empty(&env.db).await?);

    Ok(())
}

#[tokio::test]
async fn compositor_converges_workspace_channel_after_out_of_order_sources()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = chrono::Utc::now();
    let user_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    let conversation_id = Uuid::new_v4();
    let message_id = Uuid::new_v4();

    conversation_snapshot::ActiveModel {
        conversation_id: Set(conversation_id),
        target_type: Set("workspace_channel".to_string()),
        dm_pair_id: Set(None),
        workspace_channel_id: Set(Some(channel_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&env.db)
    .await?;
    workspace_snapshot::ActiveModel {
        workspace_id: Set(workspace_id),
        name: Set("Relay HQ".to_string()),
        icon_url: Set(None),
        owner_user_id: Set(user_id),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&env.db)
    .await?;
    workspace_member_snapshot::ActiveModel {
        workspace_id: Set(workspace_id),
        user_id: Set(user_id),
        status: Set("active".to_string()),
        joined_at: Set(Some(now.into())),
        removed_at: Set(None),
        updated_at: Set(now.into()),
    }
    .insert(&env.db)
    .await?;
    workspace_channel_snapshot::ActiveModel {
        channel_id: Set(channel_id),
        workspace_id: Set(workspace_id),
        name: Set("general".to_string()),
        channel_kind: Set("text".to_string()),
        position: Set(1),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&env.db)
    .await?;
    conversation_message_state::ActiveModel {
        conversation_id: Set(conversation_id),
        last_message_id: Set(Some(message_id)),
        last_message_author_user_id: Set(Some(Uuid::new_v4())),
        last_message_seq: Set(Some(3)),
        last_message_preview: Set(Some("hello".to_string())),
        last_activity_at: Set(Some(now.into())),
        updated_at: Set(now.into()),
    }
    .insert(&env.db)
    .await?;
    conversation_read_state::ActiveModel {
        conversation_id: Set(conversation_id),
        user_id: Set(user_id),
        last_read_conversation_message_seq: Set(1),
        read_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&env.db)
    .await?;
    insert_queue(
        &env.db,
        "workspace_channel",
        None,
        None,
        None,
        Some(conversation_id),
        None,
    )
    .await?;

    Compositor::new(env.db.clone()).run_once().await?;

    let channel =
        workspace_channel_projection::Entity::find_by_id((user_id, workspace_id, channel_id))
            .one(&env.db)
            .await?
            .ok_or("missing channel projection")?;
    assert_eq!(channel.conversation_id, Some(conversation_id));
    assert_eq!(channel.unread_count, 2);
    assert_eq!(channel.last_message_seq, Some(3));

    let workspace = workspace_projection::Entity::find_by_id((user_id, workspace_id))
        .one(&env.db)
        .await?
        .ok_or("missing workspace projection")?;
    assert_eq!(workspace.unread_count, 2);
    assert_eq!(workspace.member_count, 1);
    assert!(queue_is_empty(&env.db).await?);

    Ok(())
}

#[tokio::test]
async fn compositor_does_not_count_latest_message_as_unread_for_author()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = chrono::Utc::now();
    let user_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    let conversation_id = Uuid::new_v4();

    seed_workspace_channel_sources(
        &env.db,
        user_id,
        workspace_id,
        channel_id,
        conversation_id,
        now,
    )
    .await?;
    conversation_message_state::ActiveModel {
        conversation_id: Set(conversation_id),
        last_message_id: Set(Some(Uuid::new_v4())),
        last_message_author_user_id: Set(Some(user_id)),
        last_message_seq: Set(Some(5)),
        last_message_preview: Set(Some("self message".to_string())),
        last_activity_at: Set(Some(now.into())),
        updated_at: Set(now.into()),
    }
    .insert(&env.db)
    .await?;
    insert_queue(
        &env.db,
        "workspace_channel",
        Some(user_id),
        Some(workspace_id),
        Some(channel_id),
        Some(conversation_id),
        None,
    )
    .await?;

    Compositor::new(env.db.clone()).run_once().await?;

    let channel =
        workspace_channel_projection::Entity::find_by_id((user_id, workspace_id, channel_id))
            .one(&env.db)
            .await?
            .ok_or("missing channel projection")?;
    assert_eq!(channel.unread_count, 0);

    Ok(())
}

#[tokio::test]
async fn compositor_refreshes_dm_peer_fields_from_user_only_work()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = chrono::Utc::now();
    let changed_user_id = Uuid::new_v4();
    let other_user_id = Uuid::new_v4();
    let dm_pair_id = Uuid::new_v4();

    user_snapshot::ActiveModel {
        user_id: Set(changed_user_id),
        username: Set("new-name".to_string()),
        display_name: Set("New Name".to_string()),
        avatar_url: Set(Some("https://example.com/avatar.png".to_string())),
        updated_at: Set(now.into()),
    }
    .insert(&env.db)
    .await?;
    user_snapshot::ActiveModel {
        user_id: Set(other_user_id),
        username: Set("other".to_string()),
        display_name: Set("Other".to_string()),
        avatar_url: Set(None),
        updated_at: Set(now.into()),
    }
    .insert(&env.db)
    .await?;
    dm_pair_snapshot::ActiveModel {
        dm_pair_id: Set(dm_pair_id),
        low_user_id: Set(changed_user_id.min(other_user_id)),
        high_user_id: Set(changed_user_id.max(other_user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&env.db)
    .await?;
    insert_queue(&env.db, "dm", Some(changed_user_id), None, None, None, None).await?;

    Compositor::new(env.db.clone()).run_once().await?;

    let other_user_row = dm_projection::Entity::find_by_id((other_user_id, dm_pair_id))
        .one(&env.db)
        .await?
        .ok_or("missing dm projection")?;
    assert_eq!(other_user_row.peer_user_id, changed_user_id);
    assert_eq!(other_user_row.peer_username, "new-name");
    assert_eq!(other_user_row.peer_display_name, "New Name");

    Ok(())
}

struct TestEnv {
    _postgres: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    db: sea_orm::DatabaseConnection,
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

        Ok(Self {
            _postgres: postgres,
            db,
        })
    }
}

async fn insert_queue(
    db: &sea_orm::DatabaseConnection,
    compose_kind: &str,
    user_id: Option<Uuid>,
    workspace_id: Option<Uuid>,
    channel_id: Option<Uuid>,
    conversation_id: Option<Uuid>,
    dm_pair_id: Option<Uuid>,
) -> Result<(), sea_orm::DbErr> {
    let now = chrono::Utc::now();

    compose_queue::ActiveModel {
        compose_key: Set(format!("test:{compose_kind}:{}", Uuid::new_v4())),
        compose_kind: Set(compose_kind.to_string()),
        user_id: Set(user_id),
        workspace_id: Set(workspace_id),
        channel_id: Set(channel_id),
        conversation_id: Set(conversation_id),
        dm_pair_id: Set(dm_pair_id),
        status: Set("claimed".to_string()),
        attempts: Set(0),
        available_at: Set(now.into()),
        claimed_at: Set(Some(now.into())),
        last_error: Set(None),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    Ok(())
}

async fn seed_workspace_channel_sources(
    db: &sea_orm::DatabaseConnection,
    user_id: Uuid,
    workspace_id: Uuid,
    channel_id: Uuid,
    conversation_id: Uuid,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), sea_orm::DbErr> {
    workspace_snapshot::ActiveModel {
        workspace_id: Set(workspace_id),
        name: Set("Relay HQ".to_string()),
        icon_url: Set(None),
        owner_user_id: Set(user_id),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    workspace_member_snapshot::ActiveModel {
        workspace_id: Set(workspace_id),
        user_id: Set(user_id),
        status: Set("active".to_string()),
        joined_at: Set(Some(now.into())),
        removed_at: Set(None),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    workspace_channel_snapshot::ActiveModel {
        channel_id: Set(channel_id),
        workspace_id: Set(workspace_id),
        name: Set("general".to_string()),
        channel_kind: Set("text".to_string()),
        position: Set(1),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    conversation_snapshot::ActiveModel {
        conversation_id: Set(conversation_id),
        target_type: Set("workspace_channel".to_string()),
        dm_pair_id: Set(None),
        workspace_channel_id: Set(Some(channel_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    Ok(())
}

async fn queue_is_empty(db: &sea_orm::DatabaseConnection) -> Result<bool, sea_orm::DbErr> {
    compose_queue::Entity::find()
        .filter(compose_queue::Column::Status.ne("failed"))
        .one(db)
        .await
        .map(|row| row.is_none())
}
