## Purpose

Chat owns durable message writes and message-history state for both workspace channels and direct-message conversations in v1. It is source of truth for messages, current edit state, deletes, and durable conversation identity when called by external application servers through Envoy Gateway, while using direct synchronous notification to `realtime` only for low-latency message-create fanout after durable write success.

## Owned Responsibilities

- Create conversation-scoped messages and assign durable per-conversation ordering.
- Create 1:1 direct-message conversations from explicit DM-create action, with stable `conversation_id` values and one normalized DM pair row referenced by `conversation.dm_pair_id`.
- Create workspace-channel conversation rows once after successful channel creation, with stable chat-owned `conversation_id` values distinct from `workspace_channel_id`.
- Enforce one durable 1:1 conversation per unordered user pair through canonical `(low_user_id, high_user_id)` ordering.
- Own per-user per-conversation read cursor writes used for unread convergence.
- Enforce retry-safe message creation when `client_message_id` is supplied.
- Persist message edits and soft deletes.
- Serve bounded message-history reads for conversations.
- Insert matching `outbox_event` rows in the same transaction as message-domain writes.
- Call `realtime.DeliverMessage` synchronously after successful durable message-create writes so connected clients can update with low latency.

## App Open Flow

- Frontend opens websocket to `realtime` for live delivery only.
- Frontend fetches history and sidebar data from `chat`, `workspace`, or `bootstrap`.
- Creating a workspace channel typically triggers two backend calls in sequence: create the channel in `workspace`, then create the matching conversation in `chat`.
- Creating a DM comes from explicit user action such as New DM or message-user button that calls `chat.CreateConversation` once.
- Reopening existing DM should use bootstrap-provided or previously stored `conversation_id`, not another create call.
- Workspace channel screens may begin with workspace-owned channel context, but message history and commands use the resolved chat-owned `conversation_id`.
- `chat` and `workspace` remain write and history authorities; `realtime` never serves durable source data.

## Non-Goals

- Owning workspace membership or permission truth; `workspace` owns both.
- Owning group direct messages or broader social graph state; v1 direct messages are 1:1 only.
- Making realtime fanout the durability authority; `realtime` is a delivery optimization, not the message source of truth.
- Owning aggregated UI projections such as unread counts or sidebar summaries; those belong to downstream projections such as `bootstrap`.
- Owning moderation, attachments, or media-processing systems in this v1 contract.

## Dependencies

- **external application server through Envoy Gateway** for authenticated send, edit, delete, and history commands routed to chat gRPC.
- **workspace** for durable workspace and channel metadata events plus synchronous channel authorization checks.
- **identity** as the owner of stable `user_id` references used for direct-conversation participants and message actors.
- **realtime** for best-effort synchronous low-latency fanout after a durable message-create write succeeds.
- **RabbitMQ** for durable cold-path publication of chat events.
- **outbox worker sidecar** for polling local `outbox_event` rows and publishing them.
- **Postgres** as the service-owned source of truth for messages and edit history.
- **bootstrap** and other downstream consumers for projection materialization from durable chat events.

## Storage

- Chat owns a dedicated Postgres database.
- Message-domain rows and matching `outbox_event` rows are written in the same local transaction.
- `chat_message.client_message_id` is nullable, but when present it must be unique per conversation and author for retry-safe create semantics.
- `chat_message` points to exactly one `conversation_id`.
- `conversation.target_type` distinguishes channel targets from DM targets.
- `conversation.dm_pair_id` stores DM-only pair reference when `target_type = dm`.
- `conversation_read_cursor` stores chat-owned per-user read progress by `conversation_id`.
- `conversation_id` remains chat-owned and distinct from `workspace_channel_id` for workspace-channel conversations.
- `dm_pair` stores one normalized participant pair row for each 1:1 DM conversation.
- `user_snapshot`, `workspace_snapshot`, and `workspace_channel_snapshot` are minimal legitimacy snapshots only.
- `workspace_channel_id`, `conversation_id`, and `user_id` values are service-owned or cross-service references, not foreign keys into another service database.
- Redis is not required by default for v1 chat behavior.

## gRPC Surface

- `CreateMessage`
- `EditMessage`
- `DeleteMessage`
- `ListConversationMessages`
- `MarkConversationRead`
- `CreateConversation`

See `grpc.md` for request, response, and write-path rules.

## Event Surface

- `ConversationCreated`
- `DmPairCreated`
- `MessageCreated`
- `MessageEdited`
- `MessageDeleted`
- `ConversationReadCursorUpdated`

See `events.md` for payload and publication rules.

## Realtime Coordination

- Chat remains the durable source of truth for message acceptance and persisted history.
- Workspace continues to own workspace membership, channel metadata, and channel authorization truth, while chat owns direct-message participant-pair rows and durable conversation identity.
- Chat may reject workspace-channel conversation creation when local `workspace_snapshot` or `workspace_channel_snapshot` rows do not exist yet.
- After a message-create write commits successfully, chat may synchronously call `realtime.DeliverMessage` so connected clients can receive low-latency fanout over existing websocket subscriptions.
- Edits and deletes converge through durable chat events in v1 rather than a direct synchronous `chat -> realtime` hot path.
- Before accepting a workspace-channel write or history read, chat must call `workspace.AuthorizeChannelAction` synchronously.
- Before accepting a DM write or history read, chat must validate the local `conversation.dm_pair_id` row.
- `MarkConversationRead` updates chat-owned cursor state first; bootstrap later converges unread projections from durable read-cursor events.
- Synchronous fanout is best-effort for latency only; a failed notify must not roll back or invalidate an already committed chat write.
- Durable recovery and downstream convergence come from `outbox_event` publication to RabbitMQ, not from the synchronous notify path.
