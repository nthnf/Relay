## Consumption Model

Email consumes durable integration events from RabbitMQ and records local send state in its own Postgres database. In v1 it does not publish durable delivery-result events back onto the bus.

## Consumed Events

### `VerificationEmailRequested`

**When consumed**

- After identity requests verification mail for registration, auth-after-unverified, or resend flows.
- Email uses this event to enqueue verification email sends.

**Minimum payload needed by email**

- `user_id`
- `email`
- `verification_token`
- `verification_token_id`
- `verification_token_expires_at`
- `reason`
- `requested_at`

**Local effect**

- Create or idempotently reuse one `outbound_email` row with `email_kind = registration_verification`.
- Compose the public verification URL from configured public-web base URL plus the opaque `verification_token` carried by the event.
- Render the verification email directly from the consumed payload without an identity lookup.
- Attempt provider submission and record one `email_delivery_attempt` row per send try.

## Event Rules

- Email is consume-only in v1 for durable events; no `EmailDelivered`, `EmailBounced`, or similar integration events are published yet.
- The v1 event scope is limited to registration verification.
- Password-reset triggers are explicitly out of scope for v1.
- `VerificationEmailRequested` must carry `verification_token`, `verification_token_id`, and `verification_token_expires_at` so email can build the verification link without hidden lookup behavior.
- `reason` values are `registration`, `authenticate_unverified`, and `resend_request`.
- Consumers must be idempotent because RabbitMQ replay and duplicate delivery are expected platform behaviors.
- Duplicate delivery of the same consumed event is a no-op for outbound intent creation and immediate send behavior when a matching `outbound_email` row already exists.
- Duplicate broker delivery must not trigger a resend if the existing `outbound_email` row is already queued, submitted, retryable, or terminal.
- Resend attempts after the first submission are driven only by email's local retry policy over the existing `outbound_email` row.
- Minimal v1 retry classification is: provider/network timeouts, connection errors, `429`, and provider `5xx` responses are retryable; invalid recipient, malformed payload, missing required render data, and permanent provider `4xx` responses are terminal.
- For retryable failures, email must set `next_attempt_after` and a local due-at scheduler or poller over `outbound_email` drives later attempts.
- A provider submission failure updates local email state and attempt history only; upstream services do not rely on a v1 delivery-result event.
- If a consumed event is redelivered after a successful local enqueue, email must reuse the same `outbound_email` row rather than enqueueing another send.
