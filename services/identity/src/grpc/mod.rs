pub mod authenticate_password;
pub mod check;
pub mod get_user_profile;
pub mod get_users_by_ids;
pub mod handler;
pub mod redeem_email_verification_token;
pub mod refresh_session;
pub mod register_user;
pub mod resend_verification_email;
pub mod revoke_session;
pub mod update_user_profile;

pub use handler::Handler as IdentityServer;
