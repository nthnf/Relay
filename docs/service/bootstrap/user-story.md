# Bootstrap User Stories

## Load App Shell With One Aggregated Query

As signed-in user, I want external SvelteKit server runtime to call bootstrap's `GetAppBootstrap` RPC through Envoy Gateway and receive my viewer profile, workspace sidebar rows, and badge counts in one response so UI can render first signed-in shell without cascade of backend reads.

If some projections are still catching up, application still receives latest available snapshot, using empty collections or zero-count defaults instead of missing-resource error.

## Fetch Workspace Shell Data With Conversation Mapping Included

As signed-in user, I want external SvelteKit server runtime to call bootstrap's `GetWorkspaceBootstrap` RPC through Envoy Gateway and receive workspace header data plus ordered channel list with chat `conversation_id` values already attached so UI can open workspace chat targets without extra lookup requests.

If bootstrap already knows I can access workspace but some channel projections lag, application still receives latest workspace snapshot and latest available ordered channel list, which may be temporarily empty or partial.

## Fetch DM Shell Data On Demand

As signed-in user, I want external SvelteKit server runtime to call bootstrap's `GetDmBootstrap` RPC only when I enter DM section so UI receives ordered DM thread list with peer summaries, unread counts, previews, and `conversation_id` values without bloating app first-paint bootstrap.

## Understand Short-Lived Query Lag After Recent Writes

As signed-in user, I understand that some query data may lag behind recent writes for short period because bootstrap reads from asynchronously updated projections, so client should tolerate brief eventual-consistency gaps after friendship, workspace, channel, conversation, read-cursor, profile, or message writes.
