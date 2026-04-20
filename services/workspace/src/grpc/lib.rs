use sea_orm::{DatabaseConnection, EntityTrait};
use tonic::Status;
use uuid::Uuid;

use crate::entity::user_snapshot;

#[allow(dead_code)]
pub(super) mod permission {
    use std::convert::TryFrom;

    use tonic::Status;

    pub const WORKSPACE_READ: u32 = 1 << 0;
    pub const MEMBER_ADD: u32 = 1 << 1;
    pub const MEMBER_REMOVE: u32 = 1 << 2;
    pub const MEMBER_INVITE: u32 = 1 << 3;
    pub const INVITE_LINK_CREATE: u32 = 1 << 4;
    pub const INVITE_LINK_REVOKE: u32 = 1 << 5;
    pub const CHANNEL_CREATE: u32 = 1 << 6;
    pub const CHANNEL_EDIT: u32 = 1 << 7;
    pub const CHANNEL_DELETE: u32 = 1 << 8;
    pub const ROLE_MANAGE: u32 = 1 << 9;
    pub const WORKSPACE_EDIT: u32 = 1 << 10;
    pub const WORKSPACE_DELETE: u32 = 1 << 11;

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
            | ROLE_MANAGE
            | WORKSPACE_EDIT
    }

    pub const fn member() -> u32 {
        WORKSPACE_READ
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
