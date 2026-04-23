## Persistence Scope

Chat owns durable conversation and message-write state in v1: conversation identity, normalized 1:1 DM participant pairs, minimal legitimacy snapshots, per-user read cursor state, current message state, and soft-delete state. Other services must use chat gRPC or chat events instead of reading these tables directly.

## Model Direction

- Chat uses one durable `conversation_id` for both workspace-channel and DM message targets.
- `conversation.target_type` distinguishes `channel` from `dm`.
- `conversation_id` remains chat-owned and distinct from `workspace_channel_id` even for channel conversations.
- `conversation_read_cursor` is chat-owned write model for per-user read progress.
- `chat_message` points only to `conversation_id`.
- Workspace-channel authorization remains synchronous to `workspace`; chat does not own workspace membership or permission truth.
- User, workspace, and channel snapshots exist only so chat can reject unknown targets before or alongside sync auth.
- DM authorization remains local to chat through one normalized DM pair row linked from `conversation`.

## Core Tables

### `conversation`

One row per durable message target.

| Column                 | Type          | Notes                                                                                 |
| ---------------------- | ------------- | ------------------------------------------------------------------------------------- |
| `conversation_id`      | `uuid`        | Primary key. Stable chat-owned target identifier for both DMs and workspace channels. |
| `target_type`          | `text`        | Contract values: `dm`, `channel`.                                                      |
| `dm_pair_id`           | `uuid null`   | DM pair reference for DM conversations only.                                           |
| `workspace_channel_id` | `uuid null`   | Workspace-owned channel reference for channel conversations only.                     |
| `created_at`           | `timestamptz` | Conversation creation time.                                                           |

Semantic rules:

- `conversation` is single durable target table for both DMs and workspace channels.
- For `target_type = dm`, `dm_pair_id` must be set.
- For `target_type = dm`, `workspace_channel_id` must be null.
- For `target_type = channel`, `workspace_channel_id` is required.
- `conversation_id` is the stable chat-owned message-stream address for both target types.
- Workspace-channel conversations are created once after the channel already exists, typically as a follow-up `chat.CreateConversation` call after successful channel creation.
- DM conversations are created from explicit user action such as dedicated DM-create button.
- One workspace channel may map to only one durable chat conversation, but the IDs remain separate because `workspace` owns channels and `chat` owns conversation streams.
- Chat publishes `ConversationCreated` for every new durable conversation and `DmPairCreated` when it creates a new normalized DM pair.

### `dm_pair`

One normalized participant-pair row for each 1:1 DM conversation.

| Column            | Type          | Notes                                                                 |
| ----------------- | ------------- | --------------------------------------------------------------------- |
| `id`              | `uuid`        | Primary key. Stable 1:1 participant-pair identifier.                  |
| `low_user_id`     | `uuid`        | Lower UUID of the two DM participants after canonical ordering.       |
| `high_user_id`    | `uuid`        | Higher UUID of the two DM participants after canonical ordering.      |
| `created_at`      | `timestamptz` | DM pair row creation time.                                            |

Semantic rules:

- `dm_pair` exists only for `target_type = dm` in v1.
- Each 1:1 DM conversation has exactly one `dm_pair` row in v1, reached through `conversation.dm_pair_id`.
- Application logic must derive `low_user_id` and `high_user_id` by comparing the raw UUID values, not any encoded public form.
- `low_user_id < high_user_id` must always hold, preventing self-DM rows and reversed duplicates.
- Pair uniqueness is `(low_user_id, high_user_id)`.
- The row is durable authorization and routing state for DM reads and writes.

### `user_snapshot`

Minimal local copy of user legitimacy.

| Column       | Type          | Notes                                  |
| ------------ | ------------- | -------------------------------------- |
| `user_id`    | `uuid`        | Primary key. Identity-owned reference. |
| `created_at` | `timestamptz` | Snapshot create time.                  |
| `updated_at` | `timestamptz` | Snapshot refresh time.                 |

Semantic rules:

- `user_snapshot` is legitimacy metadata only.
- Chat may use this table to reject DM conversation creation for unknown users.
- Chat must not use this table as profile truth.

### `workspace_snapshot`

Minimal local copy of workspace legitimacy.

| Column         | Type          | Notes                                   |
| -------------- | ------------- | --------------------------------------- |
| `workspace_id` | `uuid`        | Primary key. Workspace-owned reference. |
| `created_at`   | `timestamptz` | Snapshot create time.                   |
| `updated_at`   | `timestamptz` | Snapshot refresh time.                  |

Semantic rules:

- `workspace_snapshot` is legitimacy metadata only.
- Chat must not use this table as permission truth.

### `workspace_channel_snapshot`

Minimal local copy of workspace channel legitimacy.

| Column                 | Type          | Notes                                           |
| ---------------------- | ------------- | ----------------------------------------------- |
| `workspace_channel_id` | `uuid`        | Primary key. Workspace-owned channel reference. |
| `workspace_id`         | `uuid`        | Parent workspace reference.                     |
| `channel_kind`         | `text`        | Latest known channel kind.                      |
| `created_at`           | `timestamptz` | Snapshot create time.                           |
| `updated_at`           | `timestamptz` | Snapshot refresh time.                          |

Semantic rules:

- `workspace_channel_snapshot` is legitimacy metadata only.
- Chat may use this table to reject unknown workspace-channel targets.
- Authorization for workspace-channel conversations still comes from synchronous `workspace` RPC, not this snapshot.

### `chat_message`

One row per durable message accepted by chat.

| Column                     | Type               | Notes                                                              |
| -------------------------- | ------------------ | ------------------------------------------------------------------ |
| `message_id`               | `uuid`             | Primary key. Service-owned message identifier.                     |
| `conversation_id`          | `uuid`             | Durable target reference for both DMs and workspace channels.      |
| `author_user_id`           | `uuid`             | Identity-owned message author reference.                           |
| `client_message_id`        | `text null`        | Optional client-supplied idempotency key for retry-safe creates.   |
| `conversation_message_seq` | `bigint`           | Durable ordering value assigned within one conversation on create. |
| `body`                     | `text`             | Current visible message body.                                      |
| `message_status`           | `text`             | Contract values: `active`, `deleted`.                              |
| `created_at`               | `timestamptz`      | Message creation time.                                             |
| `updated_at`               | `timestamptz`      | Last mutation time.                                                |
| `deleted_at`               | `timestamptz null` | Null until soft delete.                                            |
| `deleted_by_user_id`       | `uuid null`        | Identity-owned actor who performed delete.                         |
| `last_edited_at`           | `timestamptz null` | Null until first successful edit.                                  |
| `last_edited_by_user_id`   | `uuid null`        | Identity-owned actor who performed latest edit.                    |

Semantic rules:

- `chat_message` is source-of-truth row for current message visibility and body.
- Every message belongs to exactly one `conversation_id`.
- `client_message_id`, when present, must be unique per `(author_user_id, conversation_id)`.
- Ordering is scoped to one conversation through `conversation_message_seq`.
- Soft delete keeps row for history, idempotency, and downstream recovery.

### `conversation_read_cursor`

One row per user and conversation storing latest durable read progress accepted by chat.

| Column                           | Type          | Notes                                                            |
| -------------------------------- | ------------- | ---------------------------------------------------------------- |
| `user_id`                        | `uuid`        | Identity-owned reader reference.                                 |
| `conversation_id`                | `uuid`        | Durable target reference for both DMs and workspace channels.    |
| `last_read_conversation_message_seq` | `bigint`   | Highest durable conversation sequence marked read by user.       |
| `read_at`                        | `timestamptz` | Time cursor last advanced.                                       |
| `updated_at`                     | `timestamptz` | Last mutation time.                                              |

Semantic rules:

- `conversation_read_cursor` is source-of-truth user read-progress state for unread convergence.
- There is at most one row per `(user_id, conversation_id)`.
- Cursor is monotonic and must never move backward.
- Workspace-channel cursor writes still require synchronous workspace read authorization.
- DM cursor writes are authorized locally through `dm_pair` ownership.

## Relations

- `chat_message.conversation_id -> conversation.conversation_id`
- `conversation_read_cursor.conversation_id -> conversation.conversation_id`
- `conversation.dm_pair_id -> dm_pair.id`
- `workspace_channel_snapshot.workspace_id -> workspace_snapshot.workspace_id` by value only, not cross-service foreign key

## Index and Constraint Notes

- Check constraint should enforce `conversation.target_type` invariant:
  - `dm` => `workspace_channel_id` null
  - `channel` => `workspace_channel_id` set
- Check constraint should enforce `conversation.dm_pair_id` invariant:
  - `dm` => `dm_pair_id` not null
  - `channel` => `dm_pair_id` null
- Unique constraint on `conversation.workspace_channel_id` where not null preserves one conversation per workspace channel.
- Check constraint on `dm_pair` should enforce `low_user_id < high_user_id`.
- Unique constraint on `(low_user_id, high_user_id)` preserves one durable DM conversation per unordered pair.
- Unique constraint on `(conversation_id, conversation_message_seq)` preserves stable per-conversation ordering.
- Unique constraint on `(author_user_id, conversation_id, client_message_id)` where `client_message_id IS NOT NULL` enforces retry-safe create semantics.
- Unique constraint on `(user_id, conversation_id)` preserves one read cursor per actor and conversation.

## Cross-Service References

- `workspace` owns `workspace_channel_id`; chat copies it into `conversation` for target context only.
- `workspace` remains authority for workspace-channel access and permission decisions through synchronous gRPC checks.
- `identity` owns all user IDs used for authorship, delete actors, and DM participants.
- `identity` remains authority for whether user IDs are legitimate; chat only keeps minimal user legitimacy snapshot.
- Chat authorizes DMs locally from `dm_pair` rows it owns.
- `realtime` receives best-effort synchronous `DeliverMessage` calls after durable writes and also consumes durable chat events for repair or catch-up.
- `bootstrap` and other downstream consumers materialize projections from durable chat events rather than querying chat tables directly.

## Retention Notes

- `chat_message` rows remain after soft delete in v1.
- `conversation` rows remain durable target identity for both DMs and workspace channels.
- `conversation_read_cursor` rows remain durable read-progress state and may be replay-rebuilt only from chat-owned source data.
- `dm_pair` rows remain durable DM routing and authorization state.
- Snapshot tables may be rebuilt from durable upstream events if needed.
