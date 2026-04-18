## Publication Model

Chat publishes integration events by inserting service-owned rows into `outbox_event` inside the same transaction as the source message write. The shared outbox worker later publishes those rows to RabbitMQ. The same event family covers both workspace-channel messages and direct-message conversation messages in v1.

## Published Events

All published chat events must carry `event_id` as required event metadata. The same `event_id` must be stored in the source `outbox_event` row and surfaced by the outbox worker in the broker delivery envelope or headers so downstream consumers such as `realtime` can deduplicate retries and race conditions.

### `MessageCreated`

**When published**

- After a new `chat_message` row commits successfully.

**Minimum payload**

- `event_id`
- `message_id`
- `target_kind` with contract values `workspace_channel` or `direct_message`
- `workspace_id` when `target_kind = workspace_channel`
- `channel_id` when `target_kind = workspace_channel`
- `direct_conversation_id` when `target_kind = direct_message`
- `author_user_id`
- `target_message_seq`
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

- `event_id`
- `message_id`
- `target_kind`
- `workspace_id` when `target_kind = workspace_channel`
- `channel_id` when `target_kind = workspace_channel`
- `direct_conversation_id` when `target_kind = direct_message`
- `editor_user_id`
- `edit_version`
- `body`
- `edited_at`

**Typical consumers**

- `realtime` repair or catch-up fanout
- `bootstrap` projected message updates

### `MessageDeleted`

**When published**

- After a message soft delete commits successfully.

**Minimum payload**

- `event_id`
- `message_id`
- `target_kind`
- `workspace_id` when `target_kind = workspace_channel`
- `channel_id` when `target_kind = workspace_channel`
- `direct_conversation_id` when `target_kind = direct_message`
- `deleted_by_user_id`
- `deleted_at`

**Typical consumers**

- `realtime` repair or catch-up fanout
- `bootstrap` projection cleanup or tombstoning

## Event Rules

- Chat events use chat-owned `message_id` values plus target context identifying either a workspace channel or a direct conversation.
- Every published event must carry the stable outbox-derived `event_id` used for RabbitMQ deduplication and direct-versus-durable delivery reconciliation.
- `MessageCreated` is the durable signal that a message exists, regardless of whether the synchronous realtime notify succeeded.
- A duplicate retry resolved through `client_message_id` idempotency does not emit another `MessageCreated`; the original durable create remains the only create event.
- `MessageEdited` represents the current message body after the edit commits; historical diffs stay in `chat_message_edit` and need not be fully embedded in every event.
- `MessageEdited` and `MessageDeleted` are emitted only when the actor is both the message author and currently authorized on the parent target.
- `MessageDeleted` is emitted for soft delete and should be treated downstream as a tombstone signal for current message visibility.
- A repeated delete against an already deleted message does not emit another `MessageDeleted`; downstream consumers keep the existing tombstone state.
- Publication ordering should be preserved per target so workspace-channel and DM consumers converge predictably.
- Consumers must be idempotent because replay and duplicate delivery are expected platform behaviors.
- RabbitMQ publication through the outbox worker is the durable backup and recovery path when realtime low-latency fanout is unavailable or delayed.
