use relay_proto::workspace::ChannelAction;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use tonic::Status;
use uuid::Uuid;

use crate::entity::{
    user_snapshot, workspace, workspace_channel, workspace_member, workspace_member_role,
    workspace_role,
};

#[allow(dead_code)]
pub mod permission {
    use std::convert::TryFrom;

    use tonic::Status;

    pub const WORKSPACE_READ: u32 = 1 << 0;
    pub const MEMBER_ADD: u32 = 1 << 1;
    pub const MEMBER_REMOVE: u32 = 1 << 2;
    pub const MEMBER_INVITE: u32 = 1 << 3;
    pub const INVITE_LINK_CREATE: u32 = 1 << 4;
    pub const INVITE_LINK_REVOKE: u32 = 1 << 5;
    pub const CHANNEL_READ: u32 = 1 << 6;
    pub const CHANNEL_WRITE: u32 = 1 << 7;
    pub const CHANNEL_CREATE: u32 = 1 << 8;
    pub const CHANNEL_EDIT: u32 = 1 << 9;
    pub const CHANNEL_DELETE: u32 = 1 << 10;
    pub const ROLE_MANAGE: u32 = 1 << 11;
    pub const WORKSPACE_EDIT: u32 = 1 << 12;
    pub const WORKSPACE_DELETE: u32 = 1 << 13;

    pub fn mask(bits: &[u32]) -> u32 {
        bits.iter().copied().fold(0, |acc, bit| acc | bit)
    }

    pub fn has(perms: u32, bit: u32) -> bool {
        perms & bit != 0
    }

    pub fn has_all(perms: u32, bits: u32) -> bool {
        perms & bits == bits
    }

    pub fn from_db(perms: i32) -> Result<u32, Status> {
        u32::try_from(perms).map_err(|_| Status::internal("Invalid permissions"))
    }

    pub fn to_db(perms: u32) -> Result<i32, Status> {
        i32::try_from(perms).map_err(|_| Status::internal("Invalid permissions"))
    }

    pub const fn owner() -> u32 {
        WORKSPACE_READ
            | MEMBER_ADD
            | MEMBER_REMOVE
            | MEMBER_INVITE
            | INVITE_LINK_CREATE
            | INVITE_LINK_REVOKE
            | CHANNEL_CREATE
            | CHANNEL_EDIT
            | CHANNEL_DELETE
            | CHANNEL_READ
            | CHANNEL_WRITE
            | ROLE_MANAGE
            | WORKSPACE_EDIT
            | WORKSPACE_DELETE
    }

    pub const fn admin() -> u32 {
        WORKSPACE_READ
            | MEMBER_ADD
            | MEMBER_REMOVE
            | MEMBER_INVITE
            | INVITE_LINK_CREATE
            | INVITE_LINK_REVOKE
            | CHANNEL_CREATE
            | CHANNEL_EDIT
            | CHANNEL_DELETE
            | CHANNEL_READ
            | CHANNEL_WRITE
            | ROLE_MANAGE
            | WORKSPACE_EDIT
    }

    pub const fn member() -> u32 {
        WORKSPACE_READ | CHANNEL_READ | CHANNEL_WRITE
    }
}

pub(super) async fn user_account_exists(
    db: &DatabaseConnection,
    user_id: Uuid,
) -> Result<bool, Status> {
    let account = user_snapshot::Entity::find_by_id(user_id)
        .one(db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "friendship user account lookup failed");
            Status::internal("Internal Server Error")
        })?;

    Ok(account.is_some())
}

pub(super) fn channel_action_permission(action: ChannelAction) -> Result<u32, Status> {
    match action {
        ChannelAction::Read => Ok(permission::CHANNEL_READ),
        ChannelAction::Write => Ok(permission::CHANNEL_WRITE),
        ChannelAction::Unspecified => Err(Status::invalid_argument("Invalid channel action")),
    }
}

pub(super) async fn actor_can_access_channel(
    db: &DatabaseConnection,
    actor_user_id: Uuid,
    workspace_id: Uuid,
    channel_id: Uuid,
    required_permission: u32,
) -> Result<bool, Status> {
    let workspace = workspace::Entity::find_by_id(workspace_id)
        .one(db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Workspace lookup failed");
            Status::internal("Internal Server Error")
        })?;

    let Some(workspace) = workspace else {
        return Ok(false);
    };

    if workspace.archived_at.is_some() {
        return Ok(false);
    }

    let channel = workspace_channel::Entity::find_by_id(channel_id)
        .filter(workspace_channel::Column::WorkspaceId.eq(workspace_id))
        .filter(workspace_channel::Column::ArchivedAt.is_null())
        .one(db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Workspace channel lookup failed");
            Status::internal("Internal Server Error")
        })?;

    if channel.is_none() {
        return Ok(false);
    }

    let member = workspace_member::Entity::find()
        .filter(workspace_member::Column::UserId.eq(actor_user_id))
        .filter(workspace_member::Column::WorkspaceId.eq(workspace_id))
        .filter(workspace_member::Column::MembershipStatus.eq("active"))
        .one(db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Workspace member lookup failed");
            Status::internal("Internal Server Error")
        })?;

    if member.is_none() {
        return Ok(false);
    }

    if workspace.owner_user_id == actor_user_id {
        return Ok(true);
    }

    let role_joins = workspace_member_role::Entity::find()
        .find_also_related(workspace_role::Entity)
        .filter(workspace_member_role::Column::UserId.eq(actor_user_id))
        .filter(workspace_member_role::Column::WorkspaceId.eq(workspace_id))
        .all(db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Workspace member role lookup failed");
            Status::internal("Internal Server Error")
        })?;

    for (_, role) in role_joins {
        let Some(role) = role else {
            continue;
        };
        let perms = permission::from_db(role.permissions)?;
        if permission::has(perms, required_permission) {
            return Ok(true);
        }
    }

    Ok(false)
}
