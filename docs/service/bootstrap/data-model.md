## Persistence Scope

Bootstrap owns projection tables only. It does not own write-side domain records and does not read another service's database directly.

Bootstrap stores two projection layers:

- **Source snapshots** copy minimal owner-service facts from integration events using owner-service identifiers.
- **Composed UI projections** store gRPC-ready rows used by app-shell reads.

AMQP handlers should stay short: validate/idempotently persist source facts, then enqueue affected composition keys. A bootstrap-local async composer drains the queue and updates composed UI projections. gRPC handlers read composed projections only.

## Ordering Contract

Source snapshots are required because RabbitMQ delivery order is not guaranteed across upstream services or queues. Bootstrap must converge when related events arrive in any order, including:

- `ConversationCreated` before `DmPairCreated`.
- `ConversationCreated` before `WorkspaceChannelCreated`.
- `WorkspaceChannelCreated` before `WorkspaceCreated` or `WorkspaceMemberAdded`.
- `MessageCreated` before the related conversation target snapshot has materialized.

Handlers must never permanently drop an event just because a dependent source fact is missing. Missing joins are resolved by later recomposition.

## Control Tables

### `processed_event`

Idempotency marker for durable event consumers.

| Field | Notes |
| --- | --- |
| Primary key | `event_id` |
| Projection keys | `event_id` |
| Important indexes | `PRIMARY KEY (event_id)` |
| Maintained by events | Every consumed event before mutating source snapshots. |

| Suggested column | Type | Notes |
| --- | --- | --- |
| `event_id` | `text` | Stable upstream event identity, normally AMQP `message_id` from outbox event id. |
| `routing_key` | `text` | Consumed routing key for diagnostics. |
| `processed_at` | `timestamptz` | Time bootstrap accepted the event. |

### `compose_queue`

Bootstrap-local queue of projection keys needing recomposition.

| Field | Notes |
| --- | --- |
| Primary key | `compose_key` |
| Projection keys | `compose_kind`, `user_id`, `workspace_id`, `channel_id`, `conversation_id`, `dm_pair_id` |
| Important indexes | `INDEX (status, available_at)`, `INDEX (compose_kind, user_id)`, `INDEX (workspace_id)`, `INDEX (conversation_id)`, `INDEX (dm_pair_id)` |
| Maintained by events | Every handler that changes source snapshots or unread/message state. |

| Suggested column | Type | Notes |
| --- | --- | --- |
| `compose_key` | `text` | Stable dedupe key, e.g. `workspace:user:workspace`, `channel:user:channel`, `dm:user:dm_pair`. |
| `compose_kind` | `text` | `user_app`, `workspace`, `workspace_channel`, `dm`, or `workspace_unread`. |
| `user_id` | `uuid null` | User scope when known. |
| `workspace_id` | `uuid null` | Workspace scope when known. |
| `channel_id` | `uuid null` | Workspace channel scope when known. |
| `conversation_id` | `uuid null` | Chat conversation scope when known. |
| `dm_pair_id` | `uuid null` | DM pair scope when known. |
| `status` | `text` | `pending`, `claimed`, or `failed`. |
| `attempts` | `int32` | Composer attempts. |
| `available_at` | `timestamptz` | Earliest compose time. |
| `claimed_at` | `timestamptz null` | Claim timestamp for retry. |
| `last_error` | `text null` | Last compose error for diagnostics. |
| `updated_at` | `timestamptz` | Last enqueue/update time. |

Queue entries should be upserted by `compose_key` so repeated upstream events coalesce instead of producing unbounded duplicate work.

## Source Snapshot Tables

### `user_snapshot`

Minimal identity-owned user display state copied for local joins.

| Field | Notes |
| --- | --- |
| Primary key | `user_id` |
| Projection keys | `user_id` |
| Important indexes | `PRIMARY KEY (user_id)` |
| Maintained by events | `UserRegistered`, `UserProfileUpdated` |

| Suggested column | Type | Notes |
| --- | --- | --- |
| `user_id` | `uuid` | Identity user id. |
| `username` | `text` | Current username. |
| `display_name` | `text` | Current display name. |
| `avatar_url` | `text null` | Current avatar. |
| `updated_at` | `timestamptz` | Source freshness marker. |

### `friend_request_snapshot`

Minimal friendship-owned request state used to derive pending badges safely.

| Field | Notes |
| --- | --- |
| Primary key | `friend_request_id` |
| Projection keys | `friend_request_id`, `addressee_user_id` |
| Important indexes | `PRIMARY KEY (friend_request_id)`, `INDEX (addressee_user_id, status)` |
| Maintained by events | `FriendRequestCreated`, `FriendRequestAccepted`, `FriendRequestRejected`, `FriendRequestCanceledByBlock` |

| Suggested column | Type | Notes |
| --- | --- | --- |
| `friend_request_id` | `uuid` | Friendship request id. |
| `requester_user_id` | `uuid` | Requesting user. |
| `addressee_user_id` | `uuid` | User receiving request. |
| `status` | `text` | `pending`, `accepted`, `rejected`, `canceled_by_block`. |
| `updated_at` | `timestamptz` | Source freshness marker. |

Pending badge count is derived from `status = 'pending'` rows, not increment/decrement counters.

### `workspace_snapshot`

Minimal workspace-owned workspace display state.

| Field | Notes |
| --- | --- |
| Primary key | `workspace_id` |
| Projection keys | `workspace_id` |
| Important indexes | `PRIMARY KEY (workspace_id)` |
| Maintained by events | `WorkspaceCreated` |

| Suggested column | Type | Notes |
| --- | --- | --- |
| `workspace_id` | `uuid` | Workspace id. |
| `name` | `text` | Create-time stable v1 display name. |
| `icon_url` | `text null` | Workspace icon, null until update events exist. |
| `owner_user_id` | `uuid` | Owner at creation time. |
| `created_at` | `timestamptz` | Source create time. |
| `updated_at` | `timestamptz` | Source freshness marker. |

### `workspace_member_snapshot`

Member fact copied from workspace events.

| Field | Notes |
| --- | --- |
| Primary key | `(workspace_id, user_id)` |
| Projection keys | `workspace_id`, `user_id` |
| Important indexes | `PRIMARY KEY (workspace_id, user_id)`, `INDEX (user_id, status)`, `INDEX (workspace_id, status)` |
| Maintained by events | `WorkspaceMemberAdded`, `WorkspaceMemberRemoved` |

| Suggested column | Type | Notes |
| --- | --- | --- |
| `workspace_id` | `uuid` | Workspace id. |
| `user_id` | `uuid` | Member user id. |
| `status` | `text` | `active` or `removed`. |
| `joined_at` | `timestamptz null` | Last join time. |
| `removed_at` | `timestamptz null` | Last removal time. |
| `updated_at` | `timestamptz` | Source freshness marker. |

Removed members stay as inactive source rows so replay and recomposition remain deterministic.

### `workspace_channel_snapshot`

Channel fact copied from workspace events.

| Field | Notes |
| --- | --- |
| Primary key | `channel_id` |
| Projection keys | `channel_id`, `workspace_id` |
| Important indexes | `PRIMARY KEY (channel_id)`, `INDEX (workspace_id, position, channel_id)` |
| Maintained by events | `WorkspaceChannelCreated` |

| Suggested column | Type | Notes |
| --- | --- | --- |
| `channel_id` | `uuid` | Workspace channel id. |
| `workspace_id` | `uuid` | Owning workspace id. |
| `name` | `text` | Create-time stable v1 display name. |
| `channel_kind` | `text` | Channel kind. |
| `position` | `int32` | Sidebar order. |
| `created_at` | `timestamptz` | Source create time. |
| `updated_at` | `timestamptz` | Source freshness marker. |

### `dm_pair_snapshot`

Chat-owned normalized DM pair fact.

| Field | Notes |
| --- | --- |
| Primary key | `dm_pair_id` |
| Projection keys | `dm_pair_id`, `low_user_id`, `high_user_id` |
| Important indexes | `PRIMARY KEY (dm_pair_id)`, `INDEX (low_user_id)`, `INDEX (high_user_id)` |
| Maintained by events | `DmPairCreated` |

| Suggested column | Type | Notes |
| --- | --- | --- |
| `dm_pair_id` | `uuid` | Chat DM pair id. |
| `low_user_id` | `uuid` | Canonically lower participant id. |
| `high_user_id` | `uuid` | Canonically higher participant id. |
| `created_at` | `timestamptz` | Source create time. |
| `updated_at` | `timestamptz` | Source freshness marker. |

### `conversation_snapshot`

Chat-owned conversation routing fact.

| Field | Notes |
| --- | --- |
| Primary key | `conversation_id` |
| Projection keys | `conversation_id`, `dm_pair_id`, `workspace_channel_id` |
| Important indexes | `PRIMARY KEY (conversation_id)`, `INDEX (dm_pair_id)`, `INDEX (workspace_channel_id)` |
| Maintained by events | `ConversationCreated` |

| Suggested column | Type | Notes |
| --- | --- | --- |
| `conversation_id` | `uuid` | Chat conversation id. |
| `target_type` | `text` | `dm` or `workspace_channel`. |
| `dm_pair_id` | `uuid null` | Present for DM conversations. |
| `workspace_channel_id` | `uuid null` | Present for workspace channel conversations. |
| `created_at` | `timestamptz` | Source create time. |
| `updated_at` | `timestamptz` | Source freshness marker. |

### `conversation_message_state`

Latest-message state per conversation used for activity ordering, previews, and edit/delete anchoring.

| Field | Notes |
| --- | --- |
| Primary key | `conversation_id` |
| Projection keys | `conversation_id`, `last_message_id` |
| Important indexes | `PRIMARY KEY (conversation_id)`, `INDEX (last_message_id)` |
| Maintained by events | `MessageCreated`, `MessageEdited`, `MessageDeleted` |

| Suggested column | Type | Notes |
| --- | --- | --- |
| `conversation_id` | `uuid` | Chat conversation id. |
| `last_message_id` | `uuid null` | Current preview anchor. |
| `last_message_author_user_id` | `uuid null` | Author of current preview anchor, used to avoid counting a user's own latest message as unread. |
| `last_message_seq` | `int64 null` | Latest conversation sequence seen. |
| `last_message_preview` | `text null` | Current preview text or tombstone. |
| `last_activity_at` | `timestamptz null` | Activity sort timestamp. |
| `updated_at` | `timestamptz` | State freshness marker. |

`MessageEdited` and `MessageDeleted` may update this row only when `message_id = last_message_id`.

### `conversation_read_state`

Read cursor state per user/conversation.

| Field | Notes |
| --- | --- |
| Primary key | `(conversation_id, user_id)` |
| Projection keys | `conversation_id`, `user_id` |
| Important indexes | `PRIMARY KEY (conversation_id, user_id)`, `INDEX (user_id)` |
| Maintained by events | `ConversationReadCursorUpdated` |

| Suggested column | Type | Notes |
| --- | --- | --- |
| `conversation_id` | `uuid` | Chat conversation id. |
| `user_id` | `uuid` | Reader user id. |
| `last_read_conversation_message_seq` | `int64` | Authoritative chat-owned read cursor. |
| `read_at` | `timestamptz` | Cursor update time. |
| `updated_at` | `timestamptz` | State freshness marker. |

## Composed UI Projection Tables

### `user_app_projection`

App-shell aggregate keyed by user.

| Field | Notes |
| --- | --- |
| Primary key | `user_id` |
| Projection keys | `user_id` |
| Important indexes | `PRIMARY KEY (user_id)` |
| Composed from | `user_snapshot`, `friend_request_snapshot` |

| Suggested column | Type | Notes |
| --- | --- | --- |
| `user_id` | `uuid` | Viewer identity. |
| `username` | `text` | Current username. |
| `display_name` | `text` | Current display name. |
| `avatar_url` | `text null` | Current avatar. |
| `pending_friend_request_count` | `int32` | Count of pending incoming friend requests. |
| `updated_at` | `timestamptz` | Projection freshness marker. |

### `workspace_projection`

Workspace sidebar row scoped to member.

| Field | Notes |
| --- | --- |
| Primary key | `(user_id, workspace_id)` |
| Projection keys | `user_id`, `workspace_id` |
| Important indexes | `PRIMARY KEY (user_id, workspace_id)`, `INDEX (user_id, workspace_name, workspace_id)` |
| Composed from | `workspace_snapshot`, `workspace_member_snapshot`, `workspace_unread_projection` |

| Suggested column | Type | Notes |
| --- | --- | --- |
| `user_id` | `uuid` | Member-scoped owner key. |
| `workspace_id` | `uuid` | Workspace reference. |
| `workspace_name` | `text` | Display name for sidebar. |
| `workspace_icon_url` | `text null` | Sidebar icon. |
| `member_count` | `int32` | Active member count. |
| `unread_count` | `int32` | Workspace unread badge. |
| `updated_at` | `timestamptz` | Projection freshness marker. |

Rows exist only for active members. Removed membership recomposition deletes the composed row and dependent channel rows.

### `workspace_channel_projection`

Member-scoped workspace channel row.

| Field | Notes |
| --- | --- |
| Primary key | `(user_id, workspace_id, channel_id)` |
| Projection keys | `user_id`, `workspace_id`, `channel_id`, `conversation_id` |
| Important indexes | `PRIMARY KEY (user_id, workspace_id, channel_id)`, `INDEX (user_id, workspace_id, position, channel_id)`, `INDEX (conversation_id)` |
| Composed from | `workspace_member_snapshot`, `workspace_channel_snapshot`, `conversation_snapshot`, `workspace_channel_unread_projection` |

| Suggested column | Type | Notes |
| --- | --- | --- |
| `user_id` | `uuid` | Member-scoped owner key. |
| `workspace_id` | `uuid` | Workspace reference. |
| `channel_id` | `uuid` | Workspace channel reference. |
| `conversation_id` | `uuid null` | Chat routing id for channel; nullable until conversation event arrives. |
| `channel_name` | `text` | Display name. |
| `channel_kind` | `text` | Channel kind. |
| `position` | `int32` | Sidebar order. |
| `last_message_seq` | `int64 null` | Latest message seq seen. |
| `last_read_conversation_message_seq` | `int64 null` | Read cursor for this user/channel. |
| `unread_count` | `int32` | Unread badge. |
| `updated_at` | `timestamptz` | Projection freshness marker. |

API ordering is `position` ascending, then `channel_id`.

### `dm_projection`

User-scoped DM thread row.

| Field | Notes |
| --- | --- |
| Primary key | `(user_id, dm_pair_id)` |
| Projection keys | `user_id`, `dm_pair_id`, `conversation_id` |
| Important indexes | `PRIMARY KEY (user_id, dm_pair_id)`, `INDEX (user_id, last_activity_at DESC, conversation_id)`, `INDEX (conversation_id)`, `INDEX (dm_pair_id)` |
| Composed from | `dm_pair_snapshot`, `conversation_snapshot`, `user_snapshot`, `conversation_message_state`, `dm_unread_projection` |

| Suggested column | Type | Notes |
| --- | --- | --- |
| `user_id` | `uuid` | User owner key. |
| `conversation_id` | `uuid null` | Chat conversation id, nullable until `ConversationCreated`. |
| `dm_pair_id` | `uuid` | Normalized DM pair id. |
| `peer_user_id` | `uuid` | Other participant. |
| `peer_username` | `text` | Peer username snapshot. |
| `peer_display_name` | `text` | Peer display name snapshot. |
| `peer_avatar_url` | `text null` | Peer avatar snapshot. |
| `last_message_seq` | `int64 null` | Latest message seq seen. |
| `last_read_conversation_message_seq` | `int64 null` | Read cursor for this user/thread. |
| `last_message_preview` | `text null` | Latest preview text. |
| `last_activity_at` | `timestamptz null` | Sort key for DM list. |
| `unread_count` | `int32` | Unread badge. |
| `updated_at` | `timestamptz` | Projection freshness marker. |

API ordering is `last_activity_at` descending, then `conversation_id`.

## Unread Projection Tables

### `workspace_channel_unread_projection`

Per-user unread state for a workspace channel conversation.

| Field | Notes |
| --- | --- |
| Primary key | `(user_id, channel_id)` |
| Projection keys | `user_id`, `channel_id`, `conversation_id`, `workspace_id` |
| Important indexes | `PRIMARY KEY (user_id, channel_id)`, `INDEX (user_id, workspace_id)`, `INDEX (conversation_id)` |
| Composed from | `workspace_channel_snapshot`, `conversation_snapshot`, `conversation_message_state`, `conversation_read_state` |

| Suggested column | Type | Notes |
| --- | --- | --- |
| `user_id` | `uuid` | Member user id. |
| `workspace_id` | `uuid` | Workspace id. |
| `channel_id` | `uuid` | Channel id. |
| `conversation_id` | `uuid` | Conversation id. |
| `last_message_seq` | `int64 null` | Latest message seq. |
| `last_read_conversation_message_seq` | `int64 null` | User read cursor. |
| `unread_count` | `int32` | Derived unread count. |
| `updated_at` | `timestamptz` | Projection freshness marker. |

### `dm_unread_projection`

Per-user unread state for a DM conversation.

| Field | Notes |
| --- | --- |
| Primary key | `(user_id, dm_pair_id)` |
| Projection keys | `user_id`, `dm_pair_id`, `conversation_id` |
| Important indexes | `PRIMARY KEY (user_id, dm_pair_id)`, `INDEX (conversation_id)` |
| Composed from | `dm_pair_snapshot`, `conversation_snapshot`, `conversation_message_state`, `conversation_read_state` |

| Suggested column | Type | Notes |
| --- | --- | --- |
| `user_id` | `uuid` | DM participant. |
| `dm_pair_id` | `uuid` | DM pair id. |
| `conversation_id` | `uuid` | Conversation id. |
| `last_message_seq` | `int64 null` | Latest message seq. |
| `last_read_conversation_message_seq` | `int64 null` | User read cursor. |
| `unread_count` | `int32` | Derived unread count. |
| `updated_at` | `timestamptz` | Projection freshness marker. |

### `workspace_unread_projection`

Per-user workspace unread summary.

| Field | Notes |
| --- | --- |
| Primary key | `(user_id, workspace_id)` |
| Projection keys | `user_id`, `workspace_id` |
| Important indexes | `PRIMARY KEY (user_id, workspace_id)` |
| Composed from | `workspace_channel_unread_projection` |

| Suggested column | Type | Notes |
| --- | --- | --- |
| `user_id` | `uuid` | Member user id. |
| `workspace_id` | `uuid` | Workspace id. |
| `unread_count` | `int32` | Sum of channel unread counts. |
| `updated_at` | `timestamptz` | Projection freshness marker. |

V1 unread is sequence-gap based: `max(last_message_seq - last_read_seq, 0)`, capped to `int32`, and zero for message authors where author context is available during message-state composition. Exact deletion-aware unread repair can be added later if message-level read state is introduced.

## Projection Refresh Notes

- Consumers must be idempotent because replay and duplicate delivery are expected.
- Every handler should insert `processed_event` inside the same transaction as source snapshot updates and compose-queue upserts.
- Source snapshot handlers should prefer upsert patterns keyed by stable owner-service identifiers.
- Composition should be retryable, coalesced by `compose_queue.compose_key`, and safe to run repeatedly.
- gRPC methods read composed UI projections only and should not perform runtime cross-service fanout.
- Full rebuild can truncate composed UI projections and unread projections, then enqueue composition from source snapshots. Full source rebuild requires replaying durable upstream events.
