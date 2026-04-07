## Purpose

Bootstrap is the platform's hot-path query service for UI bootstrap reads. It is a hot-path query service backed by eventually consistent projections and is the canonical UI-facing aggregated read service for cross-domain, projection-backed queries.

## Owned Read Models

- `user_home_projection` for the signed-in home screen summary.
- `friend_projection` for accepted-friend list rows only in v1.
- `workspace_projection` for workspace cards and membership-scoped ordering.
- `workspace_channel_projection` for sidebar channel lists per workspace.
- `user_unread_counter` for fast unread badges and mention counts.

## Non-Goals

- Owning writes for users, friendships, workspaces, channels, or messages.
- Replacing service-owned domain APIs for command handling.
- Doing ad hoc runtime cross-service fanout reads for aggregates owned by bootstrap.
- Publishing domain-write events in v1.

## Dependencies

- **external SvelteKit server runtime through Envoy Gateway** for approved backend gRPC reads used to assemble UI responses.
- **RabbitMQ** for durable upstream event delivery.
- **identity** events for account bootstrap data.
- **friendship** events for accepted friendship projections.
- **workspace** events for workspace and membership projections.
- **chat** events for unread and last-message projection updates.

## Storage

- Bootstrap owns a service-local Postgres database containing only read models and projection bookkeeping.
- Projection tables are optimized for denormalized reads, not normalized write ownership.
- Redis is not required by default for v1.

## gRPC Surface

- `GetHome`
- `ListFriends`
- `ListWorkspaces`
- `GetWorkspaceSidebar`

See `grpc.md` for request and response contracts.

V1 read semantics:

- User-scoped collection reads return `200` with empty collections or zero-count defaults when a projection has not materialized yet.
- `GetHome` returns the latest available projected snapshot for the authenticated actor and does not use not-found semantics for projection lag.
- `GetWorkspaceSidebar` returns not found only when bootstrap has no evidence the actor can access the workspace; otherwise it returns the latest available projected snapshot, including an empty `channels` collection if channel rows lag.
- Collection ordering is stable and contractual: friends sort by `sort_username` ascending then `friend_user_id`, workspaces sort by `last_activity_at` descending then `workspace_id`, and sidebar channels sort by `position` ascending then `channel_id`.

## Event Dependencies

- Bootstrap consumes integration events from upstream service owners to maintain local read models.
- Bootstrap consumes explicit update events where v1 producers define them; today that includes identity profile updates plus chat message-edit and message-delete corrections for preview fields.
- Workspace and channel metadata update events are deferred beyond v1, so bootstrap treats those display fields as create-time stable for this document set.
- Read responses may briefly lag behind recent writes because projection updates converge asynchronously.
- Projection repair and rebuild must be supported from durable upstream event history or replay inputs.
