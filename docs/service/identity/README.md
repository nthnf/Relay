## Purpose

Identity owns user accounts, credentials, profile basics, email verification state, and session lifecycle. It is the source of truth for account and auth-related state called by external application servers through Envoy Gateway.

## Owned Responsibilities

- Create user accounts for new registrations.
- Store password credentials and hash them server-side with Argon2id before persistence.
- Request verification email delivery separately from durable account creation.
- Mint short-lived access JWTs plus rotating refresh tokens.
- Rotate refresh tokens and revoke the prior refresh token on successful refresh.
- Revoke refresh sessions explicitly on logout, disable, or other identity-owned security actions.
- Enforce account status so disabled accounts cannot authenticate or continue using existing sessions.
- Own profile basics needed across the platform: username, display name, avatar URL, and email verification status.
- Issue and redeem email verification tokens.
- Publish durable identity integration events through the local `outbox_event` table.

## Non-Goals

- Acting as the public HTTP edge; external application servers reach identity through Envoy Gateway.
- Envoy Gateway handles backend ingress policy; identity retains service-owned authorization and account/session ownership at its own boundary.
- Owning cross-domain aggregates, friend graphs, workspaces, channels, or chat state.
- Providing a shared cross-service database for user lookups.
- Replacing `bootstrap` as the canonical UI-facing aggregate read service.

## Dependencies

- **external application server through Envoy Gateway** for public registration, login, refresh, logout, and current-user/profile routing.
- **RabbitMQ** for durable cold-path publication of identity integration events.
- **outbox worker sidecar** for polling local `outbox_event` rows and publishing them.
- **Postgres** as the service-owned source of truth for accounts, credentials, sessions, and verification tokens.

## Storage

- Identity owns a dedicated Postgres database.
- Durable domain writes and matching `outbox_event` inserts occur in the same local transaction.
- Redis is not required by default for v1 identity behavior.

## gRPC Surface

- `RegisterUser`
- `AuthenticatePassword`
- `RefreshSession`
- `RevokeSession`
- `RedeemEmailVerificationToken`
- `ResendVerificationEmail`
- `UpdateUserProfile`
- `GetUserProfile`
- `GetUsersByIds`

See `grpc.md` for request and response contracts.

## Event Surface

- `UserRegistered`
- `VerificationEmailRequested`
- `UserProfileUpdated`
- `UserEmailVerified`
- `SessionRevoked`

See `events.md` for payload and publication rules.

### V1 Account Status Rules

- `RegisterUser` creates `user_account.account_status = active` only.
- `AuthenticatePassword` rejects `disabled` accounts and must not mint new token pairs for them.
- `RefreshSession` rejects revoked, expired, rotated, or disabled-account refresh sessions.
- Envoy Gateway validates short-lived access JWTs on protected routes; identity remains the authority for refresh-token validation and rotation.
- When an account is disabled, identity must revoke that account's active sessions and persist corresponding `SessionRevoked` outbox effects as part of the disable workflow.
