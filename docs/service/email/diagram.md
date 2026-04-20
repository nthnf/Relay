## Email Data Communication Diagram

```mermaid
flowchart LR
    RabbitMQ[RabbitMQ] -->|UserRegistered| Email[email]

    subgraph Email DB
        OE[(outbound_email)]
        EDA[(email_delivery_attempt)]
    end

    Email -->|idempotent enqueue and local send state write| OE
    Email -->|record each submission or failure| EDA
    OE -->|pending email selected for provider handoff| Email
    Email -->|submit rendered message| Provider[email provider]
    Provider -->|acceptance or failure result| Email
    Email -->|update send_status, provider ids, last error| OE
    Email -->|append internal attempt record| EDA
```

Notes:

- RabbitMQ is the durable trigger source; email does not read identity or workspace databases directly.
- Email persists local send state before and after provider handoff so retries and operator inspection stay service-owned.
- `email_delivery_attempt` is an internal operational record, not a published v1 event stream.
