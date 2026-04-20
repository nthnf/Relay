## gRPC Service Scope

Workspace exposes synchronous workspace, membership, invitation, and channel-metadata commands plus bounded owner reads. External application servers through Envoy Gateway are the primary callers for end-user create, invite, join, member, and channel flows. These calls are authoritative access-control and membership decisions, not chat-like low-latency fanout.

## Shared Contract Rules

- Authenticated actor identity is derived from Envoy-validated access-token context; callers must not be allowed to mutate another actor's workspace state by supplying arbitrary user IDs.
- Envoy calls identity `Authorization/Check` on protected routes, then forwards trusted actor headers such as `x-user-id` and `x-session-id` out-of-band.
- External application callers do not supply actor identity in request payloads for end-user actions; the transport boundary or a trusted backend caller context attaches it out-of-band.
- Workspace trusts Envoy-provided actor headers and does not repeat actor existence validation on every RPC.
- Workspace enforces workspace-local authorization at its own boundary using membership and role state it owns.
- `owner_user_id` is the canonical ultimate authority for the workspace in v1. Role assignments delegate permissions to non-owner members, but the owner implicitly has all permissions regardless of explicit role assignment.
- Owner transfer is not supported in v1, so the owner cannot be removed or self-remove through existing RPCs.
- Workspace validates write-path target-user IDs with local `user_snapshot` rows before issuing invitations or directly adding a member.
- Per-user invitations are app-scoped in v1; no email delivery is required from workspace.
- Membership is the source of truth for access to workspace channels; `chat` does not decide who belongs to a workspace.
- Domain writes and matching `outbox_event` inserts happen in the same transaction.

### `CreateWorkspace`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `name` (`string`) - workspace display name.
- `first_channel_name` (`string`) - required initial channel name. Kind is fixed to `text` and position is fixed to `1`.

**Response fields**

- `workspace_id` (`uuid`)
- `name` (`string`)
- `owner_user_id` (`uuid`)
- `created_at` (`timestamp`)
- `initial_member_user_id` (`uuid`)
- `first_channel_id` (`uuid`)

**Contract notes**

- Create the `workspace` row and the creator's `workspace_member` row in one transaction.
- Create initial `workspace_channel` row in same transaction with `channel_kind = text`, `position = 1`, and return its id.
- Seed required system roles for the creator in the same transaction when role bootstrapping is enabled. Owner role should include all permission bits, including `workspace.delete`.
- Insert a matching `WorkspaceCreated` outbox row before commit.

### `GetWorkspace`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `workspace_id` (`uuid`)

**Response fields**

- `workspace_id` (`uuid`)
- `name` (`string`)
- `owner_user_id` (`uuid`)
- `member_count` (`int32`) - count of active memberships only.
- `channel_count` (`int32`) - count of current `workspace_channel` rows in v1.
- `created_at` (`timestamp`)

**Contract notes**

- Return only if the authenticated actor is an active member of the workspace.
- This is a bounded owner read from workspace-owned tables, not a cross-domain aggregate.

### `ListWorkspacesForUser`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `page_size` (`int32 optional`)
- `page_token` (`string optional`)

**Response fields**

- `workspaces` (`repeated message`) with `workspace_id`, `name`, `member_count`, `channel_count`, `joined_at`
- `next_page_token` (`string optional`)

**Contract notes**

- Return only active memberships for the authenticated actor.
- `member_count` counts active memberships only; `channel_count` counts current `workspace_channel` rows in v1.
- Ordering should be stable and documented by implementation, even if `bootstrap` later becomes the main UI query path.

### `CreateChannel`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `workspace_id` (`uuid`)
- `name` (`string`)
- `channel_kind` (`string`)
- `position` (`int32 optional`) - omitted means append using the next available ordering value.

**Response fields**

- `channel_id` (`uuid`)
- `workspace_id` (`uuid`)
- `name` (`string`)
- `channel_kind` (`string`)
- `position` (`int32`)
- `created_at` (`timestamp`)

**Contract notes**

- Require the actor to be an active member with workspace-owned permission to create channels.
- Duplicate active channel names are allowed in v1.
- If `position` is omitted, assign the next available ordering value within the workspace.
- Reject duplicate active `position` values within the same workspace in v1.
- V1 does not define a reorder RPC; callers should treat returned positions as stable sidebar ordering metadata.
- Insert the `workspace_channel` row and matching `WorkspaceChannelCreated` outbox row in one transaction.

### `ListChannels`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `workspace_id` (`uuid`)

**Response fields**

- `channels` (`repeated message`) with `channel_id`, `name`, `channel_kind`, `position`, `created_at`

**Contract notes**

- Return only channels for workspaces where the authenticated actor has active membership.
- Order by `position` ascending, then `channel_id` as a stable tiebreaker.
- `position` values are expected to be unique among active channels within a workspace in v1 because duplicate create-time positions are rejected.
- This method returns metadata only; message previews and unread counts belong to `bootstrap` or `chat`-driven projections.

### `AddMember`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `workspace_id` (`uuid`)
- `target_user_id` (`uuid`)

**Response fields**

- `workspace_id` (`uuid`)
- `user_id` (`uuid`)
- `joined_at` (`timestamp`)
- `added_by_user_id` (`uuid`) - direct adds use the acting member; invitation acceptance uses the invitation issuer.

**Contract notes**

- Require the actor to have workspace-owned permission to add members directly.
- Require actor to be active workspace member.
- Validate `target_user_id` existence through local `user_snapshot` rows before inserting membership.
- Reject or return idempotent success when the target user is already an active member; do not create duplicate memberships.
- If the target user exists with `membership_status = removed`, reactivate the same row and recreate baseline member-role assignment.
- On success, create or reactivate `workspace_member` row and matching `WorkspaceMemberAdded` outbox row in one transaction.

### `RemoveMember`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `workspace_id` (`uuid`)
- `target_user_id` (`uuid`)

**Response fields**

- `removed` (`bool`)
- `workspace_id` (`uuid`)
- `user_id` (`uuid`)
- `removed_at` (`timestamp optional`)

**Contract notes**

 - Require the actor to have workspace-owned permission to remove another non-owner member.
- Non-owner members may self-remove in v1.
- Direct removal of the owner is disallowed in v1.
- Owner self-removal is disallowed in v1 because owner transfer is not yet defined.
- Remove or deactivate matching `workspace_member_role` rows in the same transaction as membership removal.
- This method is idempotent: if no active membership exists, return `removed = false`.
- Successful removal inserts a `WorkspaceMemberRemoved` outbox row.

### `IssueInvitation`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `workspace_id` (`uuid`)
- `target_user_id` (`uuid`)
- `expires_at` (`timestamp`)

**Response fields**

- `workspace_invitation_id` (`uuid`)
- `workspace_id` (`uuid`)
- `issued_to_user_id` (`uuid`)
- `issued_by_user_id` (`uuid`)
- `status` (`string`)
- `expires_at` (`timestamp`)
- `created_at` (`timestamp`)

**Contract notes**

- Require the actor to have workspace-owned permission to invite members.
- Validate `target_user_id` existence through local `user_snapshot` rows before inserting the invitation row.
- Reject if the target user is already an active member.
- Reject if an active pending invitation already exists for `(workspace_id, target_user_id)`.
- The emitted `WorkspaceInvitationIssued` event must include workspace and inviter snapshots needed by app UI consumers.
- On success, insert the `workspace_invitation` row and matching `WorkspaceInvitationIssued` outbox row in one transaction.

### `AcceptInvitation`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `workspace_invitation_id` (`uuid`)

**Response fields**

- `workspace_id` (`uuid`)
- `workspace_invitation_id` (`uuid`)
- `user_id` (`uuid`)
- `joined_at` (`timestamp`)
- `accepted_at` (`timestamp`)
- `added_by_user_id` (`uuid`) - set to the invitation issuer recorded on the accepted invitation.

**Contract notes**

- Only the invited user may accept the invitation.
- Reject if the invitation is not `pending` or if `expires_at` has passed.
- Reject or return idempotent success if the invited user is already an active member before acceptance.
- On success, mark the invitation accepted, create the `workspace_member` row with `added_by_user_id = issued_by_user_id`, and insert `WorkspaceInvitationAccepted` plus `WorkspaceMemberAdded` outbox rows in one transaction.

### `JoinWorkspaceByInviteLink`

**Main caller:** external application server through Envoy Gateway

**Request fields**

- `code` (`string`) - opaque bearer token from the invite link URL.

**Response fields**

- `workspace_id` (`uuid`)
- `workspace_invite_link_id` (`uuid`)
- `user_id` (`uuid`)
- `joined_at` (`timestamp`)
- `added_by_user_id` (`uuid`) - set to the joining user for bearer-link redemption.

**Contract notes**

- Any authenticated user with a valid invite-link code may redeem it.
- Reject if the link is not `active`, expired, or exhausted.
- Reject if the joining user is already an active member.
- If the joining user exists with `membership_status = removed`, reactivate the same row and recreate the baseline member-role assignment.
- On success, increment `workspace_invite_link.use_count`, set the link to `expired` when `max_uses` is reached, create or reactivate the `workspace_member` row with `added_by_user_id = joining user`, and insert `WorkspaceMemberAdded` in one transaction.
