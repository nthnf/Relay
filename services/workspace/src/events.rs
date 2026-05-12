use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct WorkspaceCreatedPayload {
    pub workspace_id: String,
    pub name: String,
    pub owner_user_id: String,
    pub created_at: String,
    pub initial_member_user_id: String,
}

#[derive(Serialize)]
pub struct WorkspaceUpdatedPayload {
    pub workspace_id: String,
    pub name: String,
    pub icon_url: Option<String>,
    pub updated_by_user_id: String,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct WorkspaceDeletedPayload {
    pub workspace_id: String,
    pub deleted_by_user_id: String,
    pub deleted_at: String,
}

#[derive(Serialize)]
pub struct WorkspaceMemberAddedPayload {
    pub workspace_id: String,
    pub user_id: String,
    pub joined_at: String,
    pub added_by_user_id: String,
    pub source: String,
}

#[derive(Serialize)]
pub struct WorkspaceMemberRemovedPayload {
    pub workspace_id: String,
    pub user_id: String,
    pub removed_at: String,
    pub removed_by_user_id: String,
    pub reason: String,
}

#[derive(Serialize)]
pub struct WorkspaceInvitationIssuedPayload {
    pub workspace_invitation_id: String,
    pub workspace_id: String,
    pub issued_to_user_id: String,
    pub issued_by_user_id: String,
    pub workspace_name_snapshot: String,
    pub inviter_display_name_snapshot: String,
    pub expires_at: String,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct WorkspaceInvitationAcceptedPayload {
    pub workspace_invitation_id: String,
    pub workspace_id: String,
    pub user_id: String,
    pub accepted_at: String,
    pub joined_at: String,
}

#[derive(Serialize)]
pub struct WorkspaceChannelCreatedPayload {
    pub channel_id: String,
    pub workspace_id: String,
    pub name: String,
    pub channel_kind: String,
    pub position: i32,
    pub created_by_user_id: String,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct WorkspaceInviteLinkCreatedPayload {
    pub workspace_invite_link_id: String,
    pub workspace_id: String,
    pub code: String,
    pub created_by_user_id: String,
    pub status: String,
    pub expires_at: Option<String>,
    pub max_uses: Option<i32>,
    pub use_count: i32,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct WorkspaceInviteLinkRevokedPayload {
    pub workspace_invite_link_id: String,
    pub workspace_id: String,
    pub status: String,
    pub revoked_at: String,
}

#[derive(Deserialize)]
pub struct UserRegisteredPayload {
    pub user_id: String,
    pub email: String,
    pub email_verified: bool,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub registered_at: String,
}

#[derive(Deserialize)]
pub struct UserEmailVerifiedPayload {
    pub user_id: String,
    pub email: String,
    pub email_verified_at: String,
}

#[derive(Deserialize)]
pub struct UserProfileUpdatedPayload {
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub updated_at: String,
}
