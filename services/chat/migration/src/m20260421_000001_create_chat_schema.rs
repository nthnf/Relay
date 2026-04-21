use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UserSnapshot::Table)
                    .if_not_exists()
                    .col(uuid(UserSnapshot::UserId).not_null().primary_key())
                    .col(
                        timestamp_with_time_zone(UserSnapshot::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
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
                    .table(WorkspaceSnapshot::Table)
                    .if_not_exists()
                    .col(
                        uuid(WorkspaceSnapshot::WorkspaceId)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        timestamp_with_time_zone(WorkspaceSnapshot::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
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
                    .table(WorkspaceChannelSnapshot::Table)
                    .if_not_exists()
                    .col(
                        uuid(WorkspaceChannelSnapshot::WorkspaceChannelId)
                            .not_null()
                            .primary_key(),
                    )
                    .col(uuid(WorkspaceChannelSnapshot::WorkspaceId).not_null())
                    .col(text(WorkspaceChannelSnapshot::ChannelKind).not_null())
                    .col(
                        timestamp_with_time_zone(WorkspaceChannelSnapshot::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        timestamp_with_time_zone(WorkspaceChannelSnapshot::UpdatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Conversation::Table)
                    .if_not_exists()
                    .col(uuid(Conversation::Id).not_null().primary_key())
                    .col(text(Conversation::TargetType).not_null())
                    .col(uuid_null(Conversation::WorkspaceChannelId))
                    .col(uuid(Conversation::CreatedByUserId).not_null())
                    .col(
                        timestamp_with_time_zone(Conversation::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                ALTER TABLE "conversation"
                ADD CONSTRAINT "ck-conversation-target-type"
                CHECK ("target_type" IN ('dm', 'workspace_channel'))
                "#,
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                ALTER TABLE "conversation"
                ADD CONSTRAINT "ck-conversation-target-shape"
                CHECK (
                    (
                        "target_type" = 'dm'
                        AND "workspace_channel_id" IS NULL
                    )
                    OR (
                        "target_type" = 'workspace_channel'
                        AND "workspace_channel_id" IS NOT NULL
                    )
                )
                "#,
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("uq-conversation-workspace-channel-id")
                    .table(Conversation::Table)
                    .col(Conversation::WorkspaceChannelId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ConversationMember::Table)
                    .if_not_exists()
                    .col(uuid(ConversationMember::Id).not_null().primary_key())
                    .col(uuid(ConversationMember::ConversationId).not_null())
                    .col(uuid(ConversationMember::UserId).not_null())
                    .col(
                        timestamp_with_time_zone(ConversationMember::JoinedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-conversation-member-conversation-id")
                            .from(
                                ConversationMember::Table,
                                ConversationMember::ConversationId,
                            )
                            .to(Conversation::Table, Conversation::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("uq-conversation-member-conversation-user")
                    .table(ConversationMember::Table)
                    .col(ConversationMember::ConversationId)
                    .col(ConversationMember::UserId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ChatMessage::Table)
                    .if_not_exists()
                    .col(uuid(ChatMessage::MessageId).not_null().primary_key())
                    .col(uuid(ChatMessage::ConversationId).not_null())
                    .col(uuid(ChatMessage::AuthorUserId).not_null())
                    .col(text_null(ChatMessage::ClientMessageId))
                    .col(big_integer(ChatMessage::ConversationMessageSeq).not_null())
                    .col(text(ChatMessage::Body).not_null())
                    .col(
                        text(ChatMessage::MessageStatus)
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        timestamp_with_time_zone(ChatMessage::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        timestamp_with_time_zone(ChatMessage::UpdatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(timestamp_with_time_zone(ChatMessage::DeletedAt).null())
                    .col(uuid_null(ChatMessage::DeletedByUserId))
                    .col(timestamp_with_time_zone(ChatMessage::LastEditedAt).null())
                    .col(uuid_null(ChatMessage::LastEditedByUserId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-chat-message-conversation-id")
                            .from(ChatMessage::Table, ChatMessage::ConversationId)
                            .to(Conversation::Table, Conversation::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                ALTER TABLE "chat_message"
                ADD CONSTRAINT "ck-chat-message-status"
                CHECK ("message_status" IN ('active', 'deleted'))
                "#,
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("uq-chat-message-conversation-seq")
                    .table(ChatMessage::Table)
                    .col(ChatMessage::ConversationId)
                    .col(ChatMessage::ConversationMessageSeq)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("uq-chat-message-idempotency")
                    .table(ChatMessage::Table)
                    .col(ChatMessage::AuthorUserId)
                    .col(ChatMessage::ConversationId)
                    .col(ChatMessage::ClientMessageId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(OutboxEvent::Table)
                    .if_not_exists()
                    .col(uuid(OutboxEvent::EventId).not_null().primary_key())
                    .col(text(OutboxEvent::AggregateType).not_null())
                    .col(uuid(OutboxEvent::AggregateId).not_null())
                    .col(text(OutboxEvent::EventType).not_null())
                    .col(json_binary(OutboxEvent::Payload).not_null())
                    .col(text(OutboxEvent::Status).not_null().default("pending"))
                    .col(integer(OutboxEvent::PublishAttempts).not_null().default(0))
                    .col(
                        timestamp_with_time_zone(OutboxEvent::OccurredAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        timestamp_with_time_zone(OutboxEvent::AvailableAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(text_null(OutboxEvent::ClaimedBy))
                    .col(timestamp_with_time_zone(OutboxEvent::ClaimedAt).null())
                    .col(timestamp_with_time_zone(OutboxEvent::PublishedAt).null())
                    .col(text_null(OutboxEvent::LastError))
                    .col(
                        timestamp_with_time_zone(OutboxEvent::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OutboxEvent::Table).to_owned())
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("uq-chat-message-idempotency")
                    .table(ChatMessage::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("uq-chat-message-conversation-seq")
                    .table(ChatMessage::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(ChatMessage::Table).to_owned())
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("uq-conversation-member-conversation-user")
                    .table(ConversationMember::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(ConversationMember::Table).to_owned())
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("uq-conversation-workspace-channel-id")
                    .table(Conversation::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Conversation::Table).to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(WorkspaceChannelSnapshot::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(WorkspaceSnapshot::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(UserSnapshot::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum UserSnapshot {
    Table,
    UserId,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum WorkspaceSnapshot {
    Table,
    WorkspaceId,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum WorkspaceChannelSnapshot {
    Table,
    WorkspaceChannelId,
    WorkspaceId,
    ChannelKind,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Conversation {
    Table,
    Id,
    TargetType,
    WorkspaceChannelId,
    CreatedByUserId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum ConversationMember {
    Table,
    Id,
    ConversationId,
    UserId,
    JoinedAt,
}

#[derive(DeriveIden)]
enum ChatMessage {
    Table,
    MessageId,
    ConversationId,
    AuthorUserId,
    ClientMessageId,
    ConversationMessageSeq,
    Body,
    MessageStatus,
    CreatedAt,
    UpdatedAt,
    DeletedAt,
    DeletedByUserId,
    LastEditedAt,
    LastEditedByUserId,
}

#[derive(DeriveIden)]
enum OutboxEvent {
    Table,
    EventId,
    AggregateType,
    AggregateId,
    EventType,
    Payload,
    Status,
    PublishAttempts,
    OccurredAt,
    AvailableAt,
    ClaimedBy,
    ClaimedAt,
    PublishedAt,
    LastError,
    CreatedAt,
}
