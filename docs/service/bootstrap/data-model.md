## Persistence Scope

Bootstrap owns projection tables only. It does not own write-side domain records and does not read another service's database directly.

## Projection Tables

### `user_home_projection`

Home-screen aggregate keyed by user.

| Field | Notes |
| --- | --- |
| Primary key | `user_id` |
| Projection keys | `user_id` |
| Important indexes | `PRIMARY KEY (user_id)`, `INDEX (updated_at DESC)` for freshness audits |
| Maintained by events | `UserRegistered`, `UserProfileUpdated`, `FriendRequestAccepted`, `FriendshipRemoved`, `WorkspaceCreated`, `WorkspaceMemberAdded`, `WorkspaceMemberRemoved`, `MessageCreated` |

Suggested columns:

- `user_id`
- `username`
- `display_name`
- `avatar_url`
- `friend_count`
- `workspace_count`
- `unread_workspace_count`
- `total_unread_count`
- `updated_at`

Ordering notes:

- This row is a single aggregate snapshot, so ordering applies to embedded preview collections in API responses rather than table scans.

Refresh notes:

- Seed on `UserRegistered`.
- `UserProfileUpdated` refreshes mutable user display fields copied into the home payload.
- Increment or decrement counts from membership and friendship create/remove events.
- Recompute unread summary from `user_unread_counter` during replay or repair.

### `friend_projection`

User-scoped accepted-friend row materialized per friendship edge. V1 does not project pending or declined states.

| Field | Notes |
| --- | --- |
| Primary key | `(user_id, friend_user_id)` |
| Projection keys | `user_id`, `friend_user_id` |
| Important indexes | `PRIMARY KEY (user_id, friend_user_id)`, `INDEX (user_id, sort_username)`, `INDEX (user_id, status)` |
| Maintained by events | `UserProfileUpdated`, `FriendRequestAccepted`, `FriendshipRemoved` |

Suggested columns:

- `user_id`
- `friend_user_id`
- `username`
- `status`
- `sort_username`
- `display_name`
- `avatar_url`
- `accepted_at`
- `updated_at`

Refresh notes:

- `FriendRequestAccepted` writes two user-scoped rows, one per direction.
- `FriendshipRemoved` deletes both user-scoped rows, one per direction.
- `UserRegistered` does not create accepted-friend rows in v1 because accepted-friend projections begin only after durable friendship acceptance.
- `status` is always `accepted` in v1.
- `UserProfileUpdated` refreshes `username`, `sort_username`, `display_name`, and `avatar_url` without changing friendship ownership.
- API ordering is `sort_username` ascending, then `friend_user_id` as a stable tiebreaker.

### `workspace_projection`

Workspace card row scoped to a member.

| Field | Notes |
| --- | --- |
| Primary key | `(user_id, workspace_id)` |
| Projection keys | `user_id`, `workspace_id` |
| Important indexes | `PRIMARY KEY (user_id, workspace_id)`, `INDEX (user_id, last_activity_at DESC)`, `INDEX (workspace_id)` |
| Maintained by events | `WorkspaceCreated`, `WorkspaceMemberAdded`, `WorkspaceMemberRemoved`, `WorkspaceChannelCreated`, `MessageCreated`, `MessageEdited`, `MessageDeleted` |

Suggested columns:

- `user_id`
- `workspace_id`
- `workspace_name`
- `workspace_icon_url`
- `member_count`
- `channel_count`
- `last_activity_at`
- `last_message_preview`
- `unread_count`
- `updated_at`

Refresh notes:

- `WorkspaceCreated` seeds the creator-visible row.
- `WorkspaceMemberAdded` fans out a member-visible row for the new member.
- `WorkspaceMemberRemoved` deletes the member-visible row and any dependent unread/channel rows for that member-workspace scope.
- `MessageCreated` updates preview and activity ordering for affected members using local membership-derived rows already present in `workspace_projection` plus local `user_unread_counter` updates; it does not synchronously read workspace or chat services.
- `MessageEdited` refreshes `last_message_preview` only when the edited message is still the row's latest visible message.
- `MessageDeleted` clears or tombstones preview state only when the deleted message is still the row's latest visible message.
- API ordering is `last_activity_at` descending, then `workspace_id` as a stable tiebreaker.

### `workspace_channel_projection`

Member-scoped sidebar channel rows.

| Field | Notes |
| --- | --- |
| Primary key | `(user_id, workspace_id, channel_id)` |
| Projection keys | `user_id`, `workspace_id`, `channel_id` |
| Important indexes | `PRIMARY KEY (user_id, workspace_id, channel_id)`, `INDEX (user_id, workspace_id, position)`, `INDEX (workspace_id, channel_id)` |
| Maintained by events | `WorkspaceCreated`, `WorkspaceMemberAdded`, `WorkspaceMemberRemoved`, `WorkspaceChannelCreated`, `MessageCreated`, `MessageEdited`, `MessageDeleted` |

Suggested columns:

- `user_id`
- `workspace_id`
- `channel_id`
- `channel_name`
- `channel_kind`
- `position`
- `last_message_id`
- `last_message_preview`
- `last_activity_at`
- `unread_count`
- `mention_count`
- `updated_at`

Refresh notes:

- New members may need backfill rows for all existing channels in the workspace.
- `WorkspaceMemberRemoved` deletes all channel rows for the removed user's workspace scope.
- Replay should rebuild ordering from canonical channel metadata events, then apply message-derived activity fields.
- `MessageCreated` updates `last_message_id`, preview, and activity fields using the local `(user_id, workspace_id, channel_id)` projection rows and local unread counters; it does not do runtime cross-service fanout reads.
- `MessageEdited` refreshes `last_message_preview` only when the edited message matches `last_message_id`.
- `MessageDeleted` clears or tombstones preview state only when the deleted message matches `last_message_id`.
- API ordering is `position` ascending, then `channel_id` as a stable tiebreaker.

### `user_unread_counter`

Per-user per-channel unread counter used by home and sidebar responses.

| Field | Notes |
| --- | --- |
| Primary key | `(user_id, workspace_id, channel_id)` |
| Projection keys | `user_id`, `workspace_id`, `channel_id` |
| Important indexes | `PRIMARY KEY (user_id, workspace_id, channel_id)`, `INDEX (user_id, workspace_id)`, `INDEX (user_id, unread_count DESC)` |
| Maintained by events | `WorkspaceMemberAdded`, `WorkspaceMemberRemoved`, `WorkspaceChannelCreated`, `MessageCreated` |

Suggested columns:

- `user_id`
- `workspace_id`
- `channel_id`
- `last_read_message_id`
- `unread_count`
- `mention_count`
- `updated_at`

Refresh notes:

- `MessageCreated` increments unread counters for relevant members except the author.
- `WorkspaceMemberRemoved` deletes unread rows for the removed member's workspace scope.
- Membership scope comes from local member-scoped workspace/channel projection rows created by `WorkspaceMemberAdded`, so unread fanout is computed from local bootstrap-owned projections rather than synchronous cross-service reads.
- Read-receipt-driven decrement/reset events are expected in a later revision.
- Until read events exist, docs should treat unread counters as append-only projection behavior for v1 planning.

## Relations

- `user_home_projection.user_id` aggregates from `friend_projection`, `workspace_projection`, and `user_unread_counter`.
- `workspace_projection (user_id, workspace_id)` is the parent aggregate for `workspace_channel_projection (user_id, workspace_id, channel_id)`.
- `user_unread_counter` feeds unread fields denormalized into both `workspace_projection` and `workspace_channel_projection`.
- `MessageCreated` applies to member-scoped rows already materialized in `workspace_projection` and `workspace_channel_projection`, allowing bootstrap to update previews and unread counts without runtime cross-service fanout.

## Projection Refresh Notes

- Consumers must be idempotent because replay and duplicate delivery are expected.
- Projection handlers should prefer row upsert patterns keyed by stable projection keys.
- Mutable display-field freshness depends on explicit upstream update events where defined. In v1, `UserProfileUpdated`, `MessageEdited`, and `MessageDeleted` are defined; workspace and channel metadata update events are deferred.
- Full rebuild should truncate and replay projection tables from durable event inputs, then validate counts against source-of-truth services.
