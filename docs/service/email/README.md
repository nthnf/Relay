## Purpose

Email owns outbound email intent handling, provider handoff, delivery attempts, and email-related operational state for registration and workspace-invitation flows. In v1 it is a consume-only service on the durable event bus.

## Owned Responsibilities

- Consume durable registration and invitation events from RabbitMQ.
- Materialize one service-owned `outbound_email` row per accepted email send intent.
- Render concrete verification and workspace-invitation messages from service-owned template/version metadata and self-contained consumed-event payloads.
- Hand outbound messages to a configured email provider through a bounded provider abstraction.
- Record each provider submission or failure in `email_delivery_attempt` for retry and operator inspection.
- Drive retries from email-owned due work on existing `outbound_email` rows, not from RabbitMQ duplicate delivery.
- Expose email-related operational history through service-owned storage only.

## Non-Goals

- Owning user accounts, verification-token issuance, or invitation lifecycle; `identity` and `workspace` own those writes.
- Publishing durable delivery-result integration events in v1; email is consume-only for durable events.
- Sending password-reset, marketing, digest, or generic notification mail in v1.
- Acting as the public client edge or replacing service-owned email state in upstream domains.

## Dependencies

- **RabbitMQ** for durable consumption of registration and invitation events.
- **Postgres** as the service-owned source of truth for outbound email intents and delivery attempts.
- **identity** as the producer of registration and verification-related durable events.
- **workspace** as the producer of workspace invitation durable events.
- **email provider** such as SES, Postmark, or similar behind a service-owned abstraction.

## Storage

- Email owns a dedicated Postgres database.
- Email writes local send state and attempt history only; it does not read upstream service databases.
- Durable consumed events must be handled idempotently so replay does not create duplicate pending sends or duplicate resends.
- Redis is not required by default for v1 email behavior.

## Event Surface

### Consumed

- `UserRegistered`
- `WorkspaceInvitationIssued`

### Published

- None in v1. Email is consume-only for durable events.

See `events.md` for trigger rules and payload expectations.
