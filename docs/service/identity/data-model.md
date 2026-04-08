## Persistence Scope

Identity owns account, credential, verification, profile-basic, and refresh-session state. Other services must read identity data through gRPC or local projections built from identity events.

## Core Tables

### `user_account`

One row per platform account.

| Column | Type | Notes |
| --- | --- | --- |
| `user_id` | `uuid` | Primary key. Stable cross-service user identifier. |
| `email` | `text` | Original email as accepted for display and audit. |
| `email_normalized` | `text` | Canonical lowercase email used for uniqueness checks. Unique across all v1 accounts. |
| `email_verified_at` | `timestamptz null` | Null until email ownership is verified. |
| `account_status` | `text` | Contract values: `active`, `disabled`. |
| `created_at` | `timestamptz` | Row creation time. |
| `updated_at` | `timestamptz` | Last account metadata update time. |

Semantic rules:

- `user_id` is the canonical identity key referenced by other services and events.
- `email_normalized` must be unique so registration rejects duplicate emails independent of case.
- `email_verified_at` changes only inside identity-owned verification flows.
- `account_status = active` is the only value created by `RegisterUser` in v1.
- `RegisterUser` does not create a `user_session`; the first session is minted only after verification redemption.
- `account_status = disabled` prevents successful password authentication and causes `RefreshSession` to reject refresh attempts for the account's sessions.
- Disabling an account requires revoking all active `user_session` rows for that `user_id` and inserting matching `SessionRevoked` outbox rows.

### `user_profile`

Mutable profile basics owned by identity.

| Column | Type | Notes |
| --- | --- | --- |
| `user_id` | `uuid` | Primary key and foreign key to `user_account.user_id`. |
| `username` | `text` | Platform-unique username. |
| `display_name` | `text` | User-facing display label. |
| `avatar_url` | `text null` | Optional avatar asset URL. |
| `created_at` | `timestamptz` | Row creation time. |
| `updated_at` | `timestamptz` | Last profile update time. |

Semantic rules:

- `user_profile.user_id` is a 1:1 relation with `user_account.user_id`.
- `username` must be unique because downstream services and projections may use it as a stable display field.
- Profile basics published in events must come from this table, not duplicated edge-session state.

### `user_credential_password`

Password credential state for accounts using password auth.

| Column | Type | Notes |
| --- | --- | --- |
| `user_id` | `uuid` | Primary key and foreign key to `user_account.user_id`. |
| `password_hash` | `text` | PHC-formatted Argon2id hash string derived and stored by identity; plaintext is never stored. |
| `password_updated_at` | `timestamptz` | Time the active password hash was last rotated. |
| `failed_attempt_count` | `integer` | Optional local counter for operational controls. |
| `created_at` | `timestamptz` | Row creation time. |
| `updated_at` | `timestamptz` | Last credential metadata update time. |

Semantic rules:

- Exactly zero or one active password credential row exists per `user_id` in v1.
- `password_hash` stores only the server-derived Argon2id hash string including algorithm parameters and salt.
- Identity derives this field from the submitted plaintext password inside `RegisterUser` and never stores plaintext at rest.
- Password verification happens only inside identity against the stored Argon2id hash; the ingress layer never inspects hashes.

### `user_session`

Service-owned refresh-session state.

| Column | Type | Notes |
| --- | --- | --- |
| `session_id` | `uuid` | Primary key. Stable session identifier returned to callers. |
| `user_id` | `uuid` | Foreign key to `user_account.user_id`. |
| `refresh_token_hash` | `text` | Hash of the rotating refresh token material. |
| `issued_at` | `timestamptz` | Session issuance time. |
| `refresh_expires_at` | `timestamptz` | Hard expiry for the refresh token session. |
| `revoked_at` | `timestamptz null` | Null until explicitly revoked. |
| `revoke_reason` | `text null` | Optional contract value such as `logout` or `admin_action`. |
| `replaced_by_session_id` | `uuid null` | Set when refresh rotates this session into a newly issued session. |
| `client_instance_id` | `uuid null` | Optional client instance binding when provided by the edge. |
| `created_at` | `timestamptz` | Row creation time. |

Semantic rules:

- A refresh session is valid only when `revoked_at is null` and `refresh_expires_at > now()`.
- A refresh session is also invalid when the owning `user_account.account_status` is `disabled`.
- `refresh_token_hash` prevents storing reusable refresh-token material in plaintext.
- Successful refresh rotates the current session by setting `revoked_at`, `revoke_reason = rotated`, and `replaced_by_session_id`, then creating a new `user_session` row in the same transaction.
- If `client_instance_id` is set on the row, `RefreshSession` must receive the same value or reject the refresh as invalid.
- If `client_instance_id` is null on the row, `RefreshSession` does not require a request `client_instance_id` and does not enforce client-instance matching for that session.
- Short-lived access tokens are minted by identity but validated at Envoy Gateway on protected routes rather than looked up in this table on every request.

### `email_verification_token`

Single-purpose token state for proving email ownership.

| Column | Type | Notes |
| --- | --- | --- |
| `token_id` | `uuid` | Primary key. |
| `user_id` | `uuid` | Foreign key to `user_account.user_id`. |
| `token_hash` | `text` | Hash of the verification token delivered to the user. |
| `expires_at` | `timestamptz` | Hard expiry time. |
| `consumed_at` | `timestamptz null` | Null until the token is redeemed successfully. |
| `created_at` | `timestamptz` | Row creation time. |

Semantic rules:

- Tokens are single-use and invalid after `consumed_at` is set or `expires_at` passes.
- Token material is stored only as a hash.
- Resend-driven replacement should invalidate outstanding active verification tokens before issuing a fresh one.
- Successful redemption updates `user_account.email_verified_at` in the same transaction that records token consumption.
- `RegisterUser` creates the initial `email_verification_token` row in the same transaction as account creation.

## Relations

- `user_profile.user_id -> user_account.user_id` (1:1)
- `user_credential_password.user_id -> user_account.user_id` (1:1 in v1 password auth)
- `user_session.user_id -> user_account.user_id` (1:many)
- `email_verification_token.user_id -> user_account.user_id` (1:many over time)

## Cross-Service References

- Other services store `user_id` as the stable foreign reference only; they must not duplicate credential or session tables.
- External application servers call identity through Envoy Gateway for registration, password authentication, refresh-token rotation, and session revocation.
- External application servers also call identity through Envoy Gateway to redeem email verification tokens and update profile basics for authenticated users.
- `bootstrap` and other consumers use identity events or approved lookup RPCs for profile basics.
- Durable identity events are inserted into the local `outbox_event` table within the same transaction as the source write.
