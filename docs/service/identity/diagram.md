## Identity Data Communication Diagram

```mermaid
flowchart LR
    B[Browser Client] --> A[External SvelteKit]
    A --> G[Envoy Gateway]
    G -->|gRPC RegisterUser / AuthenticatePassword / RefreshSession / RevokeSession| I[identity]
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

    G -->|forwards ingress-authenticated request context for authenticated RPCs| Ctx[request actor context]
```

Notes:

- Envoy Gateway owns backend ingress policy, validates short-lived access JWTs on protected routes, and forwards authenticated request context where required.
- `RegisterUser`, `AuthenticatePassword`, `RefreshSession`, and `RedeemEmailVerificationToken` are auth-entry RPCs; protected actor context applies to profile-bound calls only.
- Identity writes domain rows and `outbox_event` rows in the same local Postgres transaction, including initial email verification token issuance and profile/email-verification updates.
- RabbitMQ publication is asynchronous and does not replace the synchronous token-pair or profile result returned to the external caller through Envoy Gateway.
