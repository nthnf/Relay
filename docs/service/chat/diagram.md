## Chat Data Communication Diagram

```mermaid
flowchart LR
    Client[Client] --> Gateway[gateway]
    Gateway -->|gRPC CreateMessage / ListChannelMessages| Chat[chat]
    Gateway -->|gRPC GetOrCreateDirectConversation / CreateMessage / ListDirectConversationMessages| Chat
    Gateway -->|gRPC EditMessage / DeleteMessage / AddReaction / RemoveReaction| Chat
    Chat -->|validate workspace membership and channel access| Workspace[workspace]

    subgraph Chat DB Transaction
        DC[(direct_conversation)]
        DCM[(direct_conversation_member)]
        CM[(chat_message)]
        CME[(chat_message_edit)]
        CMR[(chat_message_reaction)]
        O[(outbox_event)]
    end

    Chat -->|create or load direct conversation| DC
    Chat -->|persist direct conversation participants| DCM
    Chat -->|write channel or direct message row| CM
    Chat -->|append edit history| CME
    Chat -->|write reaction state| CMR
    Chat -->|same transaction inserts event row| O

    Chat -->|post-commit channel fanout| Realtime[realtime]
    Chat -->|post-commit direct-message fanout| Realtime

    O --> Worker[outbox worker sidecar]
    Worker -->|publish durable chat events| RabbitMQ[RabbitMQ]
    RabbitMQ -->|workspace-channel MessageCreated / MessageEdited / MessageDeleted| Bootstrap[bootstrap]
    RabbitMQ -->|chat events for repair and catch-up| Realtime

    Bootstrap -->|upsert channel projections| Projections[(bootstrap chat projections)]
```

Notes:

- `gateway` owns the authenticated edge; chat owns durable message, edit, delete, and reaction invariants.
- Workspace-channel writes and reads depend on workspace-owned membership and channel validation before chat accepts them.
- Chat also owns direct-message conversation metadata and participant membership used to authorize DM reads and writes.
- Chat writes domain rows and `outbox_event` rows in the same local Postgres transaction.
- Channel message fanout and direct-message fanout both call `realtime` only after durable write success and remain best-effort for latency.
- RabbitMQ publication is the durable path for downstream convergence, replay, and recovery when synchronous fanout is unavailable.
- `workspace` is shown as a contract dependency for validation only; chat still owns message persistence and never writes workspace data.
