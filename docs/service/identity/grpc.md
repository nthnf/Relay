## gRPC Service Scope

Identity exposes synchronous account, refresh-session, and profile-basic contracts. External application servers through Envoy Gateway are the primary callers for auth flows; `bootstrap` and selected internal services may use profile lookup methods when a direct owner lookup is justified. These calls are authoritative request/response flows, not chat-like low-latency fanout.

## Shared Contract Rules

- Authenticated end-user RPCs derive actor/session context from Envoy-validated access-JWT claims and forwarded request context.
- Envoy may call identity `Authorization/Check` before forwarding protected requests or websocket upgrades, then attach validated actor context out-of-band.
- Unauthenticated auth-entry RPCs such as `RegisterUser` and `AuthenticatePassword` do not require an already-authenticated actor.
- `RefreshSession` is also a public auth-entry RPC and validates the presented refresh token inside identity rather than relying on an already-authenticated access JWT.
- Internal bounded lookup RPCs such as `GetUsersByIds` are internal service calls and are not ingress-authenticated end-user actions.
- Identity still enforces its own service-boundary rules and authorization semantics where applicable.
- `RegisterUser` creates `user_account.account_status = active` only in v1.
- `AuthenticatePassword` must reject accounts where `account_status = disabled`.
- Protected backend gRPC routes rely on Envoy Gateway to validate short-lived access JWTs before forwarding authenticated request context downstream.
- `RefreshSession` must reject revoked, expired, rotated, or disabled-account refresh sessions.
- Disabling an account is an identity-owned workflow that revokes all active sessions for the user and emits the corresponding `SessionRevoked` outbox effects.
- Session binding rule: if a refresh session is created with `client_instance_id`, later `RefreshSession` calls must supply the same `client_instance_id` value. Missing or mismatched values make the refresh invalid.
- If a refresh session is created without `client_instance_id`, `RefreshSession` may omit it and the session remains eligible for refresh.

### `RegisterUser`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `email` (`string`)
- `password` (`string`) - plaintext password input; identity hashes it server-side with Argon2id before persistence.
- `username` (`string`)
- `display_name` (`string`)
- `avatar_url` (`string optional`)

**Response fields**

- `user_id` (`uuid`)
- `email` (`string`)
- `verification_email_requested_at` (`timestamp`)

**Contract notes**

- Reject duplicate `email_normalized` with an already-exists domain error.
- Hash the submitted plaintext password server-side with Argon2id before writing `user_credential_password.password_hash`.
- Create `user_account`, `user_profile`, `user_credential_password`, initial `email_verification_token`, and the `UserRegistered` plus `VerificationEmailRequested` outbox rows in one local transaction.
- Do not create a `user_session` during registration.
- The created account must start as `active` in v1.

### `AuthenticatePassword`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `email` (`string`)
- `password` (`string`) - plaintext password input submitted for server-side verification against the stored Argon2id hash.
- `client_instance_id` (`uuid optional`)

**Response fields**

- `user_id` (`uuid`)
- `session_id` (`uuid`)
- `access_token` (`string`)
- `access_token_expires_at` (`timestamp`)
- `refresh_token` (`string`)
- `refresh_token_expires_at` (`timestamp`)
- `email_verified` (`bool`)
- `profile` (`message`) with `user_id`, `username`, `display_name`, `avatar_url`

**Contract notes**

- Authenticate against `user_account.email_normalized` plus `user_credential_password.password_hash` using server-side Argon2id verification.
- On success, create a new rotating `user_session` owned by identity.
- Reject accounts where `account_status = disabled`.
- If `client_instance_id` is provided at session creation time, the issued refresh session is bound to that value for later refresh.
- If the account is not yet verified, `AuthenticatePassword` does not mint a session.

### `ResendVerificationEmail`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `email` (`string`)

**Response fields**

- `accepted` (`bool`)

**Contract notes**

- Always return a success-shaped response to avoid account enumeration.
- Only existing unverified accounts may trigger a new verification email request.
- Resend throttling remains internal and is not reflected in the response.

### `RefreshSession`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `refresh_token` (`string`)
- `client_instance_id` (`uuid optional`, reserved for a future session-binding hardening step and ignored in v1)

**Response fields**

- `user_id` (`uuid`)
- `session_id` (`uuid`)
- `access_token` (`string`)
- `access_token_expires_at` (`timestamp`)
- `refresh_token` (`string`)
- `refresh_token_expires_at` (`timestamp`)
- `email_verified` (`bool`)
- `profile` (`message`) with `user_id`, `username`, `display_name`, `avatar_url`

**Contract notes**

- Identity resolves the refresh token to `user_session`, verifies `refresh_token_hash`, and rejects expired, revoked, rotated, or disabled-account sessions.
- Successful refresh revokes the old refresh session, creates a new refresh session row, and returns a new access token plus refresh token pair.
- The old refresh token must no longer be usable after a successful refresh.
- `client_instance_id` is carried in the request contract for forward compatibility but is ignored in v1.

### `RevokeSession`

**Main caller:** external application server through Envoy Gateway

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

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `token` (`string`)

**Response fields**

- `user_id` (`uuid`)
- `session_id` (`uuid`)
- `access_token` (`string`)
- `access_token_expires_at` (`timestamp`)
- `refresh_token` (`string`)
- `refresh_token_expires_at` (`timestamp`)
- `email_verified` (`bool`)
- `profile` (`message`) with `user_id`, `username`, `display_name`, `avatar_url`

**Contract notes**

- Identity hashes the presented token, resolves `email_verification_token`, and rejects unknown, expired, or already-consumed tokens.
- Successful redemption marks the token consumed, sets `user_account.email_verified_at`, creates the first session, and returns a `TokenPairResponse` in one local transaction.

### `UpdateUserProfile`

**Main caller:** external application server through Envoy Gateway

**Request fields**

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
- The target user is the authenticated actor derived from Envoy-forwarded request context; callers do not supply a mutable actor ID in-band.
- Successful updates insert a `UserProfileUpdated` outbox row in the same transaction as the profile write.

### `GetUserProfile`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `user_id` (`uuid`)

**Response fields**

- `user_id` (`uuid`)
- `username` (`string`)
- `display_name` (`string`)
- `avatar_url` (`string optional`)

**Contract notes**

- Returns identity-owned profile basics for the authenticated actor when `user_id` is omitted, or for other owner-approved lookup paths when `user_id` is provided.
- This method intentionally excludes account-contact and verification-state fields; callers that need those should use a future self-account contract instead of widening profile basics.
- This is not a cross-domain aggregate query.

### `GetUsersByIds`

**Main caller:** `bootstrap` or another internal service

**Request fields**

- `user_ids` (`repeated uuid`)

**Response fields**

- `users` (`repeated message`) with `user_id`, `username`, `display_name`, `avatar_url`

**Contract notes**

- Supports bounded owner lookups for stable identity basics.
- Callers must not use this method to replace projection-backed reads owned by `bootstrap`.
