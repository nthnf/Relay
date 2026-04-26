## Published Events

- None required for v1.

Bootstrap should not publish domain-write events in v1. It is read-only aggregate service for projection-backed queries, not write owner.

## Consumed Events

- `UserRegistered` to seed user-visible app projection data.
- `UserProfileUpdated` to refresh viewer rows and DM peer display fields.
- `FriendRequestCreated` to increment incoming pending-request badge count for addressee.
- `FriendRequestAccepted` to decrement pending-request badge count when pending request resolves by acceptance.
- `FriendRequestRejected` to decrement pending-request badge count when pending request resolves by rejection.
- `FriendRequestCanceledByBlock` to decrement pending-request badge count when pending request resolves by block.
- `WorkspaceCreated` to seed creator-visible workspace rows and app aggregates.
- `WorkspaceMemberAdded` to fan out member-scoped workspace and channel projections.
- `WorkspaceMemberRemoved` to remove member-scoped workspace, channel, and unread rows.
- `WorkspaceChannelCreated` to create workspace channel rows.
- `ConversationCreated` to denormalize `conversation_id` into workspace-channel and DM projections.
- `DmPairCreated` to seed normalized DM pair ownership needed for DM-thread projections.
- `MessageCreated` to update unread counters, activity ordering, and preview fields.
- `MessageEdited` to refresh projected last-message preview text when edited message is still latest visible message for affected row.
- `MessageDeleted` to clear or tombstone projected last-message preview state when deleted message is still latest visible message for affected row.
- `ConversationReadCursorUpdated` to recompute unread counters from authoritative chat-owned read cursor.

## Projection Notes

- Bootstrap consumes events from RabbitMQ and updates only local Postgres projections.
- Handlers must be idempotent because replay and duplicate delivery are expected.
- Bootstrap keeps only pending-request count in v1, not full pending-request row projection.
- `WorkspaceMemberRemoved` deletes member-scoped workspace, channel, and DM rows for removed user when applicable.
- `DmPairCreated` seeds or repairs two participant-scoped `dm_projection` rows, one for each user in the pair.
- `ConversationCreated` denormalizes `conversation_id` into existing workspace-channel rows and existing DM rows; bootstrap does not expose separate lookup RPC in v1.
- `MessageCreated` uses message payload plus local member-scoped workspace/channel and DM projections to determine which rows receive unread and preview updates.
- `ConversationReadCursorUpdated` is authoritative reset/input for unread projection repair; bootstrap does not own read cursor writes.
- `MessageEdited` and `MessageDeleted` only affect bootstrap rows when changed message is still row's current preview anchor.
- Bootstrap does not perform synchronous cross-service reads to compute badge counts, preview text, or activity ordering.
- Workspace and channel mutable metadata update events are not defined in v1 yet, so bootstrap treats those display fields as create-time stable for now.
- If low-latency synchronous path exists elsewhere, bootstrap still relies on cold path for durable convergence and repair.
