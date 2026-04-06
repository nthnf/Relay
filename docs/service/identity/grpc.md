## gRPC Service Scope

Identity exposes synchronous account, session, and profile-basic contracts. Gateway is the primary caller for auth flows; bootstrap and selected internal services may use profile lookup methods when a direct owner lookup is justified. These calls are authoritative request/response flows, not chat-like low-latency fanout.

## Shared Contract Rules

- `RegisterUser` creates `user_account.account_status = active` only in v1.
- `AuthenticatePassword` must reject accounts where `account_status = disabled`.
- `VerifySession` must return `valid = false` for revoked sessions, expired sessions, or any session whose owning account is `disabled`.
- Disabling an account is an identity-owned workflow that revokes all active sessions for the user and emits the corresponding `SessionRevoked` outbox effects.
- Session binding rule: if a session is created with `client_instance_id`, later `VerifySession` calls must supply the same `client_instance_id` value. Missing or mismatched values make the session invalid.
- If a session is created without `client_instance_id`, `VerifySession` may omit it and the session remains eligible for validation.

### `RegisterUser`

**Main caller:** `gateway`

**Request fields**

- `email` (`string`)
- `password` (`string`)
- `username` (`string`)
- `display_name` (`string`)
- `avatar_url` (`string optional`)
- `idempotency_key` (`string optional`) - forwarded for tracing only; gateway remains the idempotency owner.

**Response fields**

- `user_id` (`uuid`)
- `session_id` (`uuid`)
- `session_token` (`string`) - bearer material for the newly created session.
- `expires_at` (`timestamp`)
- `email_verified` (`bool`)
- `profile` (`message`) with `user_id`, `username`, `display_name`, `avatar_url`

**Contract notes**

- Reject duplicate `email_normalized` with an already-exists domain error.
- Create `user_account`, `user_profile`, `user_credential_password`, initial `user_session`, initial `email_verification_token`, and `outbox_event` rows in one local transaction.
- The created account must start as `active` in v1.

### `AuthenticatePassword`

**Main caller:** `gateway`

**Request fields**

- `email` (`string`)
- `password` (`string`)
- `client_instance_id` (`uuid optional`)

**Response fields**

- `user_id` (`uuid`)
- `session_id` (`uuid`)
- `session_token` (`string`)
- `expires_at` (`timestamp`)
- `email_verified` (`bool`)
- `profile` (`message`) with `user_id`, `username`, `display_name`, `avatar_url`

**Contract notes**

- Authenticate against `user_account.email_normalized` plus `user_credential_password.password_hash`.
- On success, create a new `user_session` owned by identity.
- Reject accounts where `account_status = disabled`.
- If `client_instance_id` is provided at session creation time, the issued session is bound to that value for later verification.

### `VerifySession`

**Main caller:** `gateway`

**Request fields**

- `session_token` (`string`)
- `client_instance_id` (`uuid optional`)

**Response fields**

- `valid` (`bool`)
- `user_id` (`uuid`)
- `session_id` (`uuid`)
- `expires_at` (`timestamp`)
- `email_verified` (`bool`)
- `profile` (`message`) with `user_id`, `username`, `display_name`, `avatar_url`

**Contract notes**

- Identity resolves the token to `user_session`, verifies `session_secret_hash`, and rejects expired, revoked, or disabled-account sessions.
- `gateway` uses the returned actor context; session ownership remains in identity.
- If the stored session row has a non-null `client_instance_id`, a missing request value or mismatched request value returns `valid = false`.

### `RevokeSession`

**Main caller:** `gateway`

**Request fields**

- `session_id` (`uuid`)
- `revoke_reason` (`string optional`)

**Response fields**

- `revoked` (`bool`)
- `revoked_at` (`timestamp optional`)

**Contract notes**

- Revocation is idempotent: an already-revoked session returns `revoked = true` with the recorded timestamp.
- Successful state change inserts a matching `SessionRevoked` outbox row in the same transaction.

### `RedeemEmailVerificationToken`

**Main caller:** `gateway`

**Request fields**

- `token` (`string`)

**Response fields**

- `user_id` (`uuid`)
- `email` (`string`)
- `email_verified` (`bool`)
- `verified_at` (`timestamp`)

**Contract notes**

- Identity hashes the presented token, resolves `email_verification_token`, and rejects unknown, expired, or already-consumed tokens.
- Successful redemption marks the token consumed, sets `user_account.email_verified_at`, and inserts a `UserEmailVerified` outbox row in one local transaction.

### `UpdateUserProfile`

**Main caller:** `gateway`

**Request fields**

- `user_id` (`uuid`)
- `display_name` (`string`)
- `avatar_url` (`string optional`)

**Response fields**

- `user_id` (`uuid`)
- `username` (`string`)
- `display_name` (`string`)
- `avatar_url` (`string optional`)
- `updated_at` (`timestamp`)

**Contract notes**

- V1 scope is profile basics only; this method does not update credentials, session state, or cross-domain data.
- Successful updates insert a `UserProfileUpdated` outbox row in the same transaction as the profile write.

### `GetUserProfile`

**Main caller:** `gateway`

**Request fields**

- `user_id` (`uuid`)

**Response fields**

- `user_id` (`uuid`)
- `email` (`string`)
- `email_verified` (`bool`)
- `username` (`string`)
- `display_name` (`string`)
- `avatar_url` (`string optional`)

**Contract notes**

- Returns identity-owned profile basics for current-user and other owner-approved lookup paths.
- This is not a cross-domain aggregate query.

### `GetUsersByIds`

**Main caller:** `bootstrap` or another internal service

**Request fields**

- `user_ids` (`repeated uuid`)

**Response fields**

- `users` (`repeated message`) with `user_id`, `username`, `display_name`, `avatar_url`, `email_verified`

**Contract notes**

- Supports bounded owner lookups for stable identity basics.
- Callers must not use this method to replace projection-backed reads owned by `bootstrap`.
