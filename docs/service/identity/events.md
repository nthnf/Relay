## Publication Model

Identity publishes integration events by inserting service-owned rows into `outbox_event` inside the same transaction as the source state change. The shared outbox worker later publishes those rows to RabbitMQ.

## Published Events

### `UserRegistered`

**When published**

- After a new account, profile, password credential, and initial session are committed successfully.

**Minimum payload**

- `user_id`
- `email`
- `email_verified`
- `username`
- `display_name`
- `avatar_url`
- `registered_at`
- `initial_session_id`
- `verification_token` as the opaque verification-link material needed by email delivery
- `verification_expires_at`
- `initial_email_verification_token_id`

**Typical consumers**

- `bootstrap` projections
- Other services that need durable user creation awareness

### `UserProfileUpdated`

**When published**

- After identity-owned profile basics change.
- Produced by successful `UpdateUserProfile` writes.

**Minimum payload**

- `user_id`
- `username`
- `display_name`
- `avatar_url`
- `updated_at`

**Typical consumers**

- `bootstrap` projections
- Services maintaining denormalized user display fields

### `UserEmailVerified`

**When published**

- After a verification token is redeemed and `user_account.email_verified_at` is set.
- Produced by successful `RedeemEmailVerificationToken` writes.

**Minimum payload**

- `user_id`
- `email`
- `email_verified_at`

**Typical consumers**

- `bootstrap` projections
- Services with workflow rules gated on verified email state

### `SessionRevoked`

**When published**

- After an active session is revoked explicitly before expiry.
- Also produced when identity disables an account and revokes that account's active sessions.

**Minimum payload**

- `session_id`
- `user_id`
- `revoked_at`
- `revoke_reason`

**Typical consumers**

- Services that cache or react to session invalidation state
- Audit or security monitoring pipelines

## Event Rules

- Event payloads use identity-owned keys and fields only.
- `user_id` is the stable cross-service user reference.
- `UserRegistered` must carry the initial opaque verification-link material so `email` can render and send the registration verification message without an identity-time lookup.
- Publication ordering should be preserved per aggregate where rows share the same account or session stream.
- Consumers must be idempotent because replay and duplicate delivery are expected platform behaviors.
- `RegisterUser`, `RedeemEmailVerificationToken`, `UpdateUserProfile`, and disable-driven session revocation all write their source rows and `outbox_event` rows in the same local transaction.
