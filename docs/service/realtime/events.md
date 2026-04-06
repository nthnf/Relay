## Event Model

Realtime primarily consumes durable events published by source-of-truth services. Those events are the backup and recovery path when direct low-latency fanout was missed or delayed while a connection remained active. Full reconnect catch-up and history reload are not owned by realtime.

## Consumed Events

### From `chat`

#### `MessageCreated`

**Why consumed**

- Backup event path for workspace-channel and direct-message fanout.
- Recovery path when synchronous `PublishChannelMessage` or `PublishDirectMessage` did not reach all intended connected sessions.

**Minimum payload used by realtime**

- `event_id`
- `message_id`
- `target_kind`
- `workspace_id` when `target_kind = workspace_channel`
- `channel_id` when `target_kind = workspace_channel`
- `direct_conversation_id` when `target_kind = direct_message`
- `author_user_id`
- `target_message_seq`
- `body`
- `created_at`

**Handling rules**

- Realtime must support both workspace-channel and DM fanout from the same durable event family.
- Consumers must be idempotent because the direct gRPC path may already have delivered the update.
- Delivery from this event is allowed to arrive later than the direct gRPC fanout path.
- `event_id` is the logical dedupe key across direct gRPC and RabbitMQ paths.
- For create ordering, clients should treat `target_message_seq` as authoritative within one target.
- This path repairs short transient misses for active connections; it does not replace reconnect history reads.

#### `MessageEdited`

Consumed so connected clients converge edit state through the durable event path in v1. No separate direct hot-path edit fanout method is defined here.

#### `MessageDeleted`

Consumed so connected clients converge delete or tombstone state through the durable event path in v1. No separate direct hot-path delete fanout method is defined here.

#### `MessageReactionAdded`

Consumed so connected clients converge reaction-add state through the durable event path in v1. No separate direct hot-path reaction fanout method is defined here.

#### `MessageReactionRemoved`

Consumed so connected clients converge reaction-removal state through the durable event path in v1. No separate direct hot-path reaction fanout method is defined here.

### From `workspace`

#### `WorkspaceMemberAdded`

**Why consumed**

- Refresh connected actor subscriptions and allow newly relevant workspace sessions to observe future updates.
- Populate or permit newly authorized routing state convergence without making realtime the durable membership authority.

#### `WorkspaceMemberRemoved`

**Why consumed**

- Remove or disconnect sessions that should no longer receive workspace-channel fanout.
- Prune stale channel routes and subscription entries derived from last-known authorization.

#### `WorkspaceChannelCreated`

**Why consumed**

- Push connected-client sidebar or channel-list refresh signals.

## Published Events

### `PresenceChanged` (optional v1 integration event)

**When published**

- Only when Redis-backed user presence transitions between `online` and `offline`.

**Minimum payload**

- `user_id`
- `presence`
- `last_seen_at`
- `occurred_at`

**Typical consumers**

- `bootstrap` or notification-oriented consumers if later durable presence awareness is needed.

## Event Rules

- Durable chat and workspace events remain authoritative; realtime never republishes them as replacement message truth.
- The backup event path must treat channel and DM message-create delivery as equally supported v1 cases.
- In v1, the direct hot path is only `PublishChannelMessage` and `PublishDirectMessage` for message-create fanout.
- Edits, deletes, and reactions converge only through durable event consumption in v1.
- Consumers must be idempotent because a connected client may already have received the same update from direct gRPC fanout.
- Duplicate websocket deliveries are allowed in rare races; clients should dedupe by `event_id`.
- Ordering for message creates is authoritative by chat-assigned target-scoped sequence, not arrival order between direct and durable paths.
- Realtime may keep only short-lived repair behavior for active sessions; reconnect catch-up and historical reload are owned by chat/bootstrap read APIs.
- Presence publication, if enabled, is derived from Redis-backed state transitions and should remain lower priority than message repair.
- Presence events must describe online/offline state only; they do not make Redis a shared general-purpose store.
