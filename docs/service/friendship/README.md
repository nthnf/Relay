## Purpose

Friendship owns friend requests, accepted friendships, user blocks, and the relationship state transitions between two users. It is the write-side source of truth for interpersonal relationship state called by external application servers through Envoy Gateway.

## Owned Responsibilities

- Create and resolve directional friend requests.
- Materialize accepted friendships as symmetric service-owned edges.
- Enforce blocking precedence over requests and friendships.
- Remove friendships when a user explicitly removes a friend or when a new block invalidates an existing friendship.
- Publish durable relationship events through the local `outbox_event` table.
- Mirror identity user-account rows into friendship-local `user_account` read model for eventual-consistency target validation.

## Non-Goals

- Not source of truth for user accounts; `identity` owns that. Friendship keeps replicated `user_account` read model only.
- Serving the public HTTP edge; external application servers reach friendship through Envoy Gateway, while friendship keeps service-owned authorization.
- Acting as the canonical UI aggregate read service; `bootstrap` owns projection-backed friend-list reads.
- Inventing followers, groups, recommendations, or other social features outside direct bilateral relationships.

## Dependencies

- **external application server through Envoy Gateway** for authenticated friend and block commands routed to friendship gRPC.
- **RabbitMQ** for durable cold-path publication of relationship events.
- **outbox worker sidecar** for polling local `outbox_event` rows and publishing them.
- **Postgres** as the service-owned source of truth for requests, friendship edges, and blocks.
- **identity** events as source for friendship-local `user_account` mirror used on write paths.

## Storage

- Friendship owns a dedicated Postgres database.
- Domain writes and matching `outbox_event` inserts happen in the same local transaction.
- Redis is not required by default for v1 friendship behavior.

## gRPC Surface

- `CreateFriendRequest`
- `AcceptFriendRequest`
- `RejectFriendRequest`
- `RemoveFriend`
- `BlockUser`
- `UnblockUser`
- `ListFriends`
- `ListPendingRequests`

See `grpc.md` for request and response contracts.

## Event Surface

- `FriendRequestCreated`
- `FriendRequestAccepted`
- `FriendRequestRejected`
- `FriendRequestCanceledByBlock`
- `FriendshipRemoved`
- `UserBlocked`
- `UserUnblocked`

See `events.md` for payload and publication rules.

### V1 Relationship Rules

- Friend requests are directional: requester and addressee are not interchangeable on the request row.
- Accepted friendships are symmetric: v1 stores one edge per direction so reads stay user-scoped and simple.
- Blocking takes precedence over normal friend flows. If either direction is blocked, the pair cannot create or accept a friend request.
- Friendship validates write-path target users against local replicated `user_account` rows before creating a friend request or block. Upstream callers may pre-validate, but friendship still enforces the invariant at its own boundary.
- If local `user_account` row absent, friendship rejects the write and does not persist orphaned `friend_request`, `friendship_edge`, or `user_block` rows.
- `BlockUser` removes any accepted friendship edges for the pair and clears pending requests in either direction in the same local transaction.
- `UnblockUser` removes only the caller-owned block row; it does not restore prior friendship or pending request state.
