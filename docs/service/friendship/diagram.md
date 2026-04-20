## Friendship Data Communication Diagram

```mermaid
flowchart LR
    Browser[Browser Client] --> App[External SvelteKit]
    App --> Gateway[Envoy Gateway]
    Gateway -->|gRPC CreateFriendRequest / AcceptFriendRequest / RejectFriendRequest| Friendship[friendship]
    Gateway -->|gRPC RemoveFriend / BlockUser / UnblockUser / ListFriends / ListPendingRequests| Friendship
    RabbitMQ[RabbitMQ] -->|UserRegistered / UserEmailVerified| Friendship

    subgraph Friendship DB Transaction
        UA[(user_snapshot)]
        FR[(friend_request)]
        FE[(friendship_edge)]
        UB[(user_block)]
        O[(outbox_event)]
    end

    Friendship -->|read target existence mirror| UA
    Friendship -->|write request state| FR
    Friendship -->|write accepted symmetric edges| FE
    Friendship -->|write block state| UB
    Friendship -->|same transaction inserts event row| O

    O --> Worker[outbox worker sidecar]
    Worker -->|publish durable friendship events| RabbitMQ[RabbitMQ]
    RabbitMQ -->|FriendRequestAccepted / FriendshipRemoved| Bootstrap[bootstrap]
    Bootstrap -->|upsert or delete accepted-friend rows only| Projections[(bootstrap friend_projection)]
```

Notes:

- Envoy Gateway owns backend ingress policy; protected routes call identity `Authorization/Check`, then friendship reads trusted actor headers such as `x-user-id`.
- Friendship reads local `user_snapshot` mirror for target validation only.
- Friendship writes domain rows and `outbox_event` rows in same local Postgres transaction.
- RabbitMQ publication is asynchronous durable path that keeps `user_snapshot` mirror converged and lets `bootstrap` converge accepted-friend projections.
- V1 bootstrap scope here is accepted-friend projection maintenance only; pending-request and block state remain friendship-owned and are not projected by bootstrap.
- `BlockUser` may write `user_block`, resolve pending requests, remove friendship edges, and enqueue multiple outbox events in one transaction.
