## Persistence Scope

Email owns outbound email send state and delivery-attempt history only. Other services must not read these tables directly; upstream domains remain the source of truth for accounts, verification tokens, and invitations.

## Core Tables

### `outbound_email`

One row per durable email intent accepted by the email service.

| Column | Type | Notes |
| --- | --- | --- |
| `id` | `uuid` | Primary key. Service-owned email send identifier. |
| `dedupe_key` | `text` | Unique idempotency key derived from the consumed event and email purpose. Duplicate broker delivery must resolve to the same row. |
| `email_kind` | `text` | Contract values: `registration_verification`, `workspace_invitation`. |
| `recipient_user_id` | `uuid null` | Identity-owned user reference when the target user already exists. |
| `recipient_email` | `text` | Destination address used for provider submission. |
| `provider_message_id` | `text null` | Provider-owned submission identifier when accepted. |
| `provider_name` | `text null` | Contract value for the configured provider used to send the message. |
| `template_key` | `text` | Concrete template identifier such as `verify-email-v1` or `workspace-invitation-v1`. |
| `template_version` | `integer` | Service-owned template revision used for rendering. |
| `subject` | `text` | Resolved message subject stored for audit and resend consistency. |
| `body_text` | `text` | Resolved text body sent to the provider. |
| `body_html` | `text null` | Optional resolved HTML body sent to the provider. |
| `source_event_type` | `text` | Consumed event contract, such as `VerificationEmailRequested` or `WorkspaceInvitationIssued`. |
| `source_event_id` | `text` | Producer event identifier or broker message identifier used for traceability. |
| `source_occurred_at` | `timestamptz` | Upstream event occurrence time carried into local state. |
| `send_status` | `text` | Contract values: `pending`, `submitted`, `retryable_failure`, `failed`. |
| `last_error_code` | `text null` | Most recent provider or validation failure code. |
| `last_error_message` | `text null` | Most recent operator-visible failure detail. |
| `next_attempt_after` | `timestamptz null` | Earliest time a retry worker or scheduler may retry submission. |
| `created_at` | `timestamptz` | Row creation time. |
| `updated_at` | `timestamptz` | Last local state change time. |

Semantic rules:

- `dedupe_key` must be unique so redelivered RabbitMQ messages do not create duplicate emails for the same durable trigger.
- Duplicate consume of the same durable event is a no-op for enqueue behavior when the matching row already exists in `pending`, `submitted`, `retryable_failure`, or `failed`; email must reuse the row rather than creating a new outbound intent or issuing an extra immediate send.
- `email_kind` is intentionally narrow in v1 and excludes password-reset or marketing scopes.
- `recipient_user_id` is an identity-owned reference only and is nullable so the actual send target remains `recipient_email`.
- `template_key` and `template_version` are concrete, service-owned rendering inputs rather than a general template-management system.
- For `registration_verification`, the rendered public URL is composed by email from configured public-web base URL plus the opaque `verification_token` carried by `VerificationEmailRequested`; email does not perform an identity lookup to obtain link material.
- `send_status = submitted` means the provider accepted the handoff; it does not prove inbox delivery.
- `send_status = retryable_failure` means at least one attempt failed but the email may be retried later.
- `next_attempt_after` is set only for retryable work and is the due-at value consumed by a local scheduler or poller over `outbound_email`.
- `send_status = failed` means operator intervention or a later workflow is required; no durable v1 event is published from this state.

### `email_delivery_attempt`

One row per provider handoff attempt for an `outbound_email` row.

| Column | Type | Notes |
| --- | --- | --- |
| `id` | `uuid` | Primary key. Service-owned attempt identifier. |
| `outbound_email_id` | `uuid` | Foreign key to `outbound_email.id`. |
| `attempt_number` | `integer` | Monotonic count starting at `1` for the first provider submission attempt. |
| `provider_name` | `text` | Provider used for this attempt. |
| `provider_message_id` | `text null` | Provider-assigned message identifier when submission succeeds. |
| `attempt_status` | `text` | Contract values: `submitted`, `retryable_failure`, `failed`. |
| `failure_code` | `text null` | Provider or local classification for non-success results. |
| `failure_message` | `text null` | Operator-visible reason for failure or rejection. |
| `attempted_at` | `timestamptz` | Timestamp for the provider call completion. |
| `provider_response_snapshot` | `jsonb null` | Minimal structured provider response retained for operations. |

Semantic rules:

- `attempt_number` is unique per `outbound_email_id` and increases by one for each retry.
- Every provider submission or failure must create an `email_delivery_attempt` row before the outer status is considered updated.
- `attempt_status = retryable_failure` covers provider/network timeouts, connection errors, HTTP `429`, and provider `5xx` responses.
- `attempt_status = failed` covers invalid recipient, malformed payload, missing required render data, and permanent provider `4xx` responses.
- `attempt_status = failed` represents a terminal local classification for that attempt; the parent row should move to `failed` unless a later policy explicitly overrides that classification.
- `provider_response_snapshot` should remain minimal and operationally useful, not a dumping ground for full provider payloads.

## Relations

- `email_delivery_attempt.outbound_email_id -> outbound_email.id` (1:many)

## Cross-Service References

- `identity` remains the source of truth for `user_id`, registration state, verification tokens, and the verification target email carried by `VerificationEmailRequested`, but v1 email rendering depends on the event carrying concrete verification-link material rather than an email-time lookup.
- `workspace` remains the source of truth for `workspace_id`, `workspace_invitation_id`, inviter/invitee identity references, invitation expiry, and invitation acceptance lifecycle.
- Email stores cross-service identifiers carried in consumed-event payloads for traceability only; it does not create foreign keys into other service databases.
- Email must derive its `dedupe_key` from producer-owned identifiers so replay is idempotent across RabbitMQ redelivery; retries are driven locally from `next_attempt_after`, not from duplicate broker delivery.
