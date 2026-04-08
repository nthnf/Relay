use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UserAccount::Table)
                    .if_not_exists()
                    .col(uuid(UserAccount::UserId).not_null().primary_key())
                    .col(text(UserAccount::Email).not_null())
                    .col(text(UserAccount::EmailNormalized).not_null())
                    .col(timestamp_with_time_zone(UserAccount::EmailVerifiedAt).null())
                    .col(text(UserAccount::AccountStatus).not_null())
                    .col(
                        timestamp_with_time_zone(UserAccount::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        timestamp_with_time_zone(UserAccount::UpdatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq-user-account-email-normalized")
                    .table(UserAccount::Table)
                    .col(UserAccount::EmailNormalized)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(UserProfile::Table)
                    .if_not_exists()
                    .col(uuid(UserProfile::UserId).not_null().primary_key())
                    .col(text(UserProfile::Username).not_null())
                    .col(text(UserProfile::DisplayName).not_null())
                    .col(text_null(UserProfile::AvatarUrl))
                    .col(
                        timestamp_with_time_zone(UserProfile::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        timestamp_with_time_zone(UserProfile::UpdatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-user-profile-user-id")
                            .from(UserProfile::Table, UserProfile::UserId)
                            .to(UserAccount::Table, UserAccount::UserId)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq-user-profile-username")
                    .table(UserProfile::Table)
                    .col(UserProfile::Username)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(UserCredentialPassword::Table)
                    .if_not_exists()
                    .col(
                        uuid(UserCredentialPassword::UserId)
                            .not_null()
                            .primary_key(),
                    )
                    .col(text(UserCredentialPassword::PasswordHash).not_null())
                    .col(
                        timestamp_with_time_zone(UserCredentialPassword::PasswordUpdatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        integer(UserCredentialPassword::FailedAttemptCount)
                            .not_null()
                            .default(0),
                    )
                    .col(
                        timestamp_with_time_zone(UserCredentialPassword::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        timestamp_with_time_zone(UserCredentialPassword::UpdatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-user-credential-password-user-id")
                            .from(
                                UserCredentialPassword::Table,
                                UserCredentialPassword::UserId,
                            )
                            .to(UserAccount::Table, UserAccount::UserId)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(UserSession::Table)
                    .if_not_exists()
                    .col(uuid(UserSession::SessionId).not_null().primary_key())
                    .col(uuid(UserSession::UserId).not_null())
                    .col(text(UserSession::RefreshTokenHash).not_null())
                    .col(
                        timestamp_with_time_zone(UserSession::IssuedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(timestamp_with_time_zone(UserSession::RefreshExpiresAt).not_null())
                    .col(timestamp_with_time_zone(UserSession::RevokedAt).null())
                    .col(text_null(UserSession::RevokeReason))
                    .col(uuid_null(UserSession::ReplacedBySessionId))
                    .col(uuid_null(UserSession::ClientInstanceId))
                    .col(
                        timestamp_with_time_zone(UserSession::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-user-session-user-id")
                            .from(UserSession::Table, UserSession::UserId)
                            .to(UserAccount::Table, UserAccount::UserId)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(EmailVerificationToken::Table)
                    .if_not_exists()
                    .col(
                        uuid(EmailVerificationToken::TokenId)
                            .not_null()
                            .primary_key(),
                    )
                    .col(uuid(EmailVerificationToken::UserId).not_null())
                    .col(text(EmailVerificationToken::TokenHash).not_null())
                    .col(timestamp_with_time_zone(EmailVerificationToken::ExpiresAt).not_null())
                    .col(timestamp_with_time_zone(EmailVerificationToken::ConsumedAt).null())
                    .col(
                        timestamp_with_time_zone(EmailVerificationToken::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-email-verification-token-user-id")
                            .from(
                                EmailVerificationToken::Table,
                                EmailVerificationToken::UserId,
                            )
                            .to(UserAccount::Table, UserAccount::UserId)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
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
            .drop_table(
                Table::drop()
                    .table(EmailVerificationToken::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(UserSession::Table).to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(UserCredentialPassword::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(UserProfile::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(UserAccount::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum UserAccount {
    Table,
    UserId,
    Email,
    EmailNormalized,
    EmailVerifiedAt,
    AccountStatus,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum UserProfile {
    Table,
    UserId,
    Username,
    DisplayName,
    AvatarUrl,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum UserCredentialPassword {
    Table,
    UserId,
    PasswordHash,
    PasswordUpdatedAt,
    FailedAttemptCount,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum UserSession {
    Table,
    SessionId,
    UserId,
    RefreshTokenHash,
    IssuedAt,
    RefreshExpiresAt,
    RevokedAt,
    RevokeReason,
    ReplacedBySessionId,
    ClientInstanceId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum EmailVerificationToken {
    Table,
    TokenId,
    UserId,
    TokenHash,
    ExpiresAt,
    ConsumedAt,
    CreatedAt,
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
