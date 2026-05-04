## gRPC Service Scope

Friendship exposes synchronous relationship command and bounded read contracts. External application servers through Envoy Gateway are the primary callers for end-user request, accept, reject, remove, and block flows. These calls stay synchronous for immediate correctness and block enforcement, not because they share chat/realtime latency targets.

## Shared Contract Rules

- Authenticated actor identity is derived from Envoy-validated access-token context; callers must not be allowed to mutate another actor's relationship state by supplying arbitrary user IDs.
- Envoy calls identity `Authorization/Check` on protected routes, then forwards trusted actor headers such as `x-user-id` out-of-band to friendship.
- External application callers do not supply actor identity in request payloads for end-user actions; the transport boundary or a trusted backend caller context attaches it out-of-band.
- Friendship enforces target-user existence at its own boundary by validating write-path target IDs against local replicated `user_snapshot` rows; this mirror is for target validation only, not for re-validating the authenticated actor header.
- If local replica lacks target user, friendship rejects write with not-found-style domain error and persists no relationship row for that target.
- Blocking precedence applies to all normal friend interactions. If either direction has an active `user_block` row, `CreateFriendRequest` and `AcceptFriendRequest` must fail with a conflict-style domain error.
- Accepted friendship is symmetric and must be created or removed as two `friendship_edge` rows in one transaction.
- Duplicate-request handling is explicit: if the pair already has a pending request in either direction, `CreateFriendRequest` must not create a second active pending row.

### `CreateFriendRequest`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `target_username` (`string`) - public handle of the user receiving the request, such as `demo.agent#0420`.

**Response fields**

- `friend_request_id` (`uuid`)
- `requester_user_id` (`uuid`)
- `addressee_user_id` (`uuid`)
- `status` (`string`)
- `created_at` (`timestamp`)
- `requester` (`message`) with `user_id`, `username`, `display_name`, `avatar_url`
- `addressee` (`message`) with `user_id`, `username`, `display_name`, `avatar_url`

**Contract notes**

- Reject self-targeting requests.
- Resolve `target_username` through the local `user_snapshot.username` mirror before inserting the request row.
- Reject if the pair is already friends.
- Reject if a block exists in either direction.
- Reject if a pending request already exists in either direction.
- On success, insert `friend_request` plus matching `FriendRequestCreated` outbox row in one transaction.

### `AcceptFriendRequest`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `friend_request_id` (`uuid`)

**Response fields**

- `friend_request_id` (`uuid`)
- `requester_user_id` (`uuid`)
- `addressee_user_id` (`uuid`)
- `accepted_at` (`timestamp`)

**Contract notes**

- Only the current pending request addressee may accept.
- Reject if the request is not `pending`.
- Reject if a block exists in either direction at acceptance time.
- On success, mark the request accepted, insert both `friendship_edge` rows, and insert `FriendRequestAccepted` outbox data in one transaction.

### `RejectFriendRequest`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `friend_request_id` (`uuid`)

**Response fields**

- `friend_request_id` (`uuid`)
- `status` (`string`)
- `resolved_at` (`timestamp`)

**Contract notes**

- Only the current pending request addressee may reject.
- Reject if the request is not `pending`.
- On success, update the request status to `rejected` and insert a `FriendRequestRejected` outbox row in the same transaction.

### `RemoveFriend`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `friend_user_id` (`uuid`)

**Response fields**

- `removed` (`bool`)
- `removed_at` (`timestamp optional`)

**Contract notes**

- Remove both directional `friendship_edge` rows.
- This method is idempotent: if no active friendship exists, return `removed = false`.
- Successful removal inserts a `FriendshipRemoved` outbox row.
- v1 outbox keying uses `aggregate_type = friendship` and `aggregate_id = authenticated actor_user_id`.

### `BlockUser`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `target_user_id` (`uuid`) - user being blocked.

**Response fields**

- `blocked` (`bool`)
- `blocked_at` (`timestamp`)
- `already_blocked` (`bool`)

**Contract notes**

- Reject self-block attempts.
- Validate `target_user_id` existence through local `user_snapshot` mirror before creating a block row.
- Create or preserve the directional `user_block` row for `(authenticated actor, target_user_id)`.
- If the block already exists, return `blocked = true`, `already_blocked = true`, and the original `blocked_at` timestamp without creating another row.
- An already-blocked no-op call publishes no `UserBlocked`, `FriendRequestCanceledByBlock`, or `FriendshipRemoved` event because there is no state change.
- Remove accepted friendship edges for the pair when present.
- Resolve pending requests for the pair in either direction with a block reason.
- Publish `UserBlocked` only when a new block row is created; if the new block resolves the pair's pending request, also publish `FriendRequestCanceledByBlock`; if friendship rows were removed, also publish `FriendshipRemoved` from the same transaction.

### `UnblockUser`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `target_user_id` (`uuid`)

**Response fields**

- `unblocked` (`bool`)
- `unblocked_at` (`timestamp optional`)

**Contract notes**

- Remove only the caller-owned directional block row.
- This method is idempotent: if no active block exists, return `unblocked = false`.
- Unblocking does not restore friendship or request state automatically.
- Successful removal inserts a `UserUnblocked` outbox row.

### `ListFriends`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `page_size` (`int32 optional`)
- `page_token` (`string optional`)

**Response fields**

- `friends` (`repeated message`) with `friend_user_id`, `accepted_at`
- `next_page_token` (`string optional`)

**Contract notes**

- Returns only accepted friendship edges for the authenticated actor.
- This is a bounded owner read, not a cross-domain aggregate projection.
- Mutable display fields should come from `bootstrap` or identity-backed lookups, not duplicated friendship ownership.

### `ListPendingRequests`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `direction` (`string optional`) - `incoming`, `outgoing`, or `all`; default `incoming`.
- `page_size` (`int32 optional`)
- `page_token` (`string optional`)

**Response fields**

- `requests` (`repeated message`) with `friend_request_id`, `requester_user_id`, `addressee_user_id`, `created_at`, `status`, `requester`, `addressee`
- `next_page_token` (`string optional`)

**Contract notes**

- Returns pending rows only.
- Each row includes requester/addressee summaries from friendship's local `user_snapshot` mirror so pending request lists are self-contained.
- `incoming` filters by `addressee_user_id = authenticated actor`; `outgoing` filters by `requester_user_id = authenticated actor`.
- Blocked pairs should not surface a still-pending row because `BlockUser` resolves matching requests transactionally.

### `ListBlockedUsers`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `page_size` (`int32 optional`)
- `page_token` (`string optional`)

**Response fields**

- `blocked_users` (`repeated message`) with `target_user_id`, `blocked_at`, and `target` user summary.
- `next_page_token` (`string optional`)

**Contract notes**

- Returns only block rows owned by the authenticated actor.
- Each row includes target display fields from friendship's local `user_snapshot` mirror for self-contained safety UI rendering.
- This read does not imply friendship or request state; unblock remains a separate command.
