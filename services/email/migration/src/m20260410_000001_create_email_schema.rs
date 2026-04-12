use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OutboundEmail::Table)
                    .if_not_exists()
                    .col(uuid(OutboundEmail::Id).not_null().primary_key())
                    .col(text(OutboundEmail::DedupeKey).not_null())
                    .col(text(OutboundEmail::EmailKind).not_null())
                    .col(uuid_null(OutboundEmail::RecipientUserId))
                    .col(text(OutboundEmail::RecipientEmail).not_null())
                    .col(text_null(OutboundEmail::ProviderMessageId))
                    .col(text_null(OutboundEmail::ProviderName))
                    .col(text(OutboundEmail::TemplateKey).not_null())
                    .col(integer(OutboundEmail::TemplateVersion).not_null())
                    .col(text(OutboundEmail::Subject).not_null())
                    .col(text(OutboundEmail::BodyText).not_null())
                    .col(text_null(OutboundEmail::BodyHtml))
                    .col(text(OutboundEmail::SourceEventType).not_null())
                    .col(text(OutboundEmail::SourceEventId).not_null())
                    .col(timestamp_with_time_zone(OutboundEmail::SourceOccurredAt).not_null())
                    .col(
                        text(OutboundEmail::SendStatus)
                            .not_null()
                            .default("pending"),
                    )
                    .col(text_null(OutboundEmail::LastErrorCode))
                    .col(text_null(OutboundEmail::LastErrorMessage))
                    .col(timestamp_with_time_zone(OutboundEmail::NextAttemptAfter).null())
                    .col(
                        timestamp_with_time_zone(OutboundEmail::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        timestamp_with_time_zone(OutboundEmail::UpdatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq-outbound-email-dedupe-key")
                    .table(OutboundEmail::Table)
                    .col(OutboundEmail::DedupeKey)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-outbound-email-send-status-next-attempt-after")
                    .table(OutboundEmail::Table)
                    .col(OutboundEmail::SendStatus)
                    .col(OutboundEmail::NextAttemptAfter)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-outbound-email-source-event-id")
                    .table(OutboundEmail::Table)
                    .col(OutboundEmail::SourceEventId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(EmailDeliveryAttempt::Table)
                    .if_not_exists()
                    .col(uuid(EmailDeliveryAttempt::Id).not_null().primary_key())
                    .col(uuid(EmailDeliveryAttempt::OutboundEmailId).not_null())
                    .col(integer(EmailDeliveryAttempt::AttemptNumber).not_null())
                    .col(text(EmailDeliveryAttempt::ProviderName).not_null())
                    .col(text_null(EmailDeliveryAttempt::ProviderMessageId))
                    .col(text(EmailDeliveryAttempt::AttemptStatus).not_null())
                    .col(text_null(EmailDeliveryAttempt::FailureCode))
                    .col(text_null(EmailDeliveryAttempt::FailureMessage))
                    .col(timestamp_with_time_zone(EmailDeliveryAttempt::AttemptedAt).not_null())
                    .col(json_binary_null(
                        EmailDeliveryAttempt::ProviderResponseSnapshot,
                    ))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-email-delivery-attempt-outbound-email-id")
                            .from(
                                EmailDeliveryAttempt::Table,
                                EmailDeliveryAttempt::OutboundEmailId,
                            )
                            .to(OutboundEmail::Table, OutboundEmail::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq-email-delivery-attempt-outbound-email-id-attempt-number")
                    .table(EmailDeliveryAttempt::Table)
                    .col(EmailDeliveryAttempt::OutboundEmailId)
                    .col(EmailDeliveryAttempt::AttemptNumber)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-email-delivery-attempt-outbound-email-id")
                    .table(EmailDeliveryAttempt::Table)
                    .col(EmailDeliveryAttempt::OutboundEmailId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(EmailDeliveryAttempt::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(OutboundEmail::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum OutboundEmail {
    Table,
    Id,
    DedupeKey,
    EmailKind,
    RecipientUserId,
    RecipientEmail,
    ProviderMessageId,
    ProviderName,
    TemplateKey,
    TemplateVersion,
    Subject,
    BodyText,
    BodyHtml,
    SourceEventType,
    SourceEventId,
    SourceOccurredAt,
    SendStatus,
    LastErrorCode,
    LastErrorMessage,
    NextAttemptAfter,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum EmailDeliveryAttempt {
    Table,
    Id,
    OutboundEmailId,
    AttemptNumber,
    ProviderName,
    ProviderMessageId,
    AttemptStatus,
    FailureCode,
    FailureMessage,
    AttemptedAt,
    ProviderResponseSnapshot,
}
