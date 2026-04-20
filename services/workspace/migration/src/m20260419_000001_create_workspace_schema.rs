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
                    .col(boolean(UserSnapshot::EmailVerified).not_null())
                    .col(text(UserSnapshot::Username).not_null())
                    .col(text(UserSnapshot::DisplayName).not_null())
                    .col(text_null(UserSnapshot::AvatarUrl))
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
                    .table(Workspace::Table)
                    .if_not_exists()
                    .col(uuid(Workspace::Id).not_null().primary_key())
                    .col(uuid(Workspace::OwnerUserId).not_null())
                    .col(text(Workspace::Name).not_null())
                    .col(text_null(Workspace::IconUrl))
                    .col(
                        timestamp_with_time_zone(Workspace::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        timestamp_with_time_zone(Workspace::UpdatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(timestamp_with_time_zone(Workspace::ArchivedAt).null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-workspace-owner-user-id")
                            .from(Workspace::Table, Workspace::OwnerUserId)
                            .to(UserSnapshot::Table, UserSnapshot::UserId)
                            .on_delete(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(WorkspaceInviteLink::Table)
                    .if_not_exists()
                    .col(uuid(WorkspaceInviteLink::Id).not_null().primary_key())
                    .col(uuid(WorkspaceInviteLink::WorkspaceId).not_null())
                    .col(text(WorkspaceInviteLink::Code).not_null())
                    .col(uuid(WorkspaceInviteLink::CreatedByUserId).not_null())
                    .col(
                        text(WorkspaceInviteLink::Status)
                            .not_null()
                            .default("active"),
                    )
                    .col(timestamp_with_time_zone(WorkspaceInviteLink::ExpiresAt).null())
                    .col(integer(WorkspaceInviteLink::MaxUses).null())
                    .col(integer(WorkspaceInviteLink::UseCount).not_null().default(0))
                    .col(
                        timestamp_with_time_zone(WorkspaceInviteLink::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(timestamp_with_time_zone(WorkspaceInviteLink::RevokedAt).null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-workspace-invite-link-workspace-id")
                            .from(WorkspaceInviteLink::Table, WorkspaceInviteLink::WorkspaceId)
                            .to(Workspace::Table, Workspace::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-workspace-invite-link-created-by-user-id")
                            .from(
                                WorkspaceInviteLink::Table,
                                WorkspaceInviteLink::CreatedByUserId,
                            )
                            .to(UserSnapshot::Table, UserSnapshot::UserId)
                            .on_delete(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("uq-workspace-invite-link-code")
                    .table(WorkspaceInviteLink::Table)
                    .col(WorkspaceInviteLink::Code)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(WorkspaceMember::Table)
                    .if_not_exists()
                    .col(uuid(WorkspaceMember::WorkspaceId).not_null())
                    .col(uuid(WorkspaceMember::UserId).not_null())
                    .col(
                        text(WorkspaceMember::MembershipStatus)
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        timestamp_with_time_zone(WorkspaceMember::JoinedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(timestamp_with_time_zone(WorkspaceMember::RemovedAt).null())
                    .col(uuid_null(WorkspaceMember::AddedByUserId))
                    .primary_key(
                        Index::create()
                            .col(WorkspaceMember::WorkspaceId)
                            .col(WorkspaceMember::UserId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-workspace-member-workspace-id")
                            .from(WorkspaceMember::Table, WorkspaceMember::WorkspaceId)
                            .to(Workspace::Table, Workspace::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-workspace-member-user-id")
                            .from(WorkspaceMember::Table, WorkspaceMember::UserId)
                            .to(UserSnapshot::Table, UserSnapshot::UserId)
                            .on_delete(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-workspace-member-added-by-user-id")
                            .from(WorkspaceMember::Table, WorkspaceMember::AddedByUserId)
                            .to(UserSnapshot::Table, UserSnapshot::UserId)
                            .on_delete(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(WorkspaceRole::Table)
                    .if_not_exists()
                    .col(uuid(WorkspaceRole::Id).not_null().primary_key())
                    .col(uuid(WorkspaceRole::WorkspaceId).not_null())
                    .col(text(WorkspaceRole::Name).not_null())
                    .col(integer(WorkspaceRole::Permissions).not_null())
                    .col(
                        boolean(WorkspaceRole::IsSystemRole)
                            .not_null()
                            .default(false),
                    )
                    .col(
                        timestamp_with_time_zone(WorkspaceRole::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-workspace-role-workspace-id")
                            .from(WorkspaceRole::Table, WorkspaceRole::WorkspaceId)
                            .to(Workspace::Table, Workspace::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                ALTER TABLE "workspace_role"
                ADD CONSTRAINT "ck-workspace-role-permissions-nonnegative"
                CHECK ("permissions" >= 0)
                "#,
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("uq-workspace-role-name")
                    .table(WorkspaceRole::Table)
                    .col(WorkspaceRole::WorkspaceId)
                    .col(WorkspaceRole::Name)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(WorkspaceMemberRole::Table)
                    .if_not_exists()
                    .col(uuid(WorkspaceMemberRole::WorkspaceId).not_null())
                    .col(uuid(WorkspaceMemberRole::UserId).not_null())
                    .col(uuid(WorkspaceMemberRole::RoleId).not_null())
                    .col(
                        timestamp_with_time_zone(WorkspaceMemberRole::AssignedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(uuid_null(WorkspaceMemberRole::AssignedByUserId))
                    .primary_key(
                        Index::create()
                            .col(WorkspaceMemberRole::WorkspaceId)
                            .col(WorkspaceMemberRole::UserId)
                            .col(WorkspaceMemberRole::RoleId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-workspace-member-role-workspace-id")
                            .from(WorkspaceMemberRole::Table, WorkspaceMemberRole::WorkspaceId)
                            .to(Workspace::Table, Workspace::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-workspace-member-role-user-id")
                            .from(WorkspaceMemberRole::Table, WorkspaceMemberRole::UserId)
                            .to(UserSnapshot::Table, UserSnapshot::UserId)
                            .on_delete(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-workspace-member-role-assigned-by-user-id")
                            .from(
                                WorkspaceMemberRole::Table,
                                WorkspaceMemberRole::AssignedByUserId,
                            )
                            .to(UserSnapshot::Table, UserSnapshot::UserId)
                            .on_delete(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-workspace-member-role-workspace-role-id")
                            .from(WorkspaceMemberRole::Table, WorkspaceMemberRole::RoleId)
                            .to(WorkspaceRole::Table, WorkspaceRole::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(WorkspaceInvitation::Table)
                    .if_not_exists()
                    .col(uuid(WorkspaceInvitation::Id).not_null().primary_key())
                    .col(uuid(WorkspaceInvitation::WorkspaceId).not_null())
                    .col(uuid(WorkspaceInvitation::IssuedToUserId).not_null())
                    .col(uuid(WorkspaceInvitation::IssuedByUserId).not_null())
                    .col(
                        text(WorkspaceInvitation::Status)
                            .not_null()
                            .default("pending"),
                    )
                    .col(timestamp_with_time_zone(WorkspaceInvitation::ExpiresAt).not_null())
                    .col(timestamp_with_time_zone(WorkspaceInvitation::AcceptedAt).null())
                    .col(
                        timestamp_with_time_zone(WorkspaceInvitation::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-workspace-invitation-workspace-id")
                            .from(WorkspaceInvitation::Table, WorkspaceInvitation::WorkspaceId)
                            .to(Workspace::Table, Workspace::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-workspace-invitation-issued-to-user-id")
                            .from(
                                WorkspaceInvitation::Table,
                                WorkspaceInvitation::IssuedToUserId,
                            )
                            .to(UserSnapshot::Table, UserSnapshot::UserId)
                            .on_delete(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-workspace-invitation-issued-by-user-id")
                            .from(
                                WorkspaceInvitation::Table,
                                WorkspaceInvitation::IssuedByUserId,
                            )
                            .to(UserSnapshot::Table, UserSnapshot::UserId)
                            .on_delete(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(WorkspaceChannel::Table)
                    .if_not_exists()
                    .col(uuid(WorkspaceChannel::ChannelId).not_null().primary_key())
                    .col(uuid(WorkspaceChannel::WorkspaceId).not_null())
                    .col(text(WorkspaceChannel::Name).not_null())
                    .col(text(WorkspaceChannel::ChannelKind).not_null())
                    .col(integer(WorkspaceChannel::Position).not_null())
                    .col(uuid(WorkspaceChannel::CreatedByUserId).not_null())
                    .col(
                        timestamp_with_time_zone(WorkspaceChannel::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(timestamp_with_time_zone(WorkspaceChannel::ArchivedAt).null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-workspace-channel-workspace-id")
                            .from(WorkspaceChannel::Table, WorkspaceChannel::WorkspaceId)
                            .to(Workspace::Table, Workspace::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-workspace-channel-created-by-user-id")
                            .from(WorkspaceChannel::Table, WorkspaceChannel::CreatedByUserId)
                            .to(UserSnapshot::Table, UserSnapshot::UserId)
                            .on_delete(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE UNIQUE INDEX IF NOT EXISTS "uq-workspace-invitation-pending-target"
                ON "workspace_invitation" ("workspace_id", "issued_to_user_id")
                WHERE "status" = 'pending'
                "#,
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE UNIQUE INDEX IF NOT EXISTS "uq-workspace-channel-active-position"
                ON "workspace_channel" ("workspace_id", "position")
                WHERE "archived_at" IS NULL
                "#,
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
            .get_connection()
            .execute_unprepared(
                r#"
                ALTER TABLE "workspace_role"
                DROP CONSTRAINT IF EXISTS "ck-workspace-role-permissions-nonnegative"
                "#,
            )
            .await?;

        manager
            .drop_table(Table::drop().table(OutboxEvent::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(WorkspaceChannel::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(WorkspaceInviteLink::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(WorkspaceInvitation::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(WorkspaceMemberRole::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(WorkspaceRole::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(WorkspaceMember::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Workspace::Table).to_owned())
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
    EmailVerified,
    Username,
    DisplayName,
    AvatarUrl,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Workspace {
    Table,
    Id,
    OwnerUserId,
    Name,
    IconUrl,
    CreatedAt,
    UpdatedAt,
    ArchivedAt,
}

#[derive(DeriveIden)]
enum WorkspaceInviteLink {
    Table,
    Id,
    WorkspaceId,
    Code,
    CreatedByUserId,
    Status,
    ExpiresAt,
    MaxUses,
    UseCount,
    CreatedAt,
    RevokedAt,
}

#[derive(DeriveIden)]
enum WorkspaceMember {
    Table,
    WorkspaceId,
    UserId,
    MembershipStatus,
    JoinedAt,
    RemovedAt,
    AddedByUserId,
}

#[derive(DeriveIden)]
enum WorkspaceRole {
    Table,
    Id,
    WorkspaceId,
    Name,
    Permissions,
    IsSystemRole,
    CreatedAt,
}

#[derive(DeriveIden)]
enum WorkspaceMemberRole {
    Table,
    WorkspaceId,
    UserId,
    RoleId,
    AssignedAt,
    AssignedByUserId,
}

#[derive(DeriveIden)]
enum WorkspaceInvitation {
    Table,
    Id,
    WorkspaceId,
    IssuedToUserId,
    IssuedByUserId,
    Status,
    ExpiresAt,
    AcceptedAt,
    CreatedAt,
}

#[derive(DeriveIden)]
enum WorkspaceChannel {
    Table,
    ChannelId,
    WorkspaceId,
    Name,
    ChannelKind,
    Position,
    CreatedByUserId,
    CreatedAt,
    ArchivedAt,
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
