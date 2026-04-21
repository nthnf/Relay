## Persistence Scope

Chat owns durable conversation and message-write state in v1: conversation identity, DM participant membership, minimal legitimacy snapshots, current message state, and soft-delete state. Other services must use chat gRPC or chat events instead of reading these tables directly.

## Model Direction

- Chat uses one durable `conversation_id` for both workspace-channel and DM message targets.
- `conversation.target_type` distinguishes `workspace_channel` from `dm`.
- `chat_message` points only to `conversation_id`.
- Workspace-channel authorization remains synchronous to `workspace`; chat does not own workspace membership or permission truth.
- User, workspace, and channel snapshots exist only so chat can reject unknown targets before or alongside sync auth.
- DM authorization remains local to chat through `conversation_member` rows.

## Core Tables

### `conversation`

One row per durable message target.

| Column                 | Type          | Notes                                                                                 |
| ---------------------- | ------------- | ------------------------------------------------------------------------------------- |
| `conversation_id`      | `uuid`        | Primary key. Stable chat-owned target identifier for both DMs and workspace channels. |
| `target_type`          | `text`        | Contract values: `dm`, `workspace_channel`.                                           |
| `workspace_channel_id` | `uuid null`   | Workspace-owned channel reference for channel conversations only.                     |
| `created_by_user_id`   | `uuid`        | Identity-owned actor who caused conversation row to exist.                            |
| `created_at`           | `timestamptz` | Conversation creation time.                                                           |

Semantic rules:

- `conversation` is single durable target table for both DMs and workspace channels.
- For `target_type = dm`, `workspace_channel_id` must be null.
- For `target_type = workspace_channel`, `workspace_channel_id` is required.
- Workspace-channel conversations are created on demand only after local snapshot existence checks and synchronous `workspace.AuthorizeChannelAction` checks pass.
- DM conversations are created on demand.
- If v1 still requires one durable DM conversation per unordered pair, application logic must enforce that invariant during `CreateConversation` handling because `dm_pair_key` is no longer stored on `conversation`.

### `conversation_member`

Participant membership for DM conversations.

| Column                   | Type          | Notes                                          |
| ------------------------ | ------------- | ---------------------------------------------- |
| `conversation_member_id` | `uuid`        | Primary key. Stable membership row identifier. |
| `conversation_id`        | `uuid`        | Refers to `conversation.conversation_id`.      |
| `user_id`                | `uuid`        | Identity-owned participant reference.          |
| `joined_at`              | `timestamptz` | Membership creation time.                      |

Semantic rules:

- `conversation_member` exists only for `target_type = dm` in v1.
- Each 1:1 DM conversation has exactly two active membership rows in v1.
- Membership uniqueness is `(conversation_id, user_id)`.
- Membership rows are durable authorization state for DM reads and writes.

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

## Relations

- `chat_message.conversation_id -> conversation.conversation_id`
- `conversation_member.conversation_id -> conversation.conversation_id`
- `workspace_channel_snapshot.workspace_id -> workspace_snapshot.workspace_id` by value only, not cross-service foreign key

## Index and Constraint Notes

- Check constraint should enforce `conversation.target_type` invariant:
  - `dm` => `workspace_channel_id` null
  - `workspace_channel` => `workspace_channel_id` set
- Unique constraint on `conversation.workspace_channel_id` where not null preserves one conversation per workspace channel.
- Unique constraint on `(conversation_id, user_id)` preserves DM membership uniqueness.
- Unique constraint on `(conversation_id, conversation_message_seq)` preserves stable per-conversation ordering.
- Unique constraint on `(author_user_id, conversation_id, client_message_id)` where `client_message_id IS NOT NULL` enforces retry-safe create semantics.

## Cross-Service References

- `workspace` owns `workspace_channel_id`; chat copies it into `conversation` for target context only.
- `workspace` remains authority for workspace-channel access and permission decisions through synchronous gRPC checks.
- `identity` owns all user IDs used for authorship, delete actors, and DM participants.
- `identity` remains authority for whether user IDs are legitimate; chat only keeps minimal user legitimacy snapshot.
- Chat authorizes DMs locally from `conversation_member` rows it owns.
- `realtime` receives best-effort synchronous `DeliverMessage` calls after durable writes and also consumes durable chat events for repair or catch-up.
- `bootstrap` and other downstream consumers materialize projections from durable chat events rather than querying chat tables directly.

## Retention Notes

- `chat_message` rows remain after soft delete in v1.
- `conversation` rows remain durable target identity for both DMs and workspace channels.
- `conversation_member` rows remain durable DM routing and authorization state.
- Snapshot tables may be rebuilt from durable upstream events if needed.
