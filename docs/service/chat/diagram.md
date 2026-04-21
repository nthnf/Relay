## Chat Data Communication Diagram

```mermaid
flowchart LR
    Browser[Browser Client] --> App[External SvelteKit]
    App --> Gateway[Envoy Gateway]
    Gateway -->|gRPC CreateMessage / ListConversationMessages| Chat[chat]
    Gateway -->|gRPC CreateConversation / CreateMessage / ListConversationMessages| Chat
    Gateway -->|gRPC EditMessage / DeleteMessage| Chat
    Chat -->|sync AuthorizeChannelAction| Workspace[workspace]

    subgraph Chat DB Transaction
        C[(conversation)]
        CMEM[(conversation_member)]
        US[(user_snapshot)]
        WS[(workspace_snapshot)]
        WCS[(workspace_channel_snapshot)]
        CM[(chat_message)]
        O[(outbox_event)]
    end

    Chat -->|create or load conversation row| C
    Chat -->|persist DM participants| CMEM
    Chat -->|project user legitimacy| US
    Chat -->|project workspace legitimacy| WS
    Chat -->|project channel legitimacy| WCS
    Chat -->|write or edit current message row| CM
    Chat -->|same transaction inserts event row| O

    Chat -->|post-commit DeliverMessage| Realtime[realtime]

    O --> Worker[outbox worker sidecar]
    Worker -->|publish durable chat events| RabbitMQ[RabbitMQ]
    RabbitMQ -->|workspace-channel MessageCreated / MessageEdited / MessageDeleted| Bootstrap[bootstrap]
    RabbitMQ -->|chat events for repair and catch-up| Realtime

    Bootstrap -->|upsert channel projections| Projections[(bootstrap chat projections)]
```

Notes:

- Envoy Gateway owns backend ingress policy; chat owns durable message, edit, and delete invariants plus service-boundary authorization.
- Workspace-channel writes and reads depend on synchronous `workspace.AuthorizeChannelAction` checks before chat accepts them.
- Chat owns DM participant membership used to authorize DM reads and writes.
- Chat writes domain rows and `outbox_event` rows in the same local Postgres transaction.
- Channel message fanout and direct-message fanout both call `realtime.DeliverMessage` only after durable write success and remain best-effort for latency.
- RabbitMQ publication is the durable path for downstream convergence, replay, and recovery when synchronous fanout is unavailable.
- `workspace` is shown as a contract dependency for validation only; chat still owns message persistence and never writes workspace data.
