## gRPC Service Scope

Chat exposes hot-path message-write commands, bounded conversation history reads, and minimal 1:1 DM conversation lifecycle reads and writes. External application servers through Envoy Gateway are primary callers for end-user send, edit, delete, history, explicit DM-create or DM-open flows, and one-time channel conversation creation after channel creation succeeds.

## Shared Contract Rules

- Authenticated actor identity is derived from Envoy-validated access-token context and still authorized by chat boundary.
- External application callers do not supply actor identity in request payloads for end-user actions.
- Chat enforces chat-local invariants and uses synchronous `workspace.AuthorizeChannelAction` for workspace-channel authorization.
- Chat authorizes DMs through `conversation.dm_pair_id` and owned `dm_pair` rows.
- Domain writes and matching `outbox_event` inserts happen in same transaction.
- Chat remains durable message-write authority; synchronous `realtime.DeliverMessage` happens only after durable write success.
- `realtime` notify failure must not roll back committed message-create write.
- V1 DMs are 1:1 only.

## Authorization Split

- `conversation.target_type = channel`
  - `workspace` is authority for channel read and write checks.
  - Chat calls `workspace.AuthorizeChannelAction` synchronously with `action = READ` or `WRITE`.
  - Chat should reject if matching `workspace_snapshot` or `workspace_channel_snapshot` row does not exist locally.
- `conversation.target_type = dm`
  - Chat is authority through `conversation.dm_pair_id` and `dm_pair`.
  - Chat should reject if `peer_user_id` has no local `user_snapshot` row.
- Message delete in v1 is author-owned inside chat.
  - Channel delete permission does not grant deleting another member's message.
  - Author delete still requires actor to retain access to parent conversation.

### `CreateMessage`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `conversation_id` (`uuid`)
- `body` (`string`)
- `client_message_id` (`string optional`)

**Response fields**

- `message_id` (`uuid`)
- `conversation_id` (`uuid`)
- `author_user_id` (`uuid`)
- `conversation_message_seq` (`int64`)
- `body` (`string`)
- `created_at` (`timestamp`)

**Contract notes**

- Validate that `conversation_id` exists.
- If target type is `workspace_channel`, call `workspace.AuthorizeChannelAction(..., WRITE)` synchronously.
- If target type is `dm`, confirm actor matches either `low_user_id` or `high_user_id` on the `dm_pair` row.
- When `client_message_id` is present, use `(authenticated actor, conversation_id, client_message_id)` as idempotency key.
- Insert `chat_message` row and matching `MessageCreated` outbox row in one transaction.
- Assign next `conversation_message_seq` unique within conversation.
- On duplicate retry, return original durable message response and do not publish another create event.
- After commit, synchronously call `realtime.DeliverMessage` for low-latency fanout.

### `EditMessage`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `message_id` (`uuid`)
- `new_body` (`string`)

**Response fields**

- `message_id` (`uuid`)
- `conversation_id` (`uuid`)
- `body` (`string`)
- `edit_version` (`int32`)
- `edited_at` (`timestamp`)

**Contract notes**

- Allow only message author in v1.
- For workspace-channel messages, actor must still pass `workspace.AuthorizeChannelAction(..., WRITE)`.
- For DMs, actor must still match either `low_user_id` or `high_user_id` on the `dm_pair` row.
- Reject edits for deleted messages.
- Update current `chat_message` and insert `MessageEdited` outbox row in one transaction.

### `DeleteMessage`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `message_id` (`uuid`)

**Response fields**

- `message_id` (`uuid`)
- `conversation_id` (`uuid`)
- `deleted_at` (`timestamp`)
- `deleted` (`bool`)
- `already_deleted` (`bool`)

**Contract notes**

- Use soft delete by updating `message_status`, `deleted_at`, and `deleted_by_user_id` on `chat_message`.
- Allow only message author in v1.
- For workspace-channel messages, require actor still passes `workspace.AuthorizeChannelAction(..., READ)` so removed members cannot mutate old messages.
- For DMs, require actor still matches either participant on the `dm_pair` row.
- Repeated deletes are idempotent and emit no duplicate delete event.

### `ListConversationMessages`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `conversation_id` (`uuid`)
- `page_size` (`int32 optional`)
- `before_conversation_message_seq` (`int64 optional`)

**Response fields**

- `messages` (`repeated message`) with `message_id`, `author_user_id`, `conversation_message_seq`, `body`, `message_status`, `created_at`, `last_edited_at`, `deleted_at`
- `next_before_conversation_message_seq` (`int64 optional`)

**Contract notes**

- For workspace-channel conversations, call `workspace.AuthorizeChannelAction(..., READ)` synchronously.
- For DMs, validate actor matches either participant on the `dm_pair` row.
- Order by `conversation_message_seq` descending for recent-history pagination.
- Reads return current message state; detailed edit history remains internal.

### `CreateConversation`

**Main caller:** external application server through Envoy Gateway

**Typical invocation patterns**

- `DM`: user presses a dedicated DM-create or DM-open action; the caller invokes `CreateConversation` once for that participant pair and reuses the returned `conversation_id` afterward.
- `WORKSPACE_CHANNEL`: after a successful channel-creation flow, the caller invokes `CreateConversation` once for the new channel so the channel has a stable chat-owned `conversation_id` before normal messaging starts.

**Request fields**

- `target_type` (`enum`) - `DM` or `CHANNEL`
- `peer_user_id` (`uuid optional`) - required for `DM`
- `workspace_channel_id` (`uuid optional`) - required for `WORKSPACE_CHANNEL`

**Response fields**

- `conversation_id` (`uuid`)
- `member_user_ids` (`repeated uuid`) - exactly actor and peer user IDs
- `created_at` (`timestamp`)
- `created` (`bool`)

**Contract notes**

- For `DM`, reject self-DM creation in v1.
- For `DM`, this method is the explicit create-or-open entry point behind the user-facing DM action; repeated calls must return the existing conversation for the same pair.
- For `DM`, require local `user_snapshot` row for `peer_user_id` before creating or returning conversation.
- For `DM`, normalize `(authenticated actor, peer_user_id)` into `(low_user_id, high_user_id)` by comparing raw UUID values.
- For `DM`, return existing durable conversation when one already exists for normalized participant pair.
- For `DM`, lookup existing 1:1 conversation by `(low_user_id, high_user_id)` before insert so duplicate DM conversations are not created.
- For `CHANNEL`, require matching `workspace_snapshot` and `workspace_channel_snapshot` rows first.
- For `CHANNEL`, this method is typically called once immediately after channel creation succeeds; it is not primarily a lazy first-message setup path.
- For `CHANNEL`, call `workspace.AuthorizeChannelAction(..., READ)` or `WRITE` as needed before creating or returning conversation.
- For `CHANNEL`, return existing durable conversation when one already exists for channel.
- For `CHANNEL`, keep `conversation_id` distinct from `workspace_channel_id`; channel identity stays workspace-owned while message-stream identity stays chat-owned.
- When no conversation exists, create one `conversation` row with requested target type in one transaction.
- For `DM`, create one `dm_pair` row in the same transaction as the `conversation` row.
- For `DM`, store resulting pair reference on `conversation.dm_pair_id` in same transaction.
- This method does not publish dedicated conversation-created event in v1.
