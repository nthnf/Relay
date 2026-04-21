## Publication Model

Chat publishes integration events by inserting service-owned rows into `outbox_event` inside same transaction as source message write. Shared outbox worker later publishes those rows to RabbitMQ. Same event family covers both workspace-channel and DM conversation messages in v1.

## Published Events

All published chat events must carry `delivery_id` as required delivery metadata. Same `delivery_id` must be stored in source `outbox_event` row and surfaced by outbox worker in broker delivery envelope or headers so downstream consumers such as `realtime` can deduplicate retries and race conditions.

### `MessageCreated`

**When published**

- After a new `chat_message` row commits successfully.

**Minimum payload**

- `delivery_id`
- `message_id`
- `conversation_id`
- `target_type` with contract values `workspace_channel` or `dm`
- `workspace_id` when `target_type = workspace_channel`
- `workspace_channel_id` when `target_type = workspace_channel`
- `author_user_id`
- `conversation_message_seq`
- `body`
- `created_at`

**Typical consumers**

- `realtime` repair or catch-up fanout
- `bootstrap` channel projections and recent-message summaries for workspace-channel targets
- Audit or analytics workflows

### `MessageEdited`

**When published**

- After a message edit commits successfully and the current message row has been updated.

**Minimum payload**

- `delivery_id`
- `message_id`
- `conversation_id`
- `target_type`
- `workspace_id` when `target_type = workspace_channel`
- `workspace_channel_id` when `target_type = workspace_channel`
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
- `workspace_id` when `target_type = workspace_channel`
- `workspace_channel_id` when `target_type = workspace_channel`
- `deleted_by_user_id`
- `deleted_at`

**Typical consumers**

- `realtime` repair or catch-up fanout
- `bootstrap` projection cleanup or tombstoning

## Event Rules

- Chat events use chat-owned `message_id` plus stable `conversation_id` target identity.
- Every published event must carry stable outbox-derived `delivery_id` used for RabbitMQ deduplication and direct-versus-durable delivery reconciliation.
- `MessageCreated` is the durable signal that a message exists, regardless of whether the synchronous realtime notify succeeded.
- A duplicate retry resolved through `client_message_id` idempotency does not emit another `MessageCreated`; the original durable create remains the only create event.
- `MessageEdited` represents current message body after edit commits; chat does not expose append-only edit-history table in v1 contract.
- `MessageEdited` and `MessageDeleted` are emitted only when the actor is both the message author and currently authorized on the parent target.
- `MessageDeleted` is emitted for soft delete and should be treated downstream as a tombstone signal for current message visibility.
- A repeated delete against an already deleted message does not emit another `MessageDeleted`; downstream consumers keep the existing tombstone state.
- Publication ordering should be preserved per target so workspace-channel and DM consumers converge predictably.
- Consumers must be idempotent because replay and duplicate delivery are expected platform behaviors.
- RabbitMQ publication through the outbox worker is the durable backup and recovery path when realtime low-latency fanout is unavailable or delayed.
