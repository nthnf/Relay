## gRPC Service Scope

Bootstrap exposes synchronous projection-backed read contracts for external SvelteKit server runtime through Envoy Gateway. It remains canonical UI-facing aggregate read service for app-shell, workspace-shell, and DM-shell reads and does not own domain writes.

## Shared Contract Rules

- Authenticated end-user reads derive actor/session context from Envoy-validated access-token claims forwarded through Envoy Gateway.
- Bootstrap serves projection-backed snapshots only; it does not perform write-side authorization or domain mutation.
- Read responses may lag recent writes because upstream projections converge asynchronously.
- Missing projected rows caused by lag return latest available snapshot shape rather than failing whole read when bootstrap already has evidence of access.
- Collection ordering is contractual: workspaces sort by `last_activity_at` descending then `workspace_id`, workspace channels sort by `position` ascending then `channel_id`, and DMs sort by `last_activity_at` descending then `conversation_id`.

### `GetAppBootstrap`

**Main caller:** external SvelteKit server runtime through Envoy Gateway

**Request fields**

- none beyond Envoy-forwarded authenticated actor/session context

**Response fields**

- `viewer` (`message`) with `user_id`, `username`, `display_name`, `avatar_url`
- `summary` (`message`) with `workspace_count`, `unread_workspace_count`, `total_unread_count`, `pending_friend_request_count`
- `workspaces` (`repeated message`) with `workspace_id`, `name`, `icon_url`

**Contract notes**

- Returns latest available projected app-shell snapshot for authenticated actor.
- Projection lag returns empty collections or zero-count defaults rather than not found.
- This method is intentionally thin for first paint: viewer identity, workspace navigation, and badge counts only.

### `GetWorkspaceBootstrap`

**Main caller:** external SvelteKit server runtime through Envoy Gateway

**Request fields**

- `workspace_id` (`uuid`)

**Response fields**

- `workspace` (`message`) with latest available projected workspace header
- `channels` (`repeated message`) ordered by position ascending then channel ID
- each channel row includes `channel_id`, `conversation_id`, `name`, `channel_kind`, `position`, `unread_count`, and `mention_count`

**Contract notes**

- Returns not found only when bootstrap has no projected evidence that actor can access workspace.
- If access is known but channel projections lag, response still succeeds with latest available workspace snapshot and empty or partial ordered channel list.
- `conversation_id` is denormalized into each row so caller can open chat without separate lookup RPC.

### `GetDmBootstrap`

**Main caller:** external SvelteKit server runtime through Envoy Gateway

**Request fields**

- none beyond Envoy-forwarded authenticated actor/session context

**Response fields**

- `items` (`repeated message`) ordered by latest activity descending then conversation ID
- each row includes `conversation_id`, `dm_pair_id`, peer profile summary, `unread_count`, `last_message_preview`, and `last_activity_at`

**Contract notes**

- Returns empty collection when no DM projection rows have materialized yet.
- `conversation_id` is denormalized into each row so caller can open chat without separate lookup RPC.
- DM list is on-demand shell data and is not required to be present in app first-paint bootstrap.
