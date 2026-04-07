## Purpose

Chat owns durable message writes and message-history state for both workspace channels and direct-message conversations in v1. It is the source of truth for messages, edits, deletes, reactions, and minimal DM conversation metadata when called by external application servers through Envoy Gateway, while using a direct synchronous notification to `realtime` only for low-latency message-create fanout after durable write success.

## Owned Responsibilities

- Create channel-scoped messages and assign durable per-channel ordering.
- Create and look up 1:1 direct-message conversations with stable `direct_conversation_id` values.
- Enforce one durable 1:1 conversation per unordered user pair using a canonical `pair_key`.
- Enforce retry-safe message creation when `client_message_id` is supplied.
- Persist message edits, soft deletes, and reaction state.
- Serve bounded message-history reads for channels and direct-message conversations.
- Insert matching `outbox_event` rows in the same transaction as message-domain writes.
- Notify `realtime` synchronously after successful durable message-create writes so connected clients can update with low latency.

## Non-Goals

- Owning workspace membership or workspace channel metadata; `workspace` owns both.
- Owning group direct messages or broader social graph state; v1 direct messages are 1:1 only.
- Making realtime fanout the durability authority; `realtime` is a delivery optimization, not the message source of truth.
- Owning aggregated UI projections such as unread counts or sidebar summaries; those belong to downstream projections such as `bootstrap`.
- Owning moderation, attachments, or media-processing systems in this v1 contract.

## Dependencies

- **external application server through Envoy Gateway** for authenticated send, edit, delete, history, and reaction commands routed to chat gRPC.
- **workspace** for synchronous channel and membership authorization checks where the chat write path must confirm actor access to the target channel.
- **identity** as the owner of stable `user_id` references used for direct-conversation participants and message actors.
- **realtime** for best-effort synchronous low-latency fanout after a durable message-create write succeeds.
- **RabbitMQ** for durable cold-path publication of chat events.
- **outbox worker sidecar** for polling local `outbox_event` rows and publishing them.
- **Postgres** as the service-owned source of truth for messages, edit history, and reactions.
- **bootstrap** and other downstream consumers for projection materialization from durable chat events.

## Storage

- Chat owns a dedicated Postgres database.
- Message-domain rows and matching `outbox_event` rows are written in the same local transaction.
- `chat_message.client_message_id` is nullable, but when present it must be unique per message target and author for retry-safe create semantics.
- `chat_message` targets either a workspace channel or a direct conversation, never both at once.
- `workspace_id`, `channel_id`, `direct_conversation_id`, and `user_id` values are service-owned or cross-service references, not foreign keys into another service database.
- Redis is not required by default for v1 chat behavior.

## gRPC Surface

- `CreateMessage`
- `EditMessage`
- `DeleteMessage`
- `ListChannelMessages`
- `GetOrCreateDirectConversation`
- `ListDirectConversationMessages`
- `AddReaction`
- `RemoveReaction`

See `grpc.md` for request, response, and write-path rules.

## Event Surface

- `MessageCreated`
- `MessageEdited`
- `MessageDeleted`
- `MessageReactionAdded`
- `MessageReactionRemoved`

See `events.md` for payload and publication rules.

## Realtime Coordination

- Chat remains the durable source of truth for message acceptance and persisted history.
- Workspace continues to own workspace membership and channel metadata, while chat owns direct-message conversation metadata and participant rows.
- After a message-create write commits successfully, chat may synchronously call `realtime` so connected clients can receive low-latency fanout.
- Edits, deletes, and reactions converge through durable chat events in v1 rather than a direct synchronous `chat -> realtime` hot path.
- Before accepting a write or history read, chat must validate channel existence and actor access against workspace-owned membership and channel state.
- Synchronous fanout is best-effort for latency only; a failed notify must not roll back or invalidate an already committed chat write.
- Durable recovery and downstream convergence come from `outbox_event` publication to RabbitMQ, not from the synchronous notify path.
