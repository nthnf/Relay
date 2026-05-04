## gRPC Service Scope

Realtime exposes hot-path delivery methods used after durable writes already succeed elsewhere, plus targeted session-control methods. `chat` is the primary latency-sensitive caller for message fanout.

## Shared Contract Rules

- Realtime is not the authority for whether a message, membership change, or DM exists; callers must invoke these methods only after the source-of-truth write commits.
- All methods are idempotent from the caller perspective: retries may result in duplicate connected-client attempts, but must not create durable source-state divergence.
- Realtime may drop delivery to disconnected recipients without failing the overall RPC when the source write is already durable.
- gRPC responses acknowledge acceptance for realtime delivery work, not durable message authority.
- Realtime routes to sessions from ephemeral per-node registries and in-memory target subscription maps populated during authenticated websocket attach/subscribe flows.
- Realtime uses last-known authorized subscription state and must prune stale routes quickly when `workspace`, `chat`, or explicit disconnect control flows indicate access changed.
- Duplicate websocket deliveries are allowed in rare failover races; delivery envelopes should carry `delivery_id` so clients can dedupe.
- For message creates, `target_message_seq` remains authoritative inside one channel or direct conversation and should be carried in `payload`.
- Realtime only repairs short transient misses while a connection remains active; full reconnect catch-up belongs to chat/bootstrap read paths.
- Direct gRPC fanout is the hot path; RabbitMQ event consumption remains repair and backup.

### `DeliverMessage`

**Main caller:** `chat` or `workspace`

**Latency-sensitive:** yes

**Request fields**

- `delivery_id` (`uuid/text`) - stable delivery identifier shared with any backup event path for the same logical update.
- `target_kind` (`string`) - e.g. `workspace_channel`, `direct_message`, `workspace_user`.
- `target_id` (`string`) - channel id, direct conversation id, or user id depending on target kind.
- `payload` (`oneof`) - one of `message_created`, `message_edited`, or `message_deleted`.
- `occurred_at` (`timestamp`)

**Response fields**

- `accepted` (`bool`)
- `attempted_recipient_count` (`int32`)
- `delivered_session_count` (`int32`)

**Contract notes**

- Used only after chat has durably committed the message.
- Targets currently connected sessions that should already have access to the target through previously converged state.
- Best-effort delivery failure must not cause chat to roll back the committed message.
- RabbitMQ `MessageCreated` consumption remains the backup event path for replay or missed connected recipients.
- If the same logical update later arrives from RabbitMQ with the same `delivery_id`, realtime or the client may dedupe it.

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

### `GetUserPresence`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `user_ids` (`repeated uuid`) - users whose current presence should be returned.

**Response fields**

- `users` (`repeated message`) with `user_id`, `online`, `last_seen_at`

**Contract notes**

- Returns a bounded user-scoped presence snapshot from realtime-owned Redis state.
- Missing or expired Redis presence state returns `online = false`.
- The response intentionally does not expose connected session counts; presence is a 0/1 online state for callers.
- This is for initial page-load and reconnect snapshots; websocket updates remain the live-change path.
- A zero-session result is success when the actor is already offline.
- This method must also remove any matching in-memory routes or subscriptions for the disconnected sessions.
