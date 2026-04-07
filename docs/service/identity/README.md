## Purpose

Identity owns user accounts, credentials, profile basics, email verification state, and session lifecycle. It is the source of truth for account and auth-related state called by external application servers through Envoy Gateway.

## Owned Responsibilities

- Create user accounts for new registrations.
- Store password credentials and validate password login attempts.
- Mint, verify, expire, and revoke service-owned sessions.
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

- **external application server through Envoy Gateway** for public registration, login, logout, and current-user routing.
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
- `VerifySession`
- `RevokeSession`
- `RedeemEmailVerificationToken`
- `UpdateUserProfile`
- `GetUserProfile`
- `GetUsersByIds`

See `grpc.md` for request and response contracts.

## Event Surface

- `UserRegistered`
- `UserProfileUpdated`
- `UserEmailVerified`
- `SessionRevoked`

See `events.md` for payload and publication rules.

### V1 Account Status Rules

- `RegisterUser` creates `user_account.account_status = active` only.
- `AuthenticatePassword` rejects `disabled` accounts and must not mint a new session for them.
- `VerifySession` treats sessions for `disabled` accounts as invalid even if the session row is otherwise unexpired.
- When an account is disabled, identity must revoke that account's active sessions and persist corresponding `SessionRevoked` outbox effects as part of the disable workflow.
