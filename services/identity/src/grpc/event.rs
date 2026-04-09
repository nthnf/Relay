use serde::Serialize;

#[derive(Serialize)]
pub struct UserRegisteredPayload {
    pub user_id: String,
    pub email: String,
    pub email_verified: bool,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub registered_at: String,
}

#[derive(Serialize)]
pub struct VerificationEmailRequestedPayload {
    pub user_id: String,
    pub email: String,
    pub verification_token: String,
    pub verification_token_expires_at: String,
    pub verification_token_id: String,
    pub reason: String,
    pub requested_at: String,
}

#[derive(Serialize)]
pub struct SessionRevokedPayload {
    pub session_id: String,
    pub user_id: String,
    pub revoked_at: String,
    pub revoke_reason: String,
}

#[derive(Serialize)]
pub struct UserEmailVerifiedPayload {
    pub user_id: String,
    pub email: String,
    pub email_verified_at: String,
}

#[derive(Serialize)]
pub struct UserProfileUpdatedPayload {
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub updated_at: String,
}
