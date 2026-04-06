## Friendship Data Communication Diagram

```mermaid
flowchart LR
    Client[Client] --> Gateway[gateway]
    Gateway -->|gRPC CreateFriendRequest / AcceptFriendRequest / RejectFriendRequest| Friendship[friendship]
    Gateway -->|gRPC RemoveFriend / BlockUser / UnblockUser / ListFriends / ListPendingRequests| Friendship

    subgraph Friendship DB Transaction
        FR[(friend_request)]
        FE[(friendship_edge)]
        UB[(user_block)]
        O[(outbox_event)]
    end

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

- `gateway` owns the authenticated public edge; friendship owns relationship state and invariants.
- Friendship writes domain rows and `outbox_event` rows in the same local Postgres transaction.
- RabbitMQ publication is asynchronous and is the durable path that lets `bootstrap` converge accepted-friend projections after friendship writes.
- V1 bootstrap scope here is accepted-friend projection maintenance only; pending-request and block state remain friendship-owned and are not projected by bootstrap.
- `BlockUser` may write `user_block`, resolve pending requests, remove friendship edges, and enqueue multiple outbox events in one transaction.
