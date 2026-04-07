## gRPC Service Scope

Bootstrap exposes synchronous projection-backed read contracts for the external SvelteKit server runtime through Envoy Gateway. It remains the canonical UI-facing aggregate read service for cross-domain reads and does not own domain writes.

## Shared Contract Rules

- Authenticated end-user reads derive actor/session context from Envoy-validated access-token claims forwarded through Envoy Gateway.
- Bootstrap serves projection-backed snapshots only; it does not perform write-side authorization or domain mutation.
- Read responses may lag recent writes because upstream projections converge asynchronously.
- Collection ordering is contractual: friends sort by `sort_username` ascending then `friend_user_id`, workspaces sort by `last_activity_at` descending then `workspace_id`, and sidebar channels sort by `position` ascending then `channel_id`.
- Missing projected rows caused by lag return the latest available snapshot shape rather than failing the whole read when bootstrap already has evidence of access.

### `GetHome`

**Main caller:** external SvelteKit server runtime through Envoy Gateway

**Request fields**

- none beyond Envoy-forwarded authenticated actor/session context

**Response fields**

- `user` (`message`) with `user_id`, `username`, `display_name`, `avatar_url`
- `summary` (`message`) with `friend_count`, `workspace_count`, `unread_workspace_count`, `total_unread_count`
- `friends_preview` (`repeated message`) ordered by username ascending then user ID
- `workspaces` (`repeated message`) ordered by latest activity descending then workspace ID

**Contract notes**

- Returns the latest available projected home snapshot for the authenticated actor.
- Projection lag returns empty collections or zero-count defaults rather than not found.

### `ListFriends`

**Main caller:** external SvelteKit server runtime through Envoy Gateway

**Request fields**

- none beyond Envoy-forwarded authenticated actor/session context

**Response fields**

- `items` (`repeated message`) with accepted-friend rows ordered by username ascending then user ID

**Contract notes**

- V1 includes accepted friends only.
- Returns an empty collection when no accepted-friend projection rows have materialized yet.

### `ListWorkspaces`

**Main caller:** external SvelteKit server runtime through Envoy Gateway

**Request fields**

- none beyond Envoy-forwarded authenticated actor/session context

**Response fields**

- `items` (`repeated message`) with workspace summaries ordered by latest activity descending then workspace ID

**Contract notes**

- Returns an empty collection when no workspace projection rows have materialized yet.

### `GetWorkspaceSidebar`

**Main caller:** external SvelteKit server runtime through Envoy Gateway

**Request fields**

- `workspace_id` (`uuid`)

**Response fields**

- `workspace` (`message`) with latest available projected workspace header
- `channels` (`repeated message`) ordered by position ascending then channel ID

**Contract notes**

- Returns not found only when bootstrap has no projected evidence that the actor can access the workspace.
- If access is known but channel projections lag, the response still succeeds with the latest available workspace snapshot and an empty or partial ordered channel list.
