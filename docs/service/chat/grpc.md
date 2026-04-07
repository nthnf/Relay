## gRPC Service Scope

Chat exposes hot-path message-write commands, bounded channel and direct-message history reads, reaction mutations, and minimal 1:1 DM conversation lifecycle reads/writes. External application servers through Envoy Gateway are the primary callers for end-user send, edit, delete, history, reaction, and DM-open flows.

## Shared Contract Rules

- Authenticated actor identity is derived from ingress-authenticated request context and must still be authorized by the chat service boundary; callers must not mutate another actor's message state by supplying arbitrary user IDs.
- External application callers do not supply actor identity in request payloads for end-user actions; the transport boundary or a trusted backend caller context attaches it out-of-band.
- Chat enforces chat-local invariants and must validate channel access against `workspace` before accepting workspace-channel writes or history reads.
- Chat authorizes direct-message writes and reads through `direct_conversation_member` rows it owns.
- Domain writes and matching `outbox_event` inserts happen in the same transaction.
- Chat remains the durable message-write authority; synchronous `realtime` notify calls for message-create fanout happen only after durable write success.
- A `realtime` notify failure must not roll back an already committed message-create write.
- V1 direct messages are 1:1 only; group DMs are not defined here.

### `CreateMessage`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `workspace_id` (`uuid optional`) - required when targeting a workspace channel.
- `channel_id` (`uuid optional`) - required when targeting a workspace channel.
- `direct_conversation_id` (`uuid optional`) - required when targeting a DM conversation.
- `body` (`string`)
- `client_message_id` (`string optional`) - caller-generated idempotency token for retry-safe sends.

**Response fields**

- `message_id` (`uuid`)
- `workspace_id` (`uuid optional`)
- `channel_id` (`uuid optional`)
- `direct_conversation_id` (`uuid optional`)
- `author_user_id` (`uuid`)
- `target_message_seq` (`int64`)
- `body` (`string`)
- `created_at` (`timestamp`)

**Contract notes**

- The request must target exactly one message destination: either `(workspace_id, channel_id)` or `direct_conversation_id`.
- Validate workspace-channel targets using workspace-owned membership and channel metadata.
- Validate DM targets by confirming the authenticated actor is an active `direct_conversation_member` of `direct_conversation_id`.
- If `client_message_id` is present, treat `(authenticated actor, channel_id, client_message_id)` or `(authenticated actor, direct_conversation_id, client_message_id)` as the idempotency key for the chosen target.
- Insert the `chat_message` row and matching `MessageCreated` outbox row in one transaction.
- Assign the next `target_message_seq` unique within the chosen target as part of the same durable write.
- Store `client_message_id` on `chat_message` when supplied.
- On duplicate retry for an existing durable message with the same idempotency key, return the original message response and do not create another row.
- Duplicate retry must not publish another `MessageCreated` event.
- After the transaction commits successfully, synchronously call `realtime` as a downstream side effect for low-latency fanout.
- The synchronous callout is best-effort: chat returns durable write success even if the post-commit notify fails, with RabbitMQ plus `outbox_event` remaining the recovery path.

### `EditMessage`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `message_id` (`uuid`)
- `new_body` (`string`)

**Response fields**

- `message_id` (`uuid`)
- `channel_id` (`uuid optional`)
- `workspace_id` (`uuid optional`)
- `direct_conversation_id` (`uuid optional`)
- `body` (`string`)
- `edit_version` (`int32`)
- `edited_at` (`timestamp`)

**Contract notes**

- Allow only the message author or a later explicitly documented privileged actor path; v1 assumes author edits only unless superseded.
- Edit authorization requires author ownership and current access to the parent target.
- For workspace-channel messages, current workspace membership and channel access are required at edit time.
- For direct messages, current `direct_conversation_member` membership is required at edit time.
- Reject edits for deleted messages.
- Insert a `chat_message_edit` row, update the current `chat_message` row, and insert `MessageEdited` into `outbox_event` in one transaction.
- Edits converge through durable event publication only in v1; there is no direct synchronous `chat -> realtime` edit fanout contract.

### `DeleteMessage`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `message_id` (`uuid`)

**Response fields**

- `message_id` (`uuid`)
- `channel_id` (`uuid optional`)
- `workspace_id` (`uuid optional`)
- `direct_conversation_id` (`uuid optional`)
- `deleted_at` (`timestamp`)
- `deleted` (`bool`)
- `already_deleted` (`bool`)

**Contract notes**

- Use soft delete by updating `message_status`, `deleted_at`, and `deleted_by_user_id` on `chat_message`.
- Delete authorization requires author ownership and current access to the parent target.
- For workspace-channel messages, current workspace membership and channel access are required at delete time.
- For direct messages, current `direct_conversation_member` membership is required at delete time.
- No privileged moderator or admin delete path is defined yet.
- This method is idempotent: repeated deletes return `deleted = true`, `already_deleted = true`, and the existing tombstone state.
- Insert a matching `MessageDeleted` outbox row in the same transaction as the soft delete.
- Repeated deletes must not publish another `MessageDeleted` event.
- Deletes converge through durable event publication only in v1; there is no direct synchronous `chat -> realtime` delete fanout contract.

### `ListChannelMessages`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `workspace_id` (`uuid`)
- `channel_id` (`uuid`)
- `page_size` (`int32 optional`)
- `before_target_message_seq` (`int64 optional`)

**Response fields**

- `messages` (`repeated message`) with `message_id`, `author_user_id`, `target_message_seq`, `body`, `message_status`, `created_at`, `last_edited_at`, `deleted_at`
- `next_before_target_message_seq` (`int64 optional`)

**Contract notes**

- Validate the authenticated actor can read the target channel through workspace-owned membership state.
- Order results by `target_message_seq` descending for recent-history pagination unless a later doc revision defines otherwise.
- Use `target_message_seq` as the pagination cursor because it is channel-scoped, monotonic, and stable under edits.
- Reads return current message state; detailed edit history is internal unless later exposed explicitly.

### `GetOrCreateDirectConversation`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `peer_user_id` (`uuid`)

**Response fields**

- `direct_conversation_id` (`uuid`)
- `member_user_ids` (`repeated uuid`) - exactly the actor and peer user IDs.
- `created_at` (`timestamp`)
- `created` (`bool`) - true only when this call created the durable conversation.

**Contract notes**

- V1 supports 1:1 DMs only; reject attempts to model more than two participants.
- Reject self-DM creation in v1 unless later documented; the authenticated actor and `peer_user_id` must differ.
- Return the existing durable conversation when one already exists for the unordered participant pair.
- Derive a canonical `pair_key` from the sorted `(authenticated actor, peer_user_id)` values and use it as the lookup/upsert key.
- When no conversation exists, create one `direct_conversation` row plus two `direct_conversation_member` rows in one transaction.
- Race-safe create semantics are defined by the unique `pair_key`: concurrent calls for the same participant pair must return the same durable conversation.
- This method does not publish a dedicated conversation-created event in v1; the conversation exists so subsequent DM messages have a stable target.

### `ListDirectConversationMessages`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `direct_conversation_id` (`uuid`)
- `page_size` (`int32 optional`)
- `before_target_message_seq` (`int64 optional`)

**Response fields**

- `messages` (`repeated message`) with `message_id`, `author_user_id`, `target_message_seq`, `body`, `message_status`, `created_at`, `last_edited_at`, `deleted_at`
- `next_before_target_message_seq` (`int64 optional`)

**Contract notes**

- Validate the authenticated actor is an active member of the direct conversation.
- Order results by `target_message_seq` descending for recent-history pagination unless a later doc revision defines otherwise.
- Use `target_message_seq` as the pagination cursor because it is conversation-scoped, monotonic, and stable under edits.
- Reads return current message state; detailed edit history is internal unless later exposed explicitly.

### `AddReaction`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `message_id` (`uuid`)
- `reaction_key` (`string`)

**Response fields**

- `message_id` (`uuid`)
- `reaction_key` (`string`)
- `user_id` (`uuid`)
- `created_at` (`timestamp`)

**Contract notes**

- Require the authenticated actor to have access to the parent message target.
- The parent message may belong to either a workspace channel or a direct conversation; chat must authorize reaction changes against the owning target.
- Insert or reactivate the unique `(message_id, reaction_key, user_id)` row and insert `MessageReactionAdded` into `outbox_event` in one transaction.
- This method is idempotent for an already active identical reaction.
- Reaction adds converge through durable event publication only in v1; there is no direct synchronous `chat -> realtime` reaction fanout contract.

### `RemoveReaction`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `message_id` (`uuid`)
- `reaction_key` (`string`)

**Response fields**

- `message_id` (`uuid`)
- `reaction_key` (`string`)
- `user_id` (`uuid`)
- `removed` (`bool`)
- `removed_at` (`timestamp optional`)

**Contract notes**

- Remove or deactivate only the authenticated actor's own matching reaction unless a later privileged moderation path is documented.
- The parent message may belong to either a workspace channel or a direct conversation; chat must authorize reaction changes against the owning target.
- This method is idempotent: if no active matching reaction exists, return `removed = false`.
- Persist the reaction removal and matching `MessageReactionRemoved` outbox row in one transaction.
- Reaction removals converge through durable event publication only in v1; there is no direct synchronous `chat -> realtime` reaction fanout contract.
