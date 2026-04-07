## gRPC Service Scope

Realtime exposes hot-path delivery methods used after durable writes already succeed elsewhere, plus targeted session-control methods. `chat` is the primary latency-sensitive caller for message fanout.

## Shared Contract Rules

- Realtime is not the authority for whether a message, membership change, or DM exists; callers must invoke these methods only after the source-of-truth write commits.
- All methods are idempotent from the caller perspective: retries may result in duplicate connected-client attempts, but must not create durable source-state divergence.
- Realtime may drop delivery to disconnected recipients without failing the overall RPC when the source write is already durable.
- gRPC responses acknowledge acceptance for realtime delivery work, not durable message authority.
- Realtime routes to sessions from ephemeral per-node registries and in-memory target subscription maps populated during authenticated websocket attach/subscribe flows.
- Realtime uses last-known authorized subscription state and must prune stale routes quickly when `workspace`, `chat`, or explicit disconnect control flows indicate access changed.
- Duplicate websocket deliveries are allowed in rare failover races; delivery envelopes should carry `event_id` so clients can dedupe.
- For message creates, `target_message_seq` is the authoritative ordering field inside a single channel or direct conversation.
- Realtime only repairs short transient misses while a connection remains active; full reconnect catch-up belongs to chat/bootstrap read paths.

### `PublishChannelMessage`

**Main caller:** `chat`

**Latency-sensitive:** yes

**Request fields**

- `event_id` (`uuid/text`) - stable delivery identifier shared with any backup event path for the same logical update.
- `message_id` (`uuid`)
- `workspace_id` (`uuid`)
- `channel_id` (`uuid`)
- `author_user_id` (`uuid`)
- `target_message_seq` (`int64`)
- `body` (`string`)
- `created_at` (`timestamp`)

**Response fields**

- `accepted` (`bool`)
- `attempted_recipient_count` (`int32`)
- `delivered_session_count` (`int32`)

**Contract notes**

- Used only after chat has durably committed the channel message.
- Targets currently connected sessions that should already have access to the workspace channel through previously converged state.
- Best-effort delivery failure must not cause chat to roll back the committed message.
- RabbitMQ `MessageCreated` consumption remains the backup event path for replay or missed connected recipients.
- If the same logical update later arrives from RabbitMQ with the same `event_id`, realtime or the client may dedupe it.

### `PublishDirectMessage`

**Main caller:** `chat`

**Latency-sensitive:** yes

**Request fields**

- `event_id` (`uuid/text`) - stable delivery identifier shared with any backup event path for the same logical update.
- `message_id` (`uuid`)
- `direct_conversation_id` (`uuid`)
- `author_user_id` (`uuid`)
- `participant_user_ids` (`repeated uuid`) - expected to contain the DM participants for v1.
- `target_message_seq` (`int64`)
- `body` (`string`)
- `created_at` (`timestamp`)

**Response fields**

- `accepted` (`bool`)
- `attempted_recipient_count` (`int32`)
- `delivered_session_count` (`int32`)

**Contract notes**

- Direct-message fanout is a first-class v1 contract, not a secondary extension of channel fanout.
- Used only after chat has durably committed the DM message.
- Realtime fans out to connected sessions for the DM participants and may include the sending actor's other active sessions.
- RabbitMQ `MessageCreated` consumption remains the backup event path for any missed DM delivery.
- If the same logical update later arrives from RabbitMQ with the same `event_id`, realtime or the client may dedupe it.

### `PushWorkspaceEvent`

**Main caller:** `workspace` or a workspace-event bridge owned by `realtime`

**Latency-sensitive:** no

**Request fields**

- `event_type` (`string`) - v1 values include `WorkspaceMemberAdded`, `WorkspaceMemberRemoved`, `WorkspaceChannelCreated`.
- `workspace_id` (`uuid`)
- `channel_id` (`uuid optional`)
- `user_id` (`uuid optional`) - targeted member when relevant.
- `occurred_at` (`timestamp`)
- `payload` (`bytes` or structured message`) - event-specific fields.

**Response fields**

- `accepted` (`bool`)
- `affected_session_count` (`int32`)

**Contract notes**

- Used for connected-client refresh signals, sidebar updates, or access-change handling where websocket delivery helps UX.
- This method does not replace RabbitMQ consumption; durable workspace events remain the authority for replay and recovery.
- `WorkspaceMemberRemoved` handling may trigger immediate connected-session eviction or subscription cleanup for the affected actor.
- This method may also prune channel routing or subscription state derived from last-known authorization.

### `DisconnectActorSessions`

**Main caller:** `identity`, `workspace`, or an external application server control path through Envoy Gateway

**Latency-sensitive:** yes

**Request fields**

- `actor_user_id` (`uuid`)
- `reason_code` (`string`) - examples: `session_revoked`, `workspace_access_removed`, `account_disabled`.
- `disconnect_before` (`timestamp optional`) - disconnect sessions established before a cutoff when needed.

**Response fields**

- `accepted` (`bool`)
- `disconnected_session_count` (`int32`)

**Contract notes**

- Used when an upstream authority decides a user's active websocket sessions should no longer remain connected.
- Realtime clears matching Redis-backed session-presence entries as part of disconnect handling.
- A zero-session result is success when the actor is already offline.
- This method must also remove any matching in-memory routes or subscriptions for the disconnected sessions.
