## Purpose

Bootstrap is platform hot-path query service for personalized app-shell reads. It is canonical UI-facing aggregated read service for cross-domain, projection-backed queries and should return navigation-ready data without runtime fanout across owner services.

## Owned Read Models

- `user_app_projection` for signed-in app-shell summary and badge counts.
- `workspace_projection` for sidebar workspace rows.
- `workspace_channel_projection` for workspace channel lists, including denormalized `conversation_id`.
- `dm_projection` for DM thread lists, including denormalized `conversation_id`.
- `user_unread_counter` for fast workspace-channel unread badges and aggregation.

## Non-Goals

- Owning writes for users, friendships, workspaces, channels, conversations, read cursors, or messages.
- Replacing service-owned domain APIs for command handling.
- Doing ad hoc runtime cross-service fanout reads for aggregates owned by bootstrap.
- Publishing domain-write events in v1.
- Serving message history; `chat` remains durable history authority.

## Dependencies

- **external SvelteKit server runtime through Envoy Gateway** for approved backend gRPC reads used to assemble UI responses.
- **RabbitMQ** for durable upstream event delivery.
- **identity** events for account bootstrap data and profile freshness.
- **friendship** events for pending-request badge counts.
- **workspace** events for workspace and membership projections.
- **chat** events for conversation-id mapping, unread updates, DM-thread updates, and last-message preview updates.

## Storage

- Bootstrap owns service-local Postgres database containing only read models and projection bookkeeping.
- Projection tables are optimized for denormalized reads, not normalized write ownership.
- Redis is not required by default for v1.

## gRPC Surface

- `GetAppBootstrap`
- `GetWorkspaceBootstrap`
- `GetDmBootstrap`

See `grpc.md` for request and response contracts.

V1 read semantics:

- User-scoped collection reads return `200` with empty collections or zero-count defaults when projection has not materialized yet.
- `GetAppBootstrap` returns latest available projected shell snapshot for authenticated actor and does not use not-found semantics for projection lag.
- `GetWorkspaceBootstrap` returns not found only when bootstrap has no projected evidence that actor can access workspace; otherwise it returns latest available projected snapshot, including empty `channels` collection if channel rows lag.
- `GetDmBootstrap` returns latest available projected DM thread list for actor and does not wait for unrelated workspace projections.
- Collection ordering is stable and contractual: workspaces sort by `last_activity_at` descending then `workspace_id`, workspace channels sort by `position` ascending then `channel_id`, and DMs sort by `last_activity_at` descending then `conversation_id`.

## Event Dependencies

- Bootstrap consumes integration events from upstream service owners to maintain local read models.
- Bootstrap consumes explicit update events where producers define them; in v1 that includes identity profile updates, friendship pending-request lifecycle events for badge counts, chat conversation-create events, chat read-cursor updates, and chat message edit/delete corrections for preview fields.
- Workspace and channel metadata update events are deferred beyond v1, so bootstrap treats those display fields as create-time stable for this document set.
- Read responses may briefly lag behind recent writes because projection updates converge asynchronously.
- Projection repair and rebuild must be supported from durable upstream event history or replay inputs.
