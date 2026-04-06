# Bootstrap User Stories

## Load The Home Screen With One Aggregated Query

As a signed-in user, I want to call `GET /v1/bootstrap/home` and receive my profile summary, accepted-friend preview, workspace cards, and unread counts in one response so that the client can render the initial home screen without a cascade of follow-up fetches.

If some projections are still catching up, I still receive `200` with the latest available snapshot, using empty collections or zero-count defaults instead of a missing-resource error.

## Fetch Workspace Sidebar Data Without Calling Multiple Domain Services

As a signed-in user, I want to call `GET /v1/bootstrap/workspaces/{workspaceId}/sidebar` and receive workspace header data plus the ordered channel sidebar in one payload so that the UI does not need to fan out to workspace, membership, channel, and unread-specific services at runtime.

If bootstrap already knows I can access the workspace but some channel projections lag, I still receive `200` with the latest workspace snapshot and the latest available ordered channel list, which may be temporarily empty or partial.

## Understand Short-Lived Query Lag After Recent Writes

As a signed-in user, I understand that some query data may lag behind recent writes for a short period because bootstrap reads from asynchronously updated projections, so the client should tolerate brief eventual-consistency gaps after a friendship, workspace, channel, profile, or message write.
