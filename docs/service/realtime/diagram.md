## Realtime Data Communication Diagram

```mermaid
flowchart LR
    Client[Client] --> Gateway[gateway]
    Gateway -->|authenticated websocket upgrade| Realtime[realtime]
    Realtime -->|attach/subscribe populates routes| Routes[In-memory session registry and subscription maps]

    Chat[chat] -->|gRPC PublishChannelMessage| Realtime
    Chat -->|gRPC PublishDirectMessage| Realtime

    Realtime -->|fanout channel message by last-known authorized route| WS1[connected channel sessions]
    Realtime -->|fanout direct message by last-known authorized route| WS2[connected DM participant sessions]

    Workspace[workspace] -->|gRPC PushWorkspaceEvent or durable workspace events| Realtime

    subgraph Realtime State
        Routes
        Redis[(Redis\npresence_state / presence_sessions)]
        Cursor[(Postgres\nrealtime_delivery_cursor)]
    end

    Realtime -->|set online/offline presence| Redis
    Realtime -->|track minimal recovery cursor| Cursor

    Chat -->|outbox_event via sidecar| RabbitMQ[RabbitMQ]
    Workspace -->|outbox_event via sidecar| RabbitMQ

    RabbitMQ -->|MessageCreated / MessageEdited / MessageDeleted / reaction events with event_id| Realtime
    RabbitMQ -->|WorkspaceMemberAdded / WorkspaceMemberRemoved / WorkspaceChannelCreated| Realtime

    Realtime -->|prune stale routes on access change| Routes
    Realtime -->|backup event path repair for active sessions| WS1
    Realtime -->|backup event path repair for active sessions| WS2
```

Notes:

- `chat -> realtime` gRPC is the low-latency path for already committed channel and DM writes.
- Routing state is ephemeral and populated by websocket attach/subscribe after auth; stale routes are pruned when upstream ownership changes converge.
- RabbitMQ consumption is the backup and recovery path when direct fanout fails or is delayed for active sessions.
- Full reconnect history catch-up is not performed by realtime; clients must reload from chat/bootstrap reads after reconnect.
- Redis is the primary v1 presence store; Postgres stays minimal and operational.
- Realtime owns websocket delivery and presence only; it does not become the durable message authority.
