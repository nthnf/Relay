## Identity Data Communication Diagram

```mermaid
flowchart LR
    C[Client] --> G[gateway]
    G -->|gRPC RegisterUser / AuthenticatePassword / VerifySession / RevokeSession| I[identity]
    G -->|gRPC RedeemEmailVerificationToken / UpdateUserProfile| I

    subgraph Identity DB Transaction
        UA[(user_account)]
        UP[(user_profile)]
        UC[(user_credential_password)]
        US[(user_session)]
        EV[(email_verification_token)]
        O[(outbox_event)]
    end

    I -->|write account + profile + credential + session| UA
    I --> UP
    I --> UC
    I --> US
    I --> EV
    I -->|same transaction inserts event row| O

    O --> W[outbox worker sidecar]
    W -->|publish durable events| R[RabbitMQ]
    R --> B[bootstrap and other consumers]

    G -->|uses session result as edge auth context| Ctx[forwarded actor context]
```

Notes:

- `gateway` owns the public auth edge but not account or session persistence.
- Identity writes domain rows and `outbox_event` rows in the same local Postgres transaction, including initial email verification token issuance and profile/email-verification updates.
- RabbitMQ publication is asynchronous and does not replace the synchronous registration or session result returned to `gateway`.
