## Publication Model

Friendship publishes integration events by inserting service-owned rows into `outbox_event` inside the same transaction as the source relationship write. The shared outbox worker later publishes those rows to RabbitMQ.

## Published Events

### `FriendRequestCreated`

**When published**

- After a new pending friend request is committed successfully.

**Minimum payload**

- `friend_request_id`
- `requester_user_id`
- `addressee_user_id`
- `status`
- `created_at`

**Typical consumers**

- Audit or notification workflows
- Services that need durable awareness of a newly pending request

### `FriendRequestAccepted`

**When published**

- After a pending request is accepted and both directional `friendship_edge` rows are committed.

**Minimum payload**

- `friend_request_id`
- `requester_user_id`
- `addressee_user_id`
- `accepted_at`
- `friendship_pairs` with both `(user_id, friend_user_id)` directions

**Typical consumers**

- `bootstrap` friend projections
- Services that need accepted-relationship state for authorization or denormalized counters

### `FriendRequestRejected`

**When published**

- After a pending request is explicitly rejected by the addressee.

**Minimum payload**

- `friend_request_id`
- `requester_user_id`
- `addressee_user_id`
- `rejected_at`

**Typical consumers**

- Audit or notification workflows

### `FriendRequestCanceledByBlock`

**When published**

- After `BlockUser` resolves the pair's single active pending friend request.

**Minimum payload**

- `friend_request_id`
- `requester_user_id`
- `addressee_user_id`
- `blocked_by_user_id`
- `canceled_at`
- `status` with value `canceled_by_block`

**Typical consumers**

- Audit or notification workflows
- Services that need durable visibility into request closure that was not an explicit rejection

### `FriendshipRemoved`

**When published**

- After an accepted friendship is removed by `RemoveFriend`.
- Also published when `BlockUser` removes an accepted friendship because block state overrides friendship state.

**Minimum payload**

- `friendship_pairs` with both `(user_id, friend_user_id)` directions removed from the pair
- `removed_at`
- `reason` with contract values such as `removed_by_user` or `blocked`

**Typical consumers**

- `bootstrap` friend projections
- Services that need durable friendship-removal awareness

### `UserBlocked`

**When published**

- After a directional block row becomes active.
- Emitted only on state change; an already-blocked `BlockUser` no-op does not publish another `UserBlocked` event.

**Minimum payload**

- `blocker_user_id`
- `blocked_user_id`
- `blocked_at`

**Typical consumers**

- Authorization-sensitive services
- Audit or abuse-monitoring workflows

### `UserUnblocked`

**When published**

- After a directional block row is removed.

**Minimum payload**

- `blocker_user_id`
- `blocked_user_id`
- `unblocked_at`

**Typical consumers**

- Authorization-sensitive services
- Audit workflows

## Event Rules

- Event payloads use friendship-owned relationship keys plus identity-owned `user_id` references only.
- `FriendRequestAccepted` is the durable create signal for accepted friendships; `FriendshipRemoved` is the durable delete signal.
- `UserBlocked` does not by itself imply symmetric blocking; the event is directional like `user_block`.
- When one transaction creates a block, resolves the pair's single pending request, and removes a friendship, the outbox must contain the block event plus any matching `FriendRequestCanceledByBlock` and `FriendshipRemoved` events so downstream consumers converge correctly.
- Publication ordering should be preserved per user pair where request, friendship, and block writes affect the same relationship.
- Consumers must be idempotent because replay and duplicate delivery are expected platform behaviors.
