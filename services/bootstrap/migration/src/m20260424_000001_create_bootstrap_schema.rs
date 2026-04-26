use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_control_tables(manager).await?;
        create_source_snapshot_tables(manager).await?;
        create_unread_projection_tables(manager).await?;
        create_ui_projection_tables(manager).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_ui_projection_tables(manager).await?;
        drop_unread_projection_tables(manager).await?;
        drop_source_snapshot_tables(manager).await?;
        drop_control_tables(manager).await?;

        Ok(())
    }
}

async fn create_control_tables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(ProcessedEvent::Table)
                .if_not_exists()
                .col(text(ProcessedEvent::EventId).not_null().primary_key())
                .col(text(ProcessedEvent::RoutingKey).not_null())
                .col(
                    timestamp_with_time_zone(ProcessedEvent::ProcessedAt)
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .to_owned(),
        )
        .await?;

    manager
        .create_table(
            Table::create()
                .table(ComposeQueue::Table)
                .if_not_exists()
                .col(text(ComposeQueue::ComposeKey).not_null().primary_key())
                .col(text(ComposeQueue::ComposeKind).not_null())
                .col(uuid_null(ComposeQueue::UserId))
                .col(uuid_null(ComposeQueue::WorkspaceId))
                .col(uuid_null(ComposeQueue::ChannelId))
                .col(uuid_null(ComposeQueue::ConversationId))
                .col(uuid_null(ComposeQueue::DmPairId))
                .col(text(ComposeQueue::Status).not_null().default("pending"))
                .col(integer(ComposeQueue::Attempts).not_null().default(0))
                .col(
                    timestamp_with_time_zone(ComposeQueue::AvailableAt)
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .col(timestamp_with_time_zone_null(ComposeQueue::ClaimedAt))
                .col(text_null(ComposeQueue::LastError))
                .col(
                    timestamp_with_time_zone(ComposeQueue::UpdatedAt)
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .to_owned(),
        )
        .await?;

    create_index(
        manager,
        "idx-compose-queue-status-available-at",
        ComposeQueue::Table,
        [ComposeQueue::Status, ComposeQueue::AvailableAt],
    )
    .await?;
    create_index(
        manager,
        "idx-compose-queue-kind-user",
        ComposeQueue::Table,
        [ComposeQueue::ComposeKind, ComposeQueue::UserId],
    )
    .await?;
    create_index(
        manager,
        "idx-compose-queue-workspace-id",
        ComposeQueue::Table,
        [ComposeQueue::WorkspaceId],
    )
    .await?;
    create_index(
        manager,
        "idx-compose-queue-conversation-id",
        ComposeQueue::Table,
        [ComposeQueue::ConversationId],
    )
    .await?;
    create_index(
        manager,
        "idx-compose-queue-dm-pair-id",
        ComposeQueue::Table,
        [ComposeQueue::DmPairId],
    )
    .await?;

    Ok(())
}

async fn create_source_snapshot_tables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(UserSnapshot::Table)
                .if_not_exists()
                .col(uuid(UserSnapshot::UserId).not_null().primary_key())
                .col(text(UserSnapshot::Username).not_null())
                .col(text(UserSnapshot::DisplayName).not_null())
                .col(text_null(UserSnapshot::AvatarUrl))
                .col(
                    timestamp_with_time_zone(UserSnapshot::UpdatedAt)
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .to_owned(),
        )
        .await?;

    manager
        .create_table(
            Table::create()
                .table(FriendRequestSnapshot::Table)
                .if_not_exists()
                .col(
                    uuid(FriendRequestSnapshot::FriendRequestId)
                        .not_null()
                        .primary_key(),
                )
                .col(uuid(FriendRequestSnapshot::RequesterUserId).not_null())
                .col(uuid(FriendRequestSnapshot::AddresseeUserId).not_null())
                .col(text(FriendRequestSnapshot::Status).not_null())
                .col(
                    timestamp_with_time_zone(FriendRequestSnapshot::UpdatedAt)
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .to_owned(),
        )
        .await?;
    create_index(
        manager,
        "idx-friend-request-snapshot-addressee-status",
        FriendRequestSnapshot::Table,
        [
            FriendRequestSnapshot::AddresseeUserId,
            FriendRequestSnapshot::Status,
        ],
    )
    .await?;

    manager
        .create_table(
            Table::create()
                .table(WorkspaceSnapshot::Table)
                .if_not_exists()
                .col(
                    uuid(WorkspaceSnapshot::WorkspaceId)
                        .not_null()
                        .primary_key(),
                )
                .col(text(WorkspaceSnapshot::Name).not_null())
                .col(text_null(WorkspaceSnapshot::IconUrl))
                .col(uuid(WorkspaceSnapshot::OwnerUserId).not_null())
                .col(timestamp_with_time_zone(WorkspaceSnapshot::CreatedAt).not_null())
                .col(
                    timestamp_with_time_zone(WorkspaceSnapshot::UpdatedAt)
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .to_owned(),
        )
        .await?;

    manager
        .create_table(
            Table::create()
                .table(WorkspaceMemberSnapshot::Table)
                .if_not_exists()
                .col(uuid(WorkspaceMemberSnapshot::WorkspaceId).not_null())
                .col(uuid(WorkspaceMemberSnapshot::UserId).not_null())
                .col(text(WorkspaceMemberSnapshot::Status).not_null())
                .col(timestamp_with_time_zone_null(
                    WorkspaceMemberSnapshot::JoinedAt,
                ))
                .col(timestamp_with_time_zone_null(
                    WorkspaceMemberSnapshot::RemovedAt,
                ))
                .col(
                    timestamp_with_time_zone(WorkspaceMemberSnapshot::UpdatedAt)
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .primary_key(
                    Index::create()
                        .col(WorkspaceMemberSnapshot::WorkspaceId)
                        .col(WorkspaceMemberSnapshot::UserId),
                )
                .to_owned(),
        )
        .await?;
    create_index(
        manager,
        "idx-workspace-member-snapshot-user-status",
        WorkspaceMemberSnapshot::Table,
        [
            WorkspaceMemberSnapshot::UserId,
            WorkspaceMemberSnapshot::Status,
        ],
    )
    .await?;
    create_index(
        manager,
        "idx-workspace-member-snapshot-workspace-status",
        WorkspaceMemberSnapshot::Table,
        [
            WorkspaceMemberSnapshot::WorkspaceId,
            WorkspaceMemberSnapshot::Status,
        ],
    )
    .await?;

    manager
        .create_table(
            Table::create()
                .table(WorkspaceChannelSnapshot::Table)
                .if_not_exists()
                .col(
                    uuid(WorkspaceChannelSnapshot::ChannelId)
                        .not_null()
                        .primary_key(),
                )
                .col(uuid(WorkspaceChannelSnapshot::WorkspaceId).not_null())
                .col(text(WorkspaceChannelSnapshot::Name).not_null())
                .col(text(WorkspaceChannelSnapshot::ChannelKind).not_null())
                .col(integer(WorkspaceChannelSnapshot::Position).not_null())
                .col(timestamp_with_time_zone(WorkspaceChannelSnapshot::CreatedAt).not_null())
                .col(
                    timestamp_with_time_zone(WorkspaceChannelSnapshot::UpdatedAt)
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .to_owned(),
        )
        .await?;
    create_index(
        manager,
        "idx-workspace-channel-snapshot-workspace-position",
        WorkspaceChannelSnapshot::Table,
        [
            WorkspaceChannelSnapshot::WorkspaceId,
            WorkspaceChannelSnapshot::Position,
            WorkspaceChannelSnapshot::ChannelId,
        ],
    )
    .await?;

    manager
        .create_table(
            Table::create()
                .table(DmPairSnapshot::Table)
                .if_not_exists()
                .col(uuid(DmPairSnapshot::DmPairId).not_null().primary_key())
                .col(uuid(DmPairSnapshot::LowUserId).not_null())
                .col(uuid(DmPairSnapshot::HighUserId).not_null())
                .col(timestamp_with_time_zone(DmPairSnapshot::CreatedAt).not_null())
                .col(
                    timestamp_with_time_zone(DmPairSnapshot::UpdatedAt)
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .to_owned(),
        )
        .await?;
    create_index(
        manager,
        "idx-dm-pair-snapshot-low-user-id",
        DmPairSnapshot::Table,
        [DmPairSnapshot::LowUserId],
    )
    .await?;
    create_index(
        manager,
        "idx-dm-pair-snapshot-high-user-id",
        DmPairSnapshot::Table,
        [DmPairSnapshot::HighUserId],
    )
    .await?;

    manager
        .create_table(
            Table::create()
                .table(ConversationSnapshot::Table)
                .if_not_exists()
                .col(
                    uuid(ConversationSnapshot::ConversationId)
                        .not_null()
                        .primary_key(),
                )
                .col(text(ConversationSnapshot::TargetType).not_null())
                .col(uuid_null(ConversationSnapshot::DmPairId))
                .col(uuid_null(ConversationSnapshot::WorkspaceChannelId))
                .col(timestamp_with_time_zone(ConversationSnapshot::CreatedAt).not_null())
                .col(
                    timestamp_with_time_zone(ConversationSnapshot::UpdatedAt)
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .to_owned(),
        )
        .await?;
    create_index(
        manager,
        "idx-conversation-snapshot-dm-pair-id",
        ConversationSnapshot::Table,
        [ConversationSnapshot::DmPairId],
    )
    .await?;
    create_index(
        manager,
        "idx-conversation-snapshot-workspace-channel-id",
        ConversationSnapshot::Table,
        [ConversationSnapshot::WorkspaceChannelId],
    )
    .await?;

    manager
        .create_table(
            Table::create()
                .table(ConversationMessageState::Table)
                .if_not_exists()
                .col(
                    uuid(ConversationMessageState::ConversationId)
                        .not_null()
                        .primary_key(),
                )
                .col(uuid_null(ConversationMessageState::LastMessageId))
                .col(uuid_null(ConversationMessageState::LastMessageAuthorUserId))
                .col(big_integer_null(ConversationMessageState::LastMessageSeq))
                .col(text_null(ConversationMessageState::LastMessagePreview))
                .col(timestamp_with_time_zone_null(
                    ConversationMessageState::LastActivityAt,
                ))
                .col(
                    timestamp_with_time_zone(ConversationMessageState::UpdatedAt)
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .to_owned(),
        )
        .await?;
    create_index(
        manager,
        "idx-conversation-message-state-last-message-id",
        ConversationMessageState::Table,
        [ConversationMessageState::LastMessageId],
    )
    .await?;

    manager
        .create_table(
            Table::create()
                .table(ConversationReadState::Table)
                .if_not_exists()
                .col(uuid(ConversationReadState::ConversationId).not_null())
                .col(uuid(ConversationReadState::UserId).not_null())
                .col(big_integer(ConversationReadState::LastReadConversationMessageSeq).not_null())
                .col(timestamp_with_time_zone(ConversationReadState::ReadAt).not_null())
                .col(
                    timestamp_with_time_zone(ConversationReadState::UpdatedAt)
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .primary_key(
                    Index::create()
                        .col(ConversationReadState::ConversationId)
                        .col(ConversationReadState::UserId),
                )
                .to_owned(),
        )
        .await?;
    create_index(
        manager,
        "idx-conversation-read-state-user-id",
        ConversationReadState::Table,
        [ConversationReadState::UserId],
    )
    .await?;

    Ok(())
}

async fn create_unread_projection_tables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(WorkspaceChannelUnreadProjection::Table)
                .if_not_exists()
                .col(uuid(WorkspaceChannelUnreadProjection::UserId).not_null())
                .col(uuid(WorkspaceChannelUnreadProjection::WorkspaceId).not_null())
                .col(uuid(WorkspaceChannelUnreadProjection::ChannelId).not_null())
                .col(uuid(WorkspaceChannelUnreadProjection::ConversationId).not_null())
                .col(big_integer_null(
                    WorkspaceChannelUnreadProjection::LastMessageSeq,
                ))
                .col(big_integer_null(
                    WorkspaceChannelUnreadProjection::LastReadConversationMessageSeq,
                ))
                .col(
                    integer(WorkspaceChannelUnreadProjection::UnreadCount)
                        .not_null()
                        .default(0),
                )
                .col(
                    timestamp_with_time_zone(WorkspaceChannelUnreadProjection::UpdatedAt)
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .primary_key(
                    Index::create()
                        .col(WorkspaceChannelUnreadProjection::UserId)
                        .col(WorkspaceChannelUnreadProjection::ChannelId),
                )
                .to_owned(),
        )
        .await?;
    create_index(
        manager,
        "idx-workspace-channel-unread-user-workspace",
        WorkspaceChannelUnreadProjection::Table,
        [
            WorkspaceChannelUnreadProjection::UserId,
            WorkspaceChannelUnreadProjection::WorkspaceId,
        ],
    )
    .await?;
    create_index(
        manager,
        "idx-workspace-channel-unread-conversation-id",
        WorkspaceChannelUnreadProjection::Table,
        [WorkspaceChannelUnreadProjection::ConversationId],
    )
    .await?;

    manager
        .create_table(
            Table::create()
                .table(DmUnreadProjection::Table)
                .if_not_exists()
                .col(uuid(DmUnreadProjection::UserId).not_null())
                .col(uuid(DmUnreadProjection::DmPairId).not_null())
                .col(uuid(DmUnreadProjection::ConversationId).not_null())
                .col(big_integer_null(DmUnreadProjection::LastMessageSeq))
                .col(big_integer_null(
                    DmUnreadProjection::LastReadConversationMessageSeq,
                ))
                .col(
                    integer(DmUnreadProjection::UnreadCount)
                        .not_null()
                        .default(0),
                )
                .col(
                    timestamp_with_time_zone(DmUnreadProjection::UpdatedAt)
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .primary_key(
                    Index::create()
                        .col(DmUnreadProjection::UserId)
                        .col(DmUnreadProjection::DmPairId),
                )
                .to_owned(),
        )
        .await?;
    create_index(
        manager,
        "idx-dm-unread-projection-conversation-id",
        DmUnreadProjection::Table,
        [DmUnreadProjection::ConversationId],
    )
    .await?;

    manager
        .create_table(
            Table::create()
                .table(WorkspaceUnreadProjection::Table)
                .if_not_exists()
                .col(uuid(WorkspaceUnreadProjection::UserId).not_null())
                .col(uuid(WorkspaceUnreadProjection::WorkspaceId).not_null())
                .col(
                    integer(WorkspaceUnreadProjection::UnreadCount)
                        .not_null()
                        .default(0),
                )
                .col(
                    timestamp_with_time_zone(WorkspaceUnreadProjection::UpdatedAt)
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .primary_key(
                    Index::create()
                        .col(WorkspaceUnreadProjection::UserId)
                        .col(WorkspaceUnreadProjection::WorkspaceId),
                )
                .to_owned(),
        )
        .await?;

    Ok(())
}

async fn create_ui_projection_tables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(UserAppProjection::Table)
                .if_not_exists()
                .col(uuid(UserAppProjection::UserId).not_null().primary_key())
                .col(text(UserAppProjection::Username).not_null())
                .col(text(UserAppProjection::DisplayName).not_null())
                .col(text_null(UserAppProjection::AvatarUrl))
                .col(
                    integer(UserAppProjection::PendingFriendRequestCount)
                        .not_null()
                        .default(0),
                )
                .col(
                    timestamp_with_time_zone(UserAppProjection::UpdatedAt)
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .to_owned(),
        )
        .await?;

    manager
        .create_table(
            Table::create()
                .table(WorkspaceProjection::Table)
                .if_not_exists()
                .col(uuid(WorkspaceProjection::UserId).not_null())
                .col(uuid(WorkspaceProjection::WorkspaceId).not_null())
                .col(text(WorkspaceProjection::WorkspaceName).not_null())
                .col(text_null(WorkspaceProjection::WorkspaceIconUrl))
                .col(
                    integer(WorkspaceProjection::MemberCount)
                        .not_null()
                        .default(0),
                )
                .col(
                    integer(WorkspaceProjection::UnreadCount)
                        .not_null()
                        .default(0),
                )
                .col(
                    timestamp_with_time_zone(WorkspaceProjection::UpdatedAt)
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .primary_key(
                    Index::create()
                        .col(WorkspaceProjection::UserId)
                        .col(WorkspaceProjection::WorkspaceId),
                )
                .to_owned(),
        )
        .await?;
    create_index(
        manager,
        "idx-workspace-projection-user-name-id",
        WorkspaceProjection::Table,
        [
            WorkspaceProjection::UserId,
            WorkspaceProjection::WorkspaceName,
            WorkspaceProjection::WorkspaceId,
        ],
    )
    .await?;

    manager
        .create_table(
            Table::create()
                .table(WorkspaceChannelProjection::Table)
                .if_not_exists()
                .col(uuid(WorkspaceChannelProjection::UserId).not_null())
                .col(uuid(WorkspaceChannelProjection::WorkspaceId).not_null())
                .col(uuid(WorkspaceChannelProjection::ChannelId).not_null())
                .col(uuid_null(WorkspaceChannelProjection::ConversationId))
                .col(text(WorkspaceChannelProjection::ChannelName).not_null())
                .col(text(WorkspaceChannelProjection::ChannelKind).not_null())
                .col(integer(WorkspaceChannelProjection::Position).not_null())
                .col(big_integer_null(WorkspaceChannelProjection::LastMessageSeq))
                .col(big_integer_null(
                    WorkspaceChannelProjection::LastReadConversationMessageSeq,
                ))
                .col(
                    integer(WorkspaceChannelProjection::UnreadCount)
                        .not_null()
                        .default(0),
                )
                .col(
                    timestamp_with_time_zone(WorkspaceChannelProjection::UpdatedAt)
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .primary_key(
                    Index::create()
                        .col(WorkspaceChannelProjection::UserId)
                        .col(WorkspaceChannelProjection::WorkspaceId)
                        .col(WorkspaceChannelProjection::ChannelId),
                )
                .to_owned(),
        )
        .await?;
    create_index(
        manager,
        "idx-workspace-channel-projection-user-workspace-position",
        WorkspaceChannelProjection::Table,
        [
            WorkspaceChannelProjection::UserId,
            WorkspaceChannelProjection::WorkspaceId,
            WorkspaceChannelProjection::Position,
            WorkspaceChannelProjection::ChannelId,
        ],
    )
    .await?;
    create_index(
        manager,
        "idx-workspace-channel-projection-conversation-id",
        WorkspaceChannelProjection::Table,
        [WorkspaceChannelProjection::ConversationId],
    )
    .await?;

    manager
        .create_table(
            Table::create()
                .table(DmProjection::Table)
                .if_not_exists()
                .col(uuid(DmProjection::UserId).not_null())
                .col(uuid_null(DmProjection::ConversationId))
                .col(uuid(DmProjection::DmPairId).not_null())
                .col(uuid(DmProjection::PeerUserId).not_null())
                .col(text(DmProjection::PeerUsername).not_null())
                .col(text(DmProjection::PeerDisplayName).not_null())
                .col(text_null(DmProjection::PeerAvatarUrl))
                .col(big_integer_null(DmProjection::LastMessageSeq))
                .col(big_integer_null(
                    DmProjection::LastReadConversationMessageSeq,
                ))
                .col(text_null(DmProjection::LastMessagePreview))
                .col(timestamp_with_time_zone_null(DmProjection::LastActivityAt))
                .col(integer(DmProjection::UnreadCount).not_null().default(0))
                .col(
                    timestamp_with_time_zone(DmProjection::UpdatedAt)
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .primary_key(
                    Index::create()
                        .col(DmProjection::UserId)
                        .col(DmProjection::DmPairId),
                )
                .to_owned(),
        )
        .await?;
    create_index(
        manager,
        "idx-dm-projection-user-last-activity",
        DmProjection::Table,
        [
            DmProjection::UserId,
            DmProjection::LastActivityAt,
            DmProjection::ConversationId,
        ],
    )
    .await?;
    create_index(
        manager,
        "idx-dm-projection-conversation-id",
        DmProjection::Table,
        [DmProjection::ConversationId],
    )
    .await?;
    create_index(
        manager,
        "idx-dm-projection-dm-pair-id",
        DmProjection::Table,
        [DmProjection::DmPairId],
    )
    .await?;

    Ok(())
}

async fn drop_ui_projection_tables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    drop_index(manager, "idx-dm-projection-dm-pair-id", DmProjection::Table).await?;
    drop_index(
        manager,
        "idx-dm-projection-conversation-id",
        DmProjection::Table,
    )
    .await?;
    drop_index(
        manager,
        "idx-dm-projection-user-last-activity",
        DmProjection::Table,
    )
    .await?;
    drop_table(manager, DmProjection::Table).await?;

    drop_index(
        manager,
        "idx-workspace-channel-projection-conversation-id",
        WorkspaceChannelProjection::Table,
    )
    .await?;
    drop_index(
        manager,
        "idx-workspace-channel-projection-user-workspace-position",
        WorkspaceChannelProjection::Table,
    )
    .await?;
    drop_table(manager, WorkspaceChannelProjection::Table).await?;

    drop_index(
        manager,
        "idx-workspace-projection-user-name-id",
        WorkspaceProjection::Table,
    )
    .await?;
    drop_table(manager, WorkspaceProjection::Table).await?;
    drop_table(manager, UserAppProjection::Table).await?;

    Ok(())
}

async fn drop_unread_projection_tables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    drop_table(manager, WorkspaceUnreadProjection::Table).await?;
    drop_index(
        manager,
        "idx-dm-unread-projection-conversation-id",
        DmUnreadProjection::Table,
    )
    .await?;
    drop_table(manager, DmUnreadProjection::Table).await?;
    drop_index(
        manager,
        "idx-workspace-channel-unread-conversation-id",
        WorkspaceChannelUnreadProjection::Table,
    )
    .await?;
    drop_index(
        manager,
        "idx-workspace-channel-unread-user-workspace",
        WorkspaceChannelUnreadProjection::Table,
    )
    .await?;
    drop_table(manager, WorkspaceChannelUnreadProjection::Table).await?;

    Ok(())
}

async fn drop_source_snapshot_tables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    drop_index(
        manager,
        "idx-conversation-read-state-user-id",
        ConversationReadState::Table,
    )
    .await?;
    drop_table(manager, ConversationReadState::Table).await?;
    drop_index(
        manager,
        "idx-conversation-message-state-last-message-id",
        ConversationMessageState::Table,
    )
    .await?;
    drop_table(manager, ConversationMessageState::Table).await?;
    drop_index(
        manager,
        "idx-conversation-snapshot-workspace-channel-id",
        ConversationSnapshot::Table,
    )
    .await?;
    drop_index(
        manager,
        "idx-conversation-snapshot-dm-pair-id",
        ConversationSnapshot::Table,
    )
    .await?;
    drop_table(manager, ConversationSnapshot::Table).await?;
    drop_index(
        manager,
        "idx-dm-pair-snapshot-high-user-id",
        DmPairSnapshot::Table,
    )
    .await?;
    drop_index(
        manager,
        "idx-dm-pair-snapshot-low-user-id",
        DmPairSnapshot::Table,
    )
    .await?;
    drop_table(manager, DmPairSnapshot::Table).await?;
    drop_index(
        manager,
        "idx-workspace-channel-snapshot-workspace-position",
        WorkspaceChannelSnapshot::Table,
    )
    .await?;
    drop_table(manager, WorkspaceChannelSnapshot::Table).await?;
    drop_index(
        manager,
        "idx-workspace-member-snapshot-workspace-status",
        WorkspaceMemberSnapshot::Table,
    )
    .await?;
    drop_index(
        manager,
        "idx-workspace-member-snapshot-user-status",
        WorkspaceMemberSnapshot::Table,
    )
    .await?;
    drop_table(manager, WorkspaceMemberSnapshot::Table).await?;
    drop_table(manager, WorkspaceSnapshot::Table).await?;
    drop_index(
        manager,
        "idx-friend-request-snapshot-addressee-status",
        FriendRequestSnapshot::Table,
    )
    .await?;
    drop_table(manager, FriendRequestSnapshot::Table).await?;
    drop_table(manager, UserSnapshot::Table).await?;

    Ok(())
}

async fn drop_control_tables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    drop_index(manager, "idx-compose-queue-dm-pair-id", ComposeQueue::Table).await?;
    drop_index(
        manager,
        "idx-compose-queue-conversation-id",
        ComposeQueue::Table,
    )
    .await?;
    drop_index(
        manager,
        "idx-compose-queue-workspace-id",
        ComposeQueue::Table,
    )
    .await?;
    drop_index(manager, "idx-compose-queue-kind-user", ComposeQueue::Table).await?;
    drop_index(
        manager,
        "idx-compose-queue-status-available-at",
        ComposeQueue::Table,
    )
    .await?;
    drop_table(manager, ComposeQueue::Table).await?;
    drop_table(manager, ProcessedEvent::Table).await?;

    Ok(())
}

async fn create_index<I, C>(
    manager: &SchemaManager<'_>,
    name: &str,
    table: I,
    columns: impl IntoIterator<Item = C>,
) -> Result<(), DbErr>
where
    I: Iden + 'static,
    C: Iden + 'static,
{
    let mut index = Index::create();
    index.if_not_exists().name(name).table(table);
    for column in columns {
        index.col(column);
    }
    manager.create_index(index.to_owned()).await
}

async fn drop_index<I>(manager: &SchemaManager<'_>, name: &str, table: I) -> Result<(), DbErr>
where
    I: Iden + 'static,
{
    manager
        .drop_index(Index::drop().name(name).table(table).to_owned())
        .await
}

async fn drop_table<I>(manager: &SchemaManager<'_>, table: I) -> Result<(), DbErr>
where
    I: Iden + 'static,
{
    manager
        .drop_table(Table::drop().table(table).to_owned())
        .await
}

#[derive(DeriveIden)]
enum ProcessedEvent {
    Table,
    EventId,
    RoutingKey,
    ProcessedAt,
}

#[derive(DeriveIden)]
enum ComposeQueue {
    Table,
    ComposeKey,
    ComposeKind,
    UserId,
    WorkspaceId,
    ChannelId,
    ConversationId,
    DmPairId,
    Status,
    Attempts,
    AvailableAt,
    ClaimedAt,
    LastError,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum UserSnapshot {
    Table,
    UserId,
    Username,
    DisplayName,
    AvatarUrl,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum FriendRequestSnapshot {
    Table,
    FriendRequestId,
    RequesterUserId,
    AddresseeUserId,
    Status,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum WorkspaceSnapshot {
    Table,
    WorkspaceId,
    Name,
    IconUrl,
    OwnerUserId,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum WorkspaceMemberSnapshot {
    Table,
    WorkspaceId,
    UserId,
    Status,
    JoinedAt,
    RemovedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum WorkspaceChannelSnapshot {
    Table,
    ChannelId,
    WorkspaceId,
    Name,
    ChannelKind,
    Position,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum DmPairSnapshot {
    Table,
    DmPairId,
    LowUserId,
    HighUserId,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum ConversationSnapshot {
    Table,
    ConversationId,
    TargetType,
    DmPairId,
    WorkspaceChannelId,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum ConversationMessageState {
    Table,
    ConversationId,
    LastMessageId,
    LastMessageAuthorUserId,
    LastMessageSeq,
    LastMessagePreview,
    LastActivityAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum ConversationReadState {
    Table,
    ConversationId,
    UserId,
    LastReadConversationMessageSeq,
    ReadAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum WorkspaceChannelUnreadProjection {
    Table,
    UserId,
    WorkspaceId,
    ChannelId,
    ConversationId,
    LastMessageSeq,
    LastReadConversationMessageSeq,
    UnreadCount,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum DmUnreadProjection {
    Table,
    UserId,
    DmPairId,
    ConversationId,
    LastMessageSeq,
    LastReadConversationMessageSeq,
    UnreadCount,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum WorkspaceUnreadProjection {
    Table,
    UserId,
    WorkspaceId,
    UnreadCount,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum UserAppProjection {
    Table,
    UserId,
    Username,
    DisplayName,
    AvatarUrl,
    PendingFriendRequestCount,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum WorkspaceProjection {
    Table,
    UserId,
    WorkspaceId,
    WorkspaceName,
    WorkspaceIconUrl,
    MemberCount,
    UnreadCount,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum WorkspaceChannelProjection {
    Table,
    UserId,
    WorkspaceId,
    ChannelId,
    ConversationId,
    ChannelName,
    ChannelKind,
    Position,
    LastMessageSeq,
    LastReadConversationMessageSeq,
    UnreadCount,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum DmProjection {
    Table,
    UserId,
    ConversationId,
    DmPairId,
    PeerUserId,
    PeerUsername,
    PeerDisplayName,
    PeerAvatarUrl,
    LastMessageSeq,
    LastReadConversationMessageSeq,
    LastMessagePreview,
    LastActivityAt,
    UnreadCount,
    UpdatedAt,
}
