## Persistence Scope

Friendship owns relationship-write state only: pending requests, accepted friendship edges, and user blocks. Other services must use friendship gRPC or friendship events instead of reading these tables directly.

## Core Tables

### `friend_request`

One row per open or resolved directional friend request.

| Column | Type | Notes |
| --- | --- | --- |
| `friend_request_id` | `uuid` | Primary key. Stable request identifier for accept/reject flows. |
| `requester_user_id` | `uuid` | Identity-owned user reference for the actor who sent the request. |
| `addressee_user_id` | `uuid` | Identity-owned user reference for the target user. |
| `status` | `text` | Contract values: `pending`, `accepted`, `rejected`, `canceled_by_block`. |
| `created_at` | `timestamptz` | Request creation time. |
| `resolved_at` | `timestamptz null` | Null while pending; set when request leaves `pending`. |
| `resolution_reason` | `text null` | Contract values such as `accepted`, `rejected`, `blocked`. |

Semantic rules:

- Requests are directional: `(requester_user_id, addressee_user_id)` is not interchangeable with the reverse pair.
- V1 allows at most one active `pending` request across a user pair. If a reverse pending request already exists, `CreateFriendRequest` must reject the duplicate and require the caller to accept or reject the existing inbound request.
- A user cannot create a request to themselves.
- Friendship must synchronously verify `addressee_user_id` existence through `identity` before inserting the row.
- A request cannot remain `pending` once the pair has an active block in either direction or an accepted friendship already exists.
- Historical non-pending rows may be retained for audit and idempotency; open-request uniqueness applies to the pending state only.

### `friendship_edge`

User-scoped accepted friendship edge. V1 stores one row per direction so accepted-friend reads stay local to a single `user_id`.

| Column | Type | Notes |
| --- | --- | --- |
| `user_id` | `uuid` | First half of the primary key. Identity-owned user reference. |
| `friend_user_id` | `uuid` | Second half of the primary key. Identity-owned user reference for the friend. |
| `friend_request_id` | `uuid` | Source request that created the friendship. |
| `accepted_at` | `timestamptz` | Time the friendship became active. |
| `created_at` | `timestamptz` | Row creation time. |

Semantic rules:

- Accepted friendship is symmetric at the domain level but materialized as two rows: `(user_id=A, friend_user_id=B)` and `(user_id=B, friend_user_id=A)`.
- Both directional rows must be inserted and removed in the same transaction so the pair never diverges durably.
- A pair with active `friendship_edge` rows cannot create another pending request.
- `RemoveFriend` deletes both directional rows.
- `BlockUser` also deletes both directional rows when present because block state takes precedence over friendship state.

### `user_block`

Directional block state owned by the blocking user.

| Column | Type | Notes |
| --- | --- | --- |
| `blocker_user_id` | `uuid` | First half of the primary key. Identity-owned user reference for the actor enforcing the block. |
| `blocked_user_id` | `uuid` | Second half of the primary key. Identity-owned user reference for the blocked actor. |
| `created_at` | `timestamptz` | Time the block became active. |
| `reason` | `text null` | Optional product-owned reason or audit note in later revisions. |

Semantic rules:

- Blocks are directional: `A blocks B` is independent from `B blocks A`.
- Blocking precedence is explicit: if either `(A blocks B)` or `(B blocks A)` exists, the pair cannot create or accept friend requests.
- Friendship must synchronously verify `blocked_user_id` existence through `identity` before inserting the row.
- `BlockUser` is idempotent for an already-active row and must not create duplicate block rows.
- An idempotent `BlockUser` no-op returns the existing block state and causes no additional outbox publication because no relationship state change occurred.
- Creating a block removes accepted friendship edges and resolves pending requests for the pair with `resolution_reason = blocked` in the same transaction.
- `UnblockUser` removes only `(blocker_user_id, blocked_user_id)` and does not recreate deleted friendship or request state.

## Relations

- `friendship_edge.friend_request_id -> friend_request.friend_request_id` (accepted edges originate from one accepted request)
- `friend_request.requester_user_id` and `friend_request.addressee_user_id` reference identity-owned `user_id` values.
- `friendship_edge.user_id`, `friendship_edge.friend_user_id`, `user_block.blocker_user_id`, and `user_block.blocked_user_id` all reference identity-owned `user_id` values.

## Cross-Service References

- `identity` owns the stable `user_id` namespace and profile basics used by friendship payloads and downstream projections.
- Friendship synchronously validates write-path target users through `identity` and rejects unknown users instead of persisting orphaned relationship rows.
- External application servers call friendship through Envoy Gateway; friendship does not trust arbitrary caller-supplied actor identity and must authorize from Envoy-validated access-token context at its own boundary.
- `bootstrap` consumes `FriendRequestAccepted` and `FriendshipRemoved` to materialize and remove accepted-friend projection rows.
- Friendship inserts durable integration events into its local `outbox_event` table in the same transaction as the source write.
