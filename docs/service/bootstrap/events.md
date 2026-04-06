## Published Events

- None required for v1.

Bootstrap should not publish domain-write events in v1. It is a read-only aggregate service for projection-backed queries, not a write owner.

## Consumed Events

- `UserRegistered` to seed user-visible home projection data.
- `UserProfileUpdated` to refresh usernames, display names, avatar URLs, and friend ordering keys.
- `FriendRequestAccepted` to materialize accepted friend rows and home counters.
- `FriendshipRemoved` to remove accepted friend rows and decrement home counters.
- `WorkspaceCreated` to seed workspace visibility and initial home aggregates.
- `WorkspaceMemberAdded` to fan out member-scoped workspace and channel projections.
- `WorkspaceMemberRemoved` to remove member-scoped workspace, channel, and unread rows.
- `WorkspaceChannelCreated` to create sidebar channel rows and workspace channel counts.
- `MessageCreated` to update unread counters, activity ordering, and preview fields.
- `MessageEdited` to refresh projected last-message preview text when the edited message is still the latest visible message for the affected row.
- `MessageDeleted` to clear or tombstone projected last-message preview state when the deleted message is still the latest visible message for the affected row.

## Projection Notes

- Bootstrap consumes events from RabbitMQ and updates only its local Postgres projections.
- Handlers must be idempotent because replay and duplicate delivery are expected.
- V1 friend projections cover accepted friends only; pending or declined request state is not projected by bootstrap, and `UserRegistered` alone does not create accepted-friend rows.
- `FriendshipRemoved` deletes the pair's accepted-friend projection rows if present.
- `WorkspaceMemberRemoved` deletes member-scoped workspace, channel, and unread rows for the removed user if present.
- `MessageCreated` uses the message payload plus bootstrap's local member-scoped workspace/channel projections created from `WorkspaceMemberAdded` to determine which rows receive unread and preview updates.
- `MessageEdited` and `MessageDeleted` only affect bootstrap rows when the changed message is still the row's current preview anchor.
- Bootstrap does not perform synchronous cross-service reads to compute unread counters, preview text, or activity ordering.
- Workspace and channel mutable metadata update events are not defined in v1 yet, so bootstrap treats workspace and channel display fields as create-time stable for now.
- If a low-latency synchronous path exists elsewhere, bootstrap still relies on the cold path for durable convergence and repair.
- Later versions may publish infrastructure-scoped rebuild or snapshot signals, but not domain-write ownership events.
