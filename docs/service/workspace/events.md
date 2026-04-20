## Publication Model

Workspace publishes integration events by inserting service-owned rows into `outbox_event` inside the same transaction as the source workspace write. The shared outbox worker later publishes those rows to RabbitMQ.

## Published Events

### `WorkspaceCreated`

**When published**

- After a new workspace and its creator membership are committed successfully.
- After a new workspace, creator membership, and seeded first channel are committed successfully.

**Minimum payload**

- `workspace_id`
- `name`
- `owner_user_id`
- `created_at`
- `initial_member_user_id`

**Typical consumers**

- `bootstrap` workspace projections
- Audit or analytics workflows
- Also emits `WorkspaceMemberAdded` for creator membership and `WorkspaceChannelCreated` for seeded first channel.

### `WorkspaceMemberAdded`

**When published**

- After a new active workspace membership is committed through direct add or invitation acceptance.

**Minimum payload**

- `workspace_id`
- `user_id`
- `joined_at`
- `added_by_user_id`
- `source` with contract values such as `direct_add` or `invitation_accept`

**Typical consumers**

- `bootstrap` member-visible workspace and channel projections
- `realtime` connected-client refresh workflows

### `WorkspaceMemberRemoved`

**When published**

- After an active membership is removed or deactivated.

**Minimum payload**

- `workspace_id`
- `user_id`
- `removed_at`
- `removed_by_user_id`
- `reason` with contract values such as `left`, `removed_by_admin`, or `workspace_deleted` in later revisions

**Typical consumers**

- `bootstrap` projection cleanup
- `realtime` connected-client eviction or access-refresh workflows

### `WorkspaceInvitationIssued`

**When published**

- After a new pending invitation is committed.

**Minimum payload**

- `workspace_invitation_id`
- `workspace_id`
- `issued_to_user_id`
- `issued_by_user_id`
- `workspace_name_snapshot`
- `inviter_display_name_snapshot`
- `expires_at`
- `created_at`

**Typical consumers**

- App notification workflows
- Audit workflows

### `WorkspaceInvitationAccepted`

**When published**

- After a pending invitation is accepted and the matching membership is committed.

**Minimum payload**

- `workspace_invitation_id`
- `workspace_id`
- `user_id`
- `accepted_at`
- `joined_at`

**Typical consumers**

- Audit workflows
- Notification workflows that need durable join confirmation

### `WorkspaceChannelCreated`

**When published**

- After a new workspace channel metadata row is committed.

**Minimum payload**

- `channel_id`
- `workspace_id`
- `name`
- `channel_kind`
- `position`
- `created_by_user_id`
- `created_at`

**Typical consumers**

- `bootstrap` sidebar channel projections
- `realtime` connected-client sidebar refresh workflows

### `WorkspaceInviteLinkCreated`

**When published**

- After a new bearer invite link is committed.

**Minimum payload**

- `workspace_invite_link_id`
- `workspace_id`
- `code`
- `created_by_user_id`
- `status`
- `expires_at`
- `max_uses`
- `use_count`
- `created_at`

**Typical consumers**

- Audit workflows
- Admin or UI surfaces that list active join links

### `WorkspaceInviteLinkRevoked`

**When published**

- After a bearer invite link is revoked.

**Minimum payload**

- `workspace_invite_link_id`
- `workspace_id`
- `status`
- `revoked_at`

**Typical consumers**

- Audit workflows
- UI surfaces that list active join links

## Event Rules

- Event payloads use workspace-owned IDs plus identity-owned `user_id` references only.
- `WorkspaceCreated` is the durable create signal for a workspace and its creator-visible membership bootstrap.
- `WorkspaceMemberAdded` is the durable membership grant signal regardless of whether the source was direct add or invitation acceptance.
- When invitation acceptance creates membership, `WorkspaceMemberAdded.added_by_user_id` is the invitation issuer so downstream consumers can treat the membership source consistently.
- `WorkspaceInvitationAccepted` does not replace `WorkspaceMemberAdded`; downstream consumers that materialize member-visible access should key off `WorkspaceMemberAdded`.
- `WorkspaceInvitationIssued` must be self-contained enough for downstream app UI rendering and audit, including workspace and inviter snapshots.
- `WorkspaceInvitationIssued` is for per-user app invites only. Join-link delivery belongs to `workspace_invite_link`.
- `WorkspaceInviteLinkCreated` and `WorkspaceInviteLinkRevoked` cover bearer join links; `WorkspaceMemberAdded` still represents successful join after link redemption.
- `WorkspaceMemberRemoved` must not be emitted for the owner in v1 because owner transfer and owner removal are deferred.
- Publication ordering should be preserved per workspace so membership and channel consumers converge predictably.
- Consumers must be idempotent because replay and duplicate delivery are expected platform behaviors.
- This v1 event set does not yet define mutable metadata update events such as `WorkspaceUpdated` or `WorkspaceChannelUpdated`; downstream docs that anticipate them should treat them as later additions.
