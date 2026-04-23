## Persistence Scope

Bootstrap owns projection tables only. It does not own write-side domain records and does not read another service's database directly.

## Projection Tables

### `user_app_projection`

App-shell aggregate keyed by user.

| Field | Notes |
| --- | --- |
| Primary key | `user_id` |
| Projection keys | `user_id` |
| Important indexes | `PRIMARY KEY (user_id)`, `INDEX (updated_at DESC)` |
| Maintained by events | `UserRegistered`, `UserProfileUpdated`, `FriendRequestCreated`, `FriendRequestAccepted`, `FriendRequestRejected`, `FriendRequestCanceledByBlock`, `WorkspaceCreated`, `WorkspaceMemberAdded`, `WorkspaceMemberRemoved`, `MessageCreated`, `ConversationReadCursorUpdated` |

Suggested columns:

- `user_id`
- `username`
- `display_name`
- `avatar_url`
- `workspace_count`
- `unread_workspace_count`
- `total_unread_count`
- `pending_friend_request_count`
- `updated_at`

Refresh notes:

- Seed on `UserRegistered`.
- `UserProfileUpdated` refreshes mutable user display fields copied into app payload.
- Pending-request count increments on `FriendRequestCreated` for `addressee_user_id` and decrements on accept/reject/block-cancel resolution events.
- Recompute unread summary from local unread projections during replay or repair.

### `workspace_projection`

Workspace sidebar row scoped to member.

| Field | Notes |
| --- | --- |
| Primary key | `(user_id, workspace_id)` |
| Projection keys | `user_id`, `workspace_id` |
| Important indexes | `PRIMARY KEY (user_id, workspace_id)`, `INDEX (user_id, last_activity_at DESC)`, `INDEX (workspace_id)` |
| Maintained by events | `WorkspaceCreated`, `WorkspaceMemberAdded`, `WorkspaceMemberRemoved`, `WorkspaceChannelCreated`, `MessageCreated`, `MessageEdited`, `MessageDeleted`, `ConversationReadCursorUpdated` |

Suggested columns:

- `user_id`
- `workspace_id`
- `workspace_name`
- `workspace_icon_url`
- `member_count`
- `unread_count`
- `last_activity_at`
- `updated_at`

Refresh notes:

- `WorkspaceCreated` seeds creator-visible row.
- `WorkspaceMemberAdded` fans out member-visible row for new member.
- `WorkspaceMemberRemoved` deletes member-visible row and dependent channel/unread rows for member-workspace scope.
- `MessageCreated` updates activity ordering and workspace unread summary for affected members using local projections only.
- `ConversationReadCursorUpdated` repairs or reduces workspace unread summary after chat-owned cursor advances.

### `workspace_channel_projection`

Member-scoped workspace channel row.

| Field | Notes |
| --- | --- |
| Primary key | `(user_id, workspace_id, channel_id)` |
| Projection keys | `user_id`, `workspace_id`, `channel_id`, `conversation_id` |
| Important indexes | `PRIMARY KEY (user_id, workspace_id, channel_id)`, `INDEX (user_id, workspace_id, position)`, `UNIQUE (conversation_id) WHERE conversation_id IS NOT NULL` |
| Maintained by events | `WorkspaceCreated`, `WorkspaceMemberAdded`, `WorkspaceMemberRemoved`, `WorkspaceChannelCreated`, `ConversationCreated`, `MessageCreated`, `MessageEdited`, `MessageDeleted`, `ConversationReadCursorUpdated` |

Suggested columns:

- `user_id`
- `workspace_id`
- `channel_id`
- `conversation_id`
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

- New members may need backfill rows for all existing channels in workspace.
- `ConversationCreated` fills `conversation_id` for channel rows matched by `workspace_channel_id`.
- `MessageCreated` updates preview, activity, and unread fields using local row plus authoritative message sequence.
- `ConversationReadCursorUpdated` reduces unread counts using latest stored read cursor position for matching conversation.
- API ordering is `position` ascending, then `channel_id` as stable tiebreaker.

### `dm_projection`

User-scoped DM thread row.

| Field | Notes |
| --- | --- |
| Primary key | `(user_id, conversation_id)` |
| Projection keys | `user_id`, `conversation_id`, `dm_pair_id`, `peer_user_id` |
| Important indexes | `PRIMARY KEY (user_id, conversation_id)`, `INDEX (user_id, last_activity_at DESC)`, `UNIQUE (user_id, dm_pair_id)` |
| Maintained by events | `ConversationCreated`, `DmPairCreated`, `UserProfileUpdated`, `MessageCreated`, `MessageEdited`, `MessageDeleted`, `ConversationReadCursorUpdated` |

Suggested columns:

- `user_id`
- `conversation_id`
- `dm_pair_id`
- `peer_user_id`
- `peer_username`
- `peer_display_name`
- `peer_avatar_url`
- `last_message_id`
- `last_message_preview`
- `last_activity_at`
- `unread_count`
- `updated_at`

Refresh notes:

- `ConversationCreated` seeds DM thread row for both participants when target type is `dm`.
- `DmPairCreated` supplies canonical participant mapping used to derive per-user peer identity.
- `UserProfileUpdated` refreshes copied peer profile fields.
- `MessageCreated` updates preview, activity ordering, and unread count for recipient participant only.
- `ConversationReadCursorUpdated` reduces unread count for matching `(user_id, conversation_id)`.
- API ordering is `last_activity_at` descending, then `conversation_id` as stable tiebreaker.

### `user_unread_counter`

Per-user per-workspace-channel unread counter used by app and workspace responses.

| Field | Notes |
| --- | --- |
| Primary key | `(user_id, workspace_id, channel_id)` |
| Projection keys | `user_id`, `workspace_id`, `channel_id`, `conversation_id` |
| Important indexes | `PRIMARY KEY (user_id, workspace_id, channel_id)`, `INDEX (user_id, workspace_id)`, `INDEX (user_id, unread_count DESC)`, `UNIQUE (conversation_id, user_id) WHERE conversation_id IS NOT NULL` |
| Maintained by events | `WorkspaceMemberAdded`, `WorkspaceMemberRemoved`, `WorkspaceChannelCreated`, `ConversationCreated`, `MessageCreated`, `ConversationReadCursorUpdated` |

Suggested columns:

- `user_id`
- `workspace_id`
- `channel_id`
- `conversation_id`
- `last_read_conversation_message_seq`
- `unread_count`
- `mention_count`
- `updated_at`

Refresh notes:

- `ConversationCreated` fills `conversation_id` for channel unread rows matched by `workspace_channel_id`.
- `MessageCreated` increments unread counters for relevant members except author.
- `ConversationReadCursorUpdated` stores latest read sequence and resets or recomputes unread rows for matching user-conversation scope.
- Until mention-specific events exist, `mention_count` remains placeholder for later revision.

## Relations

- `user_app_projection.user_id` aggregates from `workspace_projection`, `workspace_channel_projection`, `dm_projection`, and pending-request count updates from friendship events.
- `workspace_projection (user_id, workspace_id)` is parent aggregate for `workspace_channel_projection (user_id, workspace_id, channel_id)`.
- `workspace_channel_projection.conversation_id` and `dm_projection.conversation_id` are denormalized chat-owned routing identifiers exposed directly in bootstrap reads.
- `user_unread_counter` feeds unread fields denormalized into `user_app_projection`, `workspace_projection`, and `workspace_channel_projection`.

## Projection Refresh Notes

- Consumers must be idempotent because replay and duplicate delivery are expected.
- Projection handlers should prefer row upsert patterns keyed by stable projection keys.
- Mutable display-field freshness depends on explicit upstream update events where defined. In v1, `UserProfileUpdated`, `ConversationReadCursorUpdated`, `MessageEdited`, and `MessageDeleted` are defined; workspace and channel metadata update events are deferred.
- Full rebuild should truncate and replay projection tables from durable event inputs, then validate counts against source-of-truth services.
