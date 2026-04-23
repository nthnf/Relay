## Publication Model

Chat publishes integration events by inserting service-owned rows into `outbox_event` inside same transaction as source write. Shared outbox worker later publishes those rows to RabbitMQ. Same event family covers conversation identity, read-cursor state, and both workspace-channel and DM conversation messages in v1.

## Published Events

Guard rail: use `delivery_id` only for event families that also feed `realtime` and therefore need delivery deduplication or direct-versus-durable reconciliation. In v1 that includes `MessageCreated`, `MessageEdited`, and `MessageDeleted`. `ConversationCreated`, `DmPairCreated`, and `ConversationReadCursorUpdated` do not require `delivery_id`.

### `MessageCreated`

**When published**

- After a new `chat_message` row commits successfully.

**Minimum payload**

- `delivery_id`
- `message_id`
- `conversation_id`
- `target_type` with contract values `channel` or `dm`
- `workspace_id` when `target_type = channel`
- `workspace_channel_id` when `target_type = channel`
- `author_user_id`
- `conversation_message_seq`
- `body`
- `created_at`

**Typical consumers**

- `realtime` repair or catch-up fanout
- `bootstrap` channel projections and recent-message summaries for workspace-channel targets
- Audit or analytics workflows

### `ConversationCreated`

**When published**

- After a new `conversation` row commits successfully.

**Minimum payload**

- `conversation_id`
- `target_type`
- `dm_pair_id` when `target_type = dm`
- `workspace_channel_id` when `target_type = channel`
- `created_at`

**Typical consumers**

- `bootstrap` conversation-id mapping projections
- `realtime` or analytics workflows

### `DmPairCreated`

**When published**

- After a new normalized DM pair commits successfully.

**Minimum payload**

- `dm_pair_id`
- `low_user_id`
- `high_user_id`
- `created_at`

**Typical consumers**

- `bootstrap` DM-pair to conversation mapping projections

### `MessageEdited`

**When published**

- After a message edit commits successfully and the current message row has been updated.

**Minimum payload**

- `delivery_id`
- `message_id`
- `conversation_id`
- `target_type`
- `workspace_id` when `target_type = channel`
- `workspace_channel_id` when `target_type = channel`
- `editor_user_id`
- `body`
- `edited_at`

**Typical consumers**

- `realtime` repair or catch-up fanout
- `bootstrap` projected message updates

### `MessageDeleted`

**When published**

- After a message soft delete commits successfully.

**Minimum payload**

- `delivery_id`
- `message_id`
- `conversation_id`
- `target_type`
- `workspace_id` when `target_type = channel`
- `workspace_channel_id` when `target_type = channel`
- `deleted_by_user_id`
- `deleted_at`

**Typical consumers**

- `realtime` repair or catch-up fanout
- `bootstrap` projection cleanup or tombstoning

### `ConversationReadCursorUpdated`

**When published**

- After a read-cursor upsert advances stored read position for one `(user_id, conversation_id)` pair.

**Minimum payload**

- `conversation_id`
- `target_type`
- `workspace_channel_id` when `target_type = channel`
- `user_id`
- `last_read_conversation_message_seq`
- `read_at`

**Typical consumers**

- `bootstrap` unread and badge-count projection repair

## Event Rules

- Chat events use chat-owned `message_id` plus stable `conversation_id` target identity.
- `MessageCreated`, `MessageEdited`, and `MessageDeleted` carry stable outbox-derived `delivery_id` because those logical updates also feed `realtime` and need deduplication across direct and durable paths.
- `ConversationCreated`, `DmPairCreated`, and `ConversationReadCursorUpdated` do not require `delivery_id` in v1 because they are not part of `realtime` delivery reconciliation.
- `MessageCreated` is the durable signal that a message exists, regardless of whether the synchronous realtime notify succeeded.
- A duplicate retry resolved through `client_message_id` idempotency does not emit another `MessageCreated`; the original durable create remains the only create event.
- `ConversationCreated` is the durable signal that a new conversation stream exists.
- `DmPairCreated` is emitted only when chat creates a brand-new normalized DM pair.
- `ConversationReadCursorUpdated` is durable signal that chat-owned read position advanced for one actor and conversation.
- `MessageEdited` represents current message body after edit commits; chat does not expose append-only edit-history table in v1 contract.
- `MessageEdited` and `MessageDeleted` are emitted only when the actor is both the message author and currently authorized on the parent target.
- `MessageDeleted` is emitted for soft delete and should be treated downstream as a tombstone signal for current message visibility.
- A repeated delete against an already deleted message does not emit another `MessageDeleted`; downstream consumers keep the existing tombstone state.
- Publication ordering should be preserved per target so workspace-channel and DM consumers converge predictably.
- Consumers must be idempotent because replay and duplicate delivery are expected platform behaviors.
- RabbitMQ publication through the outbox worker is the durable backup and recovery path when realtime low-latency fanout is unavailable or delayed.
