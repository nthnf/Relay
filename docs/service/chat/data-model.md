## Persistence Scope

Chat owns message-write state plus minimal direct-message conversation state in v1: message bodies, edit history, soft-delete state, reactions, direct conversations, and direct-conversation participants. Other services must use chat gRPC or chat events instead of reading these tables directly.

## Direct Message Scope

- V1 direct messages are 1:1 only; group DMs are out of scope.
- `direct_conversation_id` is the stable chat-owned identifier for a 1:1 conversation.
- Participant membership is modeled explicitly so authorization for DM reads and writes stays inside chat's own boundary.

## Core Tables

### `direct_conversation`

One row per durable 1:1 direct-message conversation.

| Column | Type | Notes |
| --- | --- | --- |
| `direct_conversation_id` | `uuid` | Primary key. Service-owned DM conversation identifier. |
| `conversation_kind` | `text` | Contract value `dm_1_to_1` in v1. |
| `pair_key` | `text` | Canonical unordered participant key derived from the sorted two user IDs. |
| `created_by_user_id` | `uuid` | Identity-owned actor who caused the conversation to be created. |
| `created_at` | `timestamptz` | Conversation creation time. |

Semantic rules:

- `direct_conversation` exists only for 1:1 direct messages in v1.
- The conversation row is durable even before the first message is sent once created through `GetOrCreateDirectConversation`.
- There is exactly one durable conversation for a given unordered pair of active participants in v1.
- `pair_key` is the race-safe uniqueness key for the conversation and is derived from the two sorted participant user IDs so `(A,B)` and `(B,A)` resolve to the same value.

### `direct_conversation_member`

Participant membership for a direct conversation.

| Column | Type | Notes |
| --- | --- | --- |
| `direct_conversation_id` | `uuid` | Part of the composite key. |
| `user_id` | `uuid` | Identity-owned participant reference, part of the composite key. |
| `joined_at` | `timestamptz` | Membership creation time. |

Semantic rules:

- Each 1:1 conversation has exactly two active membership rows in v1.
- Membership uniqueness is `(direct_conversation_id, user_id)`.
- `GetOrCreateDirectConversation` creates the conversation row plus both membership rows in one transaction when the pair does not already exist.
- Membership rows are durable authorization state for DM reads and writes; chat does not depend on `workspace` to authorize direct messages.

### `chat_message`

One row per durable message accepted by chat.

| Column | Type | Notes |
| --- | --- | --- |
| `message_id` | `uuid` | Primary key. Service-owned message identifier. |
| `workspace_id` | `uuid null` | Workspace-owned reference copied for channel-message authorization and event context. Null for direct messages. |
| `channel_id` | `uuid null` | Workspace-owned channel reference. Null for direct messages. |
| `direct_conversation_id` | `uuid null` | Chat-owned direct-message conversation reference. Null for workspace-channel messages. |
| `author_user_id` | `uuid` | Identity-owned message author reference. |
| `client_message_id` | `text null` | Optional client-supplied idempotency key for retry-safe creates. |
| `target_message_seq` | `bigint` | Durable ordering value assigned within the message target on create. |
| `body` | `text` | Current visible message body. |
| `message_status` | `text` | Contract values: `active`, `deleted`. |
| `created_at` | `timestamptz` | Message creation time. |
| `updated_at` | `timestamptz` | Last mutation time. |
| `deleted_at` | `timestamptz null` | Null until soft delete. |
| `deleted_by_user_id` | `uuid null` | Identity-owned actor who performed the delete. |
| `last_edited_at` | `timestamptz null` | Null until the first successful edit. |
| `last_edited_by_user_id` | `uuid null` | Identity-owned actor who performed the latest edit. |

Semantic rules:

- `chat_message` is the source-of-truth row for whether a message currently exists, is deleted, and what body is currently visible.
- Each message belongs to exactly one target kind: either `(workspace_id, channel_id)` for a workspace-channel message or `direct_conversation_id` for a DM. Both target kinds must never be populated at once, and at least one target kind must be present.
- When `client_message_id` is present, it must be unique per message target and author so duplicate send retries resolve to the original message.
- Ordering is scoped to the message target: `(channel_id, target_message_seq)` for channel messages or `(direct_conversation_id, target_message_seq)` for DMs.
- `target_message_seq` is monotonic within one target only; it is not comparable across channels or direct conversations.
- Soft delete keeps the row for history, idempotency, and downstream recovery; deleted rows set `message_status = deleted`, `deleted_at`, and `deleted_by_user_id` instead of being hard-removed in v1.
- `workspace_id` is stored with `channel_id` only for workspace-channel messages so event payloads and authorization checks do not require a cross-service read after persistence.
- Duplicate retry detection keyed by author plus the message target plus `client_message_id` returns the original durable message response and must not create another `chat_message` row.

### `chat_message_edit`

Append-only edit history for mutable messages.

| Column | Type | Notes |
| --- | --- | --- |
| `message_edit_id` | `uuid` | Primary key. Service-owned edit identifier. |
| `message_id` | `uuid` | Message being edited. |
| `editor_user_id` | `uuid` | Identity-owned actor who performed the edit. |
| `edit_version` | `int` | Monotonic version number starting at `1` for the first edit. |
| `prior_body` | `text` | Message body before the edit was applied. |
| `new_body` | `text` | Message body after the edit was applied. |
| `edited_at` | `timestamptz` | Edit timestamp. |

Semantic rules:

- Editor tracking is explicit: every successful edit records `editor_user_id` even if the editor is the original author.
- `edit_version` should be unique per `message_id` so edit history remains ordered and replayable.
- Editing updates `chat_message.body`, `last_edited_at`, and `last_edited_by_user_id` in the same transaction that inserts the new edit-history row.
- Deleted messages are not edited in v1; edit attempts after delete should fail or return a documented conflict.

### `chat_message_reaction`

Current reaction membership for a message.

| Column | Type | Notes |
| --- | --- | --- |
| `message_id` | `uuid` | Part of the composite key. |
| `reaction_key` | `text` | Contracted reaction token such as a Unicode emoji or application-defined key. |
| `user_id` | `uuid` | Identity-owned reacting user reference, part of the composite key. |
| `created_at` | `timestamptz` | Time the reaction was added. |
| `removed_at` | `timestamptz null` | Null while active if soft-retained for audit/idempotency. |

Semantic rules:

- Reaction uniqueness is `(message_id, reaction_key, user_id)`; chat must never persist two active identical reactions from the same user on the same message.
- `AddReaction` should be idempotent when the active reaction already exists.
- `RemoveReaction` should mark or remove the matching row in a way that preserves idempotent repeated removals.
- Reactions attach to the durable message row, not to edit-history rows.
- Reactions to deleted messages are rejected in v1.

## Relations

- `chat_message_edit.message_id -> chat_message.message_id`
- `chat_message_reaction.message_id -> chat_message.message_id`
- `direct_conversation_member.direct_conversation_id -> direct_conversation.direct_conversation_id`
- `chat_message.direct_conversation_id -> direct_conversation.direct_conversation_id`
- `chat_message.(channel_id, workspace_id)` references workspace-owned channel context by value only when the target kind is `workspace_channel`; it is not a cross-database foreign key.

## Index and Constraint Notes

- Check constraint should enforce the message target invariant: exactly one of `direct_conversation_id` or `(workspace_id, channel_id)` is populated.
- Unique partial index on `(channel_id, target_message_seq)` where `channel_id IS NOT NULL` preserves stable channel-scoped ordering.
- Unique partial index on `(direct_conversation_id, target_message_seq)` where `direct_conversation_id IS NOT NULL` preserves stable DM ordering.
- Unique partial index on `(author_user_id, channel_id, client_message_id)` where `client_message_id IS NOT NULL AND channel_id IS NOT NULL` enforces channel `CreateMessage` idempotency.
- Unique partial index on `(author_user_id, direct_conversation_id, client_message_id)` where `client_message_id IS NOT NULL AND direct_conversation_id IS NOT NULL` enforces DM `CreateMessage` idempotency.
- Unique constraint on `direct_conversation.pair_key` enforces that only one 1:1 `direct_conversation` exists for a given unordered user pair in v1.
- Unique constraint on `(message_id, edit_version)` preserves ordered edit history.
- Unique active-reaction constraint on `(message_id, reaction_key, user_id)` preserves per-user reaction uniqueness.

## Cross-Service References

- `workspace` owns `workspace_id` and `channel_id`; chat references them for authorization context and event payloads but does not own membership or channel metadata.
- `identity` owns `author_user_id`, `created_by_user_id`, `editor_user_id`, `deleted_by_user_id`, `last_edited_by_user_id`, and `user_id` participant or reaction references.
- External application servers call chat through Envoy Gateway; chat must not trust arbitrary caller-supplied actor identity and must authorize from Envoy-validated access-token context at its own boundary.
- Chat authorizes direct-message reads and writes from `direct_conversation_member` rows it owns.
- `realtime` receives best-effort synchronous `PublishEvent` calls after durable writes and also consumes durable chat events for repair or catch-up behavior.
- `bootstrap` and other downstream consumers materialize projections from durable chat events rather than querying chat tables directly.
- Chat inserts integration events into its local `outbox_event` table in the same transaction as the source write.

## Retention Notes

- `chat_message` rows are retained after soft delete in v1 to preserve ordered history, idempotency, and replay context.
- `direct_conversation` and `direct_conversation_member` rows are retained as durable DM routing and authorization state in v1.
- `chat_message_edit` is append-only and retained as durable edit history unless a later retention policy explicitly supersedes it.
- `chat_message_reaction` may be soft-retained for audit and idempotency, but only active reactions should appear in user-facing reads.
- Any future purge policy must preserve the contract that durable downstream consumers can reconcile from previously published events even after hot data ages out.
