## Persistence Scope

Workspace owns workspace-write state only: workspace metadata, memberships, roles, invitations, and channel metadata. Other services must use workspace gRPC or workspace events instead of reading these tables directly.

## Replicated User Model

### `user_snapshot`

Workspace-local copy of identity account existence, verification state, and profile snapshot.

| Column           | Type          | Notes                                              |
| ---------------- | ------------- | -------------------------------------------------- |
| `user_id`        | `uuid`        | Primary key. Stable identity-owned user reference. |
| `email_verified` | `bool`        | Mirrored from identity email verification state.   |
| `username`       | `text`        | Mirrored username snapshot.                        |
| `display_name`   | `text`        | Mirrored display name snapshot.                    |
| `avatar_url`     | `text null`   | Mirrored avatar snapshot.                          |
| `created_at`     | `timestamptz` | Mirror insert time.                                |
| `updated_at`     | `timestamptz` | Last mirrored update time.                         |

Semantic rules:

- Workspace uses this table only for target-user existence validation on write paths.
- `UserRegistered` seeds row with profile fields.
- `UserProfileUpdated` refreshes username/display_name/avatar_url.
- `UserEmailVerified` flips `email_verified` true.

## Core Tables

### `workspace`

One row per workspace container.

| Column          | Type               | Notes                                                |
| --------------- | ------------------ | ---------------------------------------------------- |
| `workspace_id`  | `uuid`             | Primary key. Service-owned workspace identifier.     |
| `owner_user_id` | `uuid`             | Identity-owned user reference for the creator in v1. |
| `name`          | `text`             | Workspace display name.                              |
| `icon_url`      | `text null`        | Optional mutable display field for later revisions.  |
| `created_at`    | `timestamptz`      | Workspace creation time.                             |
| `updated_at`    | `timestamptz`      | Last metadata update time.                           |
| `archived_at`   | `timestamptz null` | Null for active workspaces in v1.                    |

Semantic rules:

- `workspace_id` is generated and owned by workspace service.
- `owner_user_id` is an identity-owned `user_id` reference, not a foreign-key dependency on another service database.
- `CreateWorkspace` inserts the workspace row and the creator membership row in one transaction.
- V1 treats the creator as the first durable member and initial administrative authority.
- `owner_user_id` is the canonical ultimate authority for the workspace in v1. Workspace-scoped roles may delegate permissions to non-owner members, but the owner implicitly has all permissions whether or not they hold an explicit role assignment.
- Owner transfer is deferred until explicitly documented; v1 does not allow changing `owner_user_id` after workspace creation.

### `workspace_invite_link`

Bearer-style invite link that lets a user join without a pre-issued per-user invitation row.

| Column                     | Type               | Notes                                              |
| -------------------------- | ------------------ | -------------------------------------------------- |
| `workspace_invite_link_id` | `uuid`             | Primary key. Service-owned invite-link identifier. |
| `workspace_id`             | `uuid`             | Target workspace.                                  |
| `code`                     | `text`             | Opaque join token exposed in the URL. Unique.      |
| `created_by_user_id`       | `uuid`             | Identity-owned creator reference.                  |
| `status`                   | `text`             | Contract values: `active`, `revoked`, `expired`.   |
| `expires_at`               | `timestamptz null` | Optional hard expiry.                              |
| `max_uses`                 | `int null`         | Optional use cap.                                  |
| `use_count`                | `int`              | Current redemption count.                          |
| `created_at`               | `timestamptz`      | Link creation time.                                |
| `revoked_at`               | `timestamptz null` | Null while active.                                 |

Semantic rules:

- Join-link redemption creates membership without a preexisting per-user invitation row.
- `code` must be unique.
- `max_uses` and `expires_at` are optional controls for public invite links.
- `use_count` increments on successful redemption.
- `revoked_at` makes link unusable immediately.

### `workspace_member`

One row per user membership inside a workspace.

| Column              | Type               | Notes                                                                                                                                                                  |
| ------------------- | ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `workspace_id`      | `uuid`             | First half of the primary key. Workspace-owned reference.                                                                                                              |
| `user_id`           | `uuid`             | Second half of the primary key. Identity-owned member reference.                                                                                                       |
| `membership_status` | `text`             | Contract values: `active`, `removed`.                                                                                                                                  |
| `joined_at`         | `timestamptz`      | Time the membership became active.                                                                                                                                     |
| `removed_at`        | `timestamptz null` | Null while active.                                                                                                                                                     |
| `added_by_user_id`  | `uuid null`        | Identity-owned user recorded as the source of membership creation. For direct adds this is the acting member; for invitation acceptance this is the invitation issuer. |

Semantic rules:

- Membership uniqueness is `(workspace_id, user_id)`; workspace must never persist two active memberships for the same user in one workspace.
- `membership_status = removed` may be retained for audit or idempotency, but rejoin behavior must not create conflicting active rows.
- Every channel-visible or role-assignment-visible user must have an active membership row first.
- Non-owner members may self-remove in v1.
- The owner cannot self-remove and cannot be removed directly in v1 because owner transfer is not yet defined.
- `RemoveMember` marks or removes the membership and must also remove dependent member-role assignments in the same transaction.

### `workspace_role`

Workspace-scoped role definition.

| Column              | Type          | Notes                                                     |
| ------------------- | ------------- | --------------------------------------------------------- |
| `workspace_role_id` | `uuid`        | Primary key. Service-owned role identifier.               |
| `workspace_id`      | `uuid`        | Workspace that owns the role.                             |
| `name`              | `text`        | Role display name unique within the workspace.            |
| `permissions`       | `uint32`      | Bitmask. Append-only. |
| `is_system_role`    | `bool`        | True for seeded roles such as an owner/admin role.        |
| `created_at`        | `timestamptz` | Role creation time.                                       |

Semantic rules:

- Roles are scoped to one workspace and cannot be shared across workspaces.
- `name` should be unique per `workspace_id` so permission labels stay stable locally.
- V1 may seed system roles during workspace creation even if role-management RPCs are deferred.
- Roles are never the source of truth for ultimate ownership; they only delegate workspace-owned permissions to non-owner members.
- Owner role should carry every bit, including `workspace.delete`.
- Admin role should carry every bit except `workspace.delete`.
- Member role should carry only `workspace.read`.

Permission bits, low -> high:

| Bit | Permission |
| --- | --- |
| 0 | `workspace.read` |
| 1 | `member.add` |
| 2 | `member.remove` |
| 3 | `member.invite` |
| 4 | `invite_link.create` |
| 5 | `invite_link.revoke` |
| 6 | `channel.create` |
| 7 | `channel.edit` |
| 8 | `channel.delete` |
| 9 | `role.manage` |
| 10 | `workspace.edit` |
| 11 | `workspace.delete` |

`u32` is enough for 12 bits.

Rules:

- append only
- never reorder bits
- never reuse bit positions

### `workspace_member_role`

Join table assigning workspace roles to active members.

| Column                | Type          | Notes                                                       |
| --------------------- | ------------- | ----------------------------------------------------------- |
| `workspace_id`        | `uuid`        | Workspace scope, part of the composite key.                 |
| `user_id`             | `uuid`        | Identity-owned member reference, part of the composite key. |
| `workspace_role_id`   | `uuid`        | Workspace-owned role reference, part of the composite key.  |
| `assigned_at`         | `timestamptz` | Assignment time.                                            |
| `assigned_by_user_id` | `uuid null`   | Identity-owned actor who made the assignment.               |

Semantic rules:

- A role assignment is valid only when both the `workspace_member` row is active and the `workspace_role` row belongs to the same `workspace_id`.
- The composite uniqueness should prevent duplicate role assignments for the same `(workspace_id, user_id, workspace_role_id)`.
- Removing a member must delete or invalidate all matching `workspace_member_role` rows transactionally.

### `workspace_invitation`

Invitation record that grants a user a path into a workspace.

| Column                    | Type               | Notes                                                         |
| ------------------------- | ------------------ | ------------------------------------------------------------- |
| `workspace_invitation_id` | `uuid`             | Primary key. Service-owned invitation identifier.             |
| `workspace_id`            | `uuid`             | Target workspace.                                             |
| `issued_to_user_id`       | `uuid`             | Identity-owned target user reference.                         |
| `issued_by_user_id`       | `uuid`             | Identity-owned inviter reference.                             |
| `status`                  | `text`             | Contract values: `pending`, `accepted`, `expired`, `revoked`. |
| `expires_at`              | `timestamptz`      | Required invitation expiry timestamp.                         |
| `accepted_at`             | `timestamptz null` | Null until redeemed.                                          |
| `created_at`              | `timestamptz`      | Invitation issuance time.                                     |

Semantic rules:

- Invitation expiry is explicit: `AcceptInvitation` must reject `expired` invitations or pending rows whose `expires_at` is in the past.
- V1 allows at most one active pending invitation per `(workspace_id, issued_to_user_id)`.
- An invitation may only be accepted by `issued_to_user_id`.
- Accepting an invitation changes the invitation status and creates the membership in one transaction.
- When invitation acceptance creates membership, `workspace_member.added_by_user_id` is populated with `issued_by_user_id`.
- If the target user is already an active member, new invitation issuance should fail or return an idempotent conflict-style response rather than storing redundant pending rows.
- This table is for per-user invitations only. Discord-style bearer join links belong in `workspace_invite_link`.

### `workspace_channel`

Workspace-owned channel metadata row. Message data is not stored here.

| Column               | Type               | Notes                                                    |
| -------------------- | ------------------ | -------------------------------------------------------- |
| `channel_id`         | `uuid`             | Primary key. Service-owned channel identifier.           |
| `workspace_id`       | `uuid`             | Parent workspace reference.                              |
| `name`               | `text`             | Channel display name.                                    |
| `channel_kind`       | `text`             | Contract values such as `text` or later-supported kinds. |
| `position`           | `int`              | Ordering key within the workspace sidebar.               |
| `created_by_user_id` | `uuid`             | Identity-owned creator reference.                        |
| `created_at`         | `timestamptz`      | Channel creation time.                                   |
| `archived_at`        | `timestamptz null` | Null for active channels in v1.                          |

Semantic rules:

- Duplicate active channel names are allowed in v1.
- `position` ordering is scoped to the workspace, not global.
- If `position` is omitted on create, workspace assigns the next available ordering value for that workspace.
- If a create request provides a duplicate active `position` within the same workspace, workspace rejects it in v1.
- V1 does not define a reorder RPC; downstream sidebar consumers should treat `position` as a stable write-time ordering contract.
- Workspace owns channel metadata only; `chat` later owns message rows keyed by `channel_id`.
- Channel creation requires the actor to already be an active workspace member with sufficient workspace-owned permissions.

## Relations

- `workspace_member.workspace_id -> workspace.workspace_id`
- `workspace_role.workspace_id -> workspace.workspace_id`
- `workspace_member_role.workspace_id -> workspace.workspace_id`
- `workspace_member_role.(workspace_id, user_id) -> workspace_member.(workspace_id, user_id)`
- `workspace_member_role.workspace_role_id -> workspace_role.workspace_role_id`
- `workspace_invitation.workspace_id -> workspace.workspace_id`
- `workspace_channel.workspace_id -> workspace.workspace_id`
- `workspace.owner_user_id -> user_snapshot.user_id`
- `workspace_member.user_id -> user_snapshot.user_id`
- `workspace_member.added_by_user_id -> user_snapshot.user_id`
- `workspace_member_role.user_id -> user_snapshot.user_id`
- `workspace_member_role.assigned_by_user_id -> user_snapshot.user_id`
- `workspace_invitation.issued_to_user_id -> user_snapshot.user_id`
- `workspace_invitation.issued_by_user_id -> user_snapshot.user_id`
- `workspace_invite_link.created_by_user_id -> user_snapshot.user_id`
- `workspace_channel.created_by_user_id -> user_snapshot.user_id`

## Cross-Service References

- `identity` owns every `user_id` referenced by `owner_user_id`, `user_id`, `issued_to_user_id`, `issued_by_user_id`, `added_by_user_id`, `assigned_by_user_id`, and `created_by_user_id`.
- Workspace consumes identity `UserRegistered`, `UserProfileUpdated`, and `UserEmailVerified` events to maintain local `user_snapshot` mirror rows for write-path validation.
- Workspace validates write-path target users through local `user_snapshot` rows before issuing invitations or directly adding members.
- Invitation issuance stays app-scoped; workspace does not resolve recipient email for v1.
- Bearer invite links are separate from per-user invitations and do not require email delivery.
- External application servers call workspace through Envoy Gateway; workspace does not trust arbitrary caller-supplied actor identity and must authorize from Envoy-validated access-token context at its own boundary.
- `bootstrap` consumes `WorkspaceCreated`, `WorkspaceMemberAdded`, `WorkspaceMemberRemoved`, and `WorkspaceChannelCreated` to maintain member-visible workspace and channel projections.
- `realtime` consumes selected membership and channel events to update connected clients after durable publication.
- Workspace inserts durable integration events into its local `outbox_event` table in the same transaction as the source write.
