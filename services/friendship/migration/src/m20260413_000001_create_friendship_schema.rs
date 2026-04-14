use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FriendRequest::Table)
                    .if_not_exists()
                    .col(
                        uuid(FriendRequest::FriendRequestId)
                            .not_null()
                            .primary_key(),
                    )
                    .col(uuid(FriendRequest::RequesterUserId).not_null())
                    .col(uuid(FriendRequest::AddresseeUserId).not_null())
                    .col(text(FriendRequest::Status).not_null())
                    .col(
                        timestamp_with_time_zone(FriendRequest::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(timestamp_with_time_zone(FriendRequest::ResolvedAt).null())
                    .col(text_null(FriendRequest::ResolutionReason))
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE UNIQUE INDEX IF NOT EXISTS "uq-friend-request-active-pair"
                ON "friend_request" (
                    LEAST("requester_user_id", "addressee_user_id"),
                    GREATEST("requester_user_id", "addressee_user_id")
                )
                WHERE "status" = 'pending'
                "#,
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-friend-request-requester-status-created-at")
                    .table(FriendRequest::Table)
                    .col(FriendRequest::RequesterUserId)
                    .col(FriendRequest::Status)
                    .col(FriendRequest::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-friend-request-addressee-status-created-at")
                    .table(FriendRequest::Table)
                    .col(FriendRequest::AddresseeUserId)
                    .col(FriendRequest::Status)
                    .col(FriendRequest::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(FriendshipEdge::Table)
                    .if_not_exists()
                    .col(uuid(FriendshipEdge::UserId).not_null())
                    .col(uuid(FriendshipEdge::FriendUserId).not_null())
                    .col(uuid(FriendshipEdge::FriendRequestId).not_null())
                    .col(
                        timestamp_with_time_zone(FriendshipEdge::AcceptedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        timestamp_with_time_zone(FriendshipEdge::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(FriendshipEdge::UserId)
                            .col(FriendshipEdge::FriendUserId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-friendship-edge-friend-request-id")
                            .from(FriendshipEdge::Table, FriendshipEdge::FriendRequestId)
                            .to(FriendRequest::Table, FriendRequest::FriendRequestId)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-friendship-edge-user-id-accepted-at")
                    .table(FriendshipEdge::Table)
                    .col(FriendshipEdge::UserId)
                    .col(FriendshipEdge::AcceptedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-friendship-edge-friend-user-id")
                    .table(FriendshipEdge::Table)
                    .col(FriendshipEdge::FriendUserId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(UserBlock::Table)
                    .if_not_exists()
                    .col(uuid(UserBlock::BlockerUserId).not_null())
                    .col(uuid(UserBlock::BlockedUserId).not_null())
                    .col(
                        timestamp_with_time_zone(UserBlock::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(text_null(UserBlock::Reason))
                    .primary_key(
                        Index::create()
                            .col(UserBlock::BlockerUserId)
                            .col(UserBlock::BlockedUserId),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-user-block-blocked-user-id")
                    .table(UserBlock::Table)
                    .col(UserBlock::BlockedUserId)
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

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-outbox-event-status-available-at")
                    .table(OutboxEvent::Table)
                    .col(OutboxEvent::Status)
                    .col(OutboxEvent::AvailableAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-outbox-event-status-claimed-at")
                    .table(OutboxEvent::Table)
                    .col(OutboxEvent::Status)
                    .col(OutboxEvent::ClaimedAt)
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
            .drop_table(Table::drop().table(UserBlock::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(FriendshipEdge::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(FriendRequest::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum FriendRequest {
    Table,
    FriendRequestId,
    RequesterUserId,
    AddresseeUserId,
    Status,
    CreatedAt,
    ResolvedAt,
    ResolutionReason,
}

#[derive(DeriveIden)]
enum FriendshipEdge {
    Table,
    UserId,
    FriendUserId,
    FriendRequestId,
    AcceptedAt,
    CreatedAt,
}

#[derive(DeriveIden)]
enum UserBlock {
    Table,
    BlockerUserId,
    BlockedUserId,
    CreatedAt,
    Reason,
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
