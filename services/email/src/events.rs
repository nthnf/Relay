use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct VerificationEmailRequested {
    pub user_id: String,
    pub email: String,
    pub verification_token: String,
    pub verification_token_id: String,
    pub verification_token_expires_at: String,
    pub reason: String,
    pub requested_at: String,
}

#[derive(Debug, Deserialize)]
pub struct WorkspaceInvitationIssued {
    pub workspace_id: String,
    pub workspace_invitation_id: String,
    pub workspace_name_snapshot: String,
    pub issued_by_user_id: String,
    pub inviter_display_name_snapshot: String,
    pub invitee_email: String,
    pub expires_at: String,
    pub created_at: String,
}

#[derive(Debug)]
pub enum EmailEvent {
    VerificationEmailRequested(VerificationEmailRequested),
    WorkspaceInvitationIssued(WorkspaceInvitationIssued),
}
