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

#[derive(Debug)]
pub enum EmailEvent {
    VerificationEmailRequested(VerificationEmailRequested),
}
