## Purpose

Realtime owns websocket connection handling, low-latency fanout to connected clients, online/offline presence state, and selected connected-session control flows for external application callers routed through Envoy Gateway. It is a delivery service, not the durable authority for chat messages or workspace membership.

## Owned Responsibilities

- Accept and manage authenticated websocket sessions for connected clients.
- Fan out workspace-channel and direct-message updates to currently connected recipients with minimal latency.
- Keep ephemeral per-node connection/session registries plus subscription maps for channel and direct-conversation targets.
- Track v1 online/offline presence in Redis for currently connected users.
- Consume durable chat and workspace events from RabbitMQ for replay, repair, and catch-up delivery behavior.
- Evict or disconnect active sessions when downstream authority changes require it.

## Non-Goals

- Owning durable message persistence or message acceptance; `chat` owns both.
- Owning workspace membership, channel metadata, or authorization truth; `workspace` owns them.
- Replacing `bootstrap` as the canonical UI-facing aggregate read service.
- Treating websocket delivery success as a prerequisite for durable message success.
- Introducing a broader stream-processing or analytics system in v1.

## Boundary Notes

- Envoy Gateway handles backend ingress policy.
- Realtime retains service-owned session/control and delivery authorization responsibility at its own boundary.

## Dependencies

- **external application server through Envoy Gateway** for authenticated websocket upgrade routing and session context forwarding.
- **chat** for synchronous low-latency `PublishChannelMessage` and `PublishDirectMessage` fanout calls after durable writes commit.
- **workspace** as the owner of durable membership and channel-change events consumed for connected-client refresh and session eviction.
- **identity** as the owner of stable `user_id` references used for actor/session targeting.
- **RabbitMQ** for durable backup and recovery inputs when direct fanout is unavailable or delayed.
- **Redis** as the primary v1 store for online/offline presence state and lightweight session-presence coordination.
- **Postgres** for any minimal service-owned durable recovery or operational cursors.

## Storage

- Realtime may own a dedicated Postgres database, but v1 durable ownership stays intentionally minimal.
- Routing and subscription state is ephemeral by default: per-node websocket session registry in memory plus in-memory target subscription maps.
- Subscription state is populated during websocket attach and subsequent subscribe flows after authenticated session context is established.
- Realtime uses last-known authorized subscription state for low-latency delivery, then converges quickly when `workspace` or `chat` ownership changes arrive through direct control calls or durable events.
- Redis is the primary v1 state store for presence because presence is ephemeral and online/offline-oriented.
- Message payloads, membership authority, and channel metadata are never durably owned here.
- Durable backup and recovery behavior may keep small operational cursor rows in Postgres without making realtime a system-of-record for chat state.

## gRPC Surface

- `PublishChannelMessage`
- `PublishDirectMessage`
- `PushWorkspaceEvent`
- `DisconnectActorSessions`

See `grpc.md` for request, response, caller, and latency rules.

## Event Surface

- Consumes durable chat message events for backup and recovery fanout.
- Consumes selected workspace membership and channel events for connected-client refresh, access cleanup, and eviction.
- May publish presence-change events only for online/offline state transitions if another service later needs durable awareness.
- Does not own full reconnect history replay; reconnect catch-up and historical reload remain on `chat` or `bootstrap` read paths.

See `events.md` for payload and handling rules.

## Latency Model

- `chat -> realtime` gRPC fanout for channel messages and direct messages is the low-latency path for already committed durable writes.
- The synchronous gRPC path is best-effort for delivery speed only; a realtime failure must not invalidate a committed chat write.
- RabbitMQ event consumption is the backup and recovery path for missed channel or DM fanout while a connection remains active, plus downstream convergence after transient direct-path failure.
- Rare duplicate websocket deliveries are allowed during failover or race conditions; clients should dedupe using `event_id`.
- Chat-assigned target-scoped sequence remains authoritative for message-create ordering inside one channel or one direct conversation.
- Realtime does not promise full per-session replay after reconnect; once a client reconnects, full catch-up belongs to chat/bootstrap history reads.
- Presence is lower urgency than message delivery correctness: Redis-backed online/offline updates may converge with slightly more delay than message fanout.
