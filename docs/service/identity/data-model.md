## Persistence Scope

Identity owns account, credential, verification, profile-basic, and session state. Other services must read identity data through gRPC or local projections built from identity events.

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
- `account_status = disabled` prevents successful password authentication and causes `VerifySession` to treat all sessions for the account as invalid.
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
- Profile basics published in events must come from this table, not duplicated gateway state.

### `user_credential_password`

Password credential state for accounts using password auth.

| Column | Type | Notes |
| --- | --- | --- |
| `user_id` | `uuid` | Primary key and foreign key to `user_account.user_id`. |
| `password_hash` | `text` | PHC-formatted Argon2id hash string; plaintext is never stored. |
| `password_updated_at` | `timestamptz` | Time the active password hash was last rotated. |
| `failed_attempt_count` | `integer` | Optional local counter for operational controls. |
| `created_at` | `timestamptz` | Row creation time. |
| `updated_at` | `timestamptz` | Last credential metadata update time. |

Semantic rules:

- Exactly zero or one active password credential row exists per `user_id` in v1.
- `password_hash` stores only the derived hash string including algorithm parameters and salt.
- Password verification happens only inside identity; gateway never inspects hashes.

### `user_session`

Service-owned login session state.

| Column | Type | Notes |
| --- | --- | --- |
| `session_id` | `uuid` | Primary key. Stable session identifier returned to callers. |
| `user_id` | `uuid` | Foreign key to `user_account.user_id`. |
| `session_secret_hash` | `text` | Hash of the bearer session secret or opaque token material. |
| `issued_at` | `timestamptz` | Session issuance time. |
| `expires_at` | `timestamptz` | Hard expiry; sessions are invalid after this time. |
| `revoked_at` | `timestamptz null` | Null until explicitly revoked. |
| `revoke_reason` | `text null` | Optional contract value such as `logout` or `admin_action`. |
| `client_instance_id` | `uuid null` | Optional client instance binding when provided by the edge. |
| `created_at` | `timestamptz` | Row creation time. |

Semantic rules:

- A session is valid only when `revoked_at is null` and `expires_at > now()`.
- A session is also invalid when the owning `user_account.account_status` is `disabled`.
- `session_secret_hash` prevents storing reusable bearer material in plaintext.
- `VerifySession` must treat revoked and expired rows as invalid without consulting another service.
- If `client_instance_id` is set on the row, `VerifySession` must receive the same value or reject the session as invalid.
- If `client_instance_id` is null on the row, `VerifySession` does not require a request `client_instance_id` and does not enforce client-instance matching for that session.

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
- Successful redemption updates `user_account.email_verified_at` in the same transaction that records token consumption.
- `RegisterUser` creates the initial `email_verification_token` row in the same transaction as account creation.

## Relations

- `user_profile.user_id -> user_account.user_id` (1:1)
- `user_credential_password.user_id -> user_account.user_id` (1:1 in v1 password auth)
- `user_session.user_id -> user_account.user_id` (1:many)
- `email_verification_token.user_id -> user_account.user_id` (1:many over time)

## Cross-Service References

- Other services store `user_id` as the stable foreign reference only; they must not duplicate credential or session tables.
- `gateway` uses identity gRPC for registration, password authentication, session verification, and session revocation.
- `gateway` also calls identity to redeem email verification tokens and update profile basics for authenticated users.
- `bootstrap` and other consumers use identity events or approved lookup RPCs for profile basics.
- Durable identity events are inserted into the local `outbox_event` table within the same transaction as the source write.
