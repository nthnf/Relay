## State Scope

Realtime owns ephemeral connection-routing and presence state, plus only the minimum durable operational state needed to resume backup or recovery workflows. It does not own durable messages, workspace membership, or channel metadata.

## Durable vs Ephemeral State

- **Ephemeral:** websocket connections, recipient routing maps, target subscription maps, and current online/offline presence.
- **Durable-minimal:** none in v1; keep realtime stateless beyond Redis presence and in-memory routing.
- **Not owned here:** message bodies as authoritative records, membership truth, DM conversation truth, or UI aggregate projections.

## In-Memory Routing State

Each realtime node keeps a local in-memory registry for currently connected sessions.

### `connection_session_registry`

Ephemeral per-node map of active websocket sessions.

| Field | Type | Notes |
| --- | --- | --- |
| `session_id` | `uuid/text` | Realtime-owned connection identifier. |
| `actor_user_id` | `uuid` | Authenticated user for the session. |
| `connection_node` | `text` | Current node/process identifier. |
| `connected_at` | `timestamp` | Session establish time. |
| `last_activity_at` | `timestamp` | Last observed heartbeat or client activity. |

Semantic rules:

- This registry is node-local and ephemeral.
- A reconnect may land on a different node and create a different `session_id`.
- The registry is populated during websocket attach after ingress authentication succeeds and realtime accepts the connection.

### `target_subscription_map`

Ephemeral in-memory routing map from target to subscribed sessions.

| Field | Type | Notes |
| --- | --- | --- |
| `target_kind` | `text` | `workspace_channel` or `direct_message`. |
| `target_id` | `uuid` | `channel_id` for channels or `direct_conversation_id` for DMs. |
| `session_id` | `uuid/text` | Connected session currently routed for the target. |
| `actor_user_id` | `uuid` | Session owner used for pruning and authorization convergence. |
| `subscribed_at` | `timestamp` | When the routing entry was created. |

Semantic rules:

- Subscription state is populated during websocket subscribe or attach flows after auth.
- Realtime does not invent durable authorization; it routes based on the last-known authorized subscription state.
- Workspace membership changes, DM participation changes, and disconnect control flows must prune stale routing entries quickly.
- Short windows of stale subscription state are allowed during convergence, so duplicate or briefly mis-targeted attempts must be prevented by rapid pruning rather than durable authorization ownership inside realtime.

## Redis State

Redis is the primary v1 store for presence because online/offline state is latency-sensitive, short-lived, and naturally ephemeral.

### `presence_state:{user_id}`

Current user presence summary.

| Field | Type | Notes |
| --- | --- | --- |
| `user_id` | `uuid` | Identity-owned user reference and key suffix. |
| `presence` | `text` | Contract values: `online`, `offline`. |
| `last_seen_at` | `timestamp` | Last server-observed activity or disconnect time. |
| `session_count` | `int` | Count of currently connected realtime sessions for the user. |
| `updated_at` | `timestamp` | Last presence-state write time. |

Semantic rules:

- A user is `online` when `session_count > 0`.
- A user is `offline` when `session_count = 0`.
- `last_seen_at` advances on heartbeat, message-acknowledged activity, or disconnect handling, whichever realtime observes last.
- Presence is user-scoped, not workspace-scoped, in v1.
- Redis key owner is `realtime`; no other service writes these keys directly.

### `presence_sessions:{user_id}`

Set or hash of active session identifiers for one user.

| Field | Type | Notes |
| --- | --- | --- |
| `session_id` | `uuid/text` | Realtime-owned connected-session identifier. |
| `connected_at` | `timestamp` | Session establish time. |
| `connection_node` | `text` | Optional pod/process identifier for disconnect routing. |

Semantic rules:

- This structure is the source for `session_count` derivation and disconnect fanout targeting.
- Entries are removed on explicit disconnect or expiry cleanup.
- TTLs must be refreshed by heartbeat so abandoned sessions age out if a node dies.

### Redis Policy Notes

- **Owner:** `realtime`
- **Keys:** `presence_state:{user_id}`, `presence_sessions:{user_id}`
- **TTL:** session entries should expire without heartbeat; the effective offline transition happens when the final active session entry disappears.
- **Invalidation:** explicit disconnect removes the session entry immediately; crash recovery relies on TTL expiry.
- **Fallback:** if Redis is unavailable, existing websocket delivery may continue best-effort in-memory on a single node, but shared presence accuracy degrades and cross-node presence should be treated as temporarily unavailable.

## Delivery Semantics State

- Realtime may observe the same logical update from both direct gRPC fanout and RabbitMQ backup delivery.
- Duplicate websocket deliveries are allowed in rare races; the logical delivery key is `delivery_id`.
- For message creates, ordering is authoritative by chat-assigned `target_message_seq` inside one channel or one direct conversation.
- Realtime does not own full reconnect catch-up state; durable history reload after reconnect belongs to chat/bootstrap read paths.

## Cross-Service References

- `identity` owns `user_id` values used in Redis presence keys and targeted disconnect calls.
- `chat` owns message IDs, channel/DM target context, and durable message ordering.
- `workspace` owns `workspace_id`, `channel_id`, and membership truth used by consumed workspace events.
- Realtime never reads another service's database directly.
