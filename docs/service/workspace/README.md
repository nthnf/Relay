## Purpose

Workspace owns workspace containers, membership, roles, invitations, and channel metadata. It is the write-side source of truth for who belongs to a workspace and which channels exist inside it when called by external application servers through Envoy Gateway.

## Owned Responsibilities

- Create workspaces and seed the creator as the first member.
- Own workspace membership lifecycle, including add, remove, and invitation acceptance.
- Own workspace-scoped roles and member-role assignments.
- Issue and track workspace invitations with explicit expiry.
- Own channel metadata such as channel name, kind, and ordering within a workspace.
- Publish durable workspace integration events through the local `outbox_event` table.

## Non-Goals

- Owning message bodies, message delivery, or unread state; `chat` owns durable message writes and `bootstrap` owns aggregated unread projections.
- Owning realtime connection state or websocket fanout; `realtime` only consumes selected workspace events.
- Acting as the public HTTP edge; external application servers reach workspace through Envoy Gateway, while workspace keeps service-owned authorization.
- Replacing `bootstrap` as the canonical UI-facing aggregate read service for workspace lists and sidebars.

## Dependencies

- **external application server through Envoy Gateway** for authenticated workspace, membership, invitation, and channel commands routed to workspace gRPC.
- **RabbitMQ** for durable cold-path publication of workspace events.
- **outbox worker sidecar** for polling local `outbox_event` rows and publishing them.
- **Postgres** as the service-owned source of truth for workspaces, memberships, roles, invitations, and channels.
- **identity** as the owner of stable `user_id` references used by membership and invitation records, and for synchronous target-user existence validation on invitation and direct-member-add write paths.
- **bootstrap** as a downstream consumer of workspace and channel events for UI-facing projections.
- **realtime** as a downstream consumer of selected membership and channel events for connected-client updates.

## Storage

- Workspace owns a dedicated Postgres database.
- Domain writes and matching `outbox_event` inserts happen in the same local transaction.
- Each workspace aggregate uses service-owned UUIDs; cross-service references use identity-owned `user_id` values only.
- Redis is not required by default for v1 workspace behavior.

## gRPC Surface

- `CreateWorkspace`
- `GetWorkspace`
- `ListWorkspacesForUser`
- `CreateChannel`
- `ListChannels`
- `AddMember`
- `RemoveMember`
- `IssueInvitation`
- `AcceptInvitation`

See `grpc.md` for request and response contracts.

## Event Surface

- `WorkspaceCreated`
- `WorkspaceMemberAdded`
- `WorkspaceMemberRemoved`
- `WorkspaceInvitationIssued`
- `WorkspaceInvitationAccepted`
- `WorkspaceChannelCreated`

See `events.md` for payload and publication rules.

### V1 Workspace Rules

- `CreateWorkspace` creates the `workspace` row and the creator's `workspace_member` row in the same transaction.
- `owner_user_id` is the canonical ultimate authority for the workspace in v1. Roles delegate permissions to non-owner members, but the owner implicitly has all permissions even without explicit role assignments.
- Owner transfer is deferred until a dedicated ownership-transfer flow is documented. Until then, `owner_user_id` does not change after workspace creation.
- The owner cannot be directly removed and cannot self-remove in v1 because that would leave the workspace without its canonical authority.
- Membership is the authority for workspace access. `chat` must not own or infer workspace membership.
- Channel names are unique only within a workspace and are not globally unique.
- If `CreateChannel.position` is omitted, workspace appends the channel at the next available ordering value. Duplicate explicit positions are rejected in v1, and full channel reordering is deferred.
- Invitation redemption creates membership durably before downstream projections converge.
- When invitation acceptance creates membership, `added_by_user_id` is recorded as the invitation issuer.
- Roles are workspace-scoped and never shared across workspaces.
- `member_count` counts active memberships only. `channel_count` counts current `workspace_channel` rows in v1 because archive/delete flows are not yet defined here.
- Mutable workspace and channel update events are deferred until a later revision; this v1 document set focuses on create, membership, invitation, and channel-create flows.
