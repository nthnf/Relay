use relay_proto::identity::identity_service_server::{IdentityService, IdentityServiceServer};
use relay_proto::identity::{
    AuthenticatePasswordRequest, GetUserProfileRequest, GetUserProfileResponse,
    GetUsersByIdsRequest, GetUsersByIdsResponse, RedeemEmailVerificationTokenRequest,
    RefreshSessionRequest, RegisterUserRequest, RegisterUserResponse,
    ResendVerificationEmailRequest, ResendVerificationEmailResponse, RevokeSessionRequest,
    RevokeSessionResponse, TokenPairResponse, UpdateUserProfileRequest, UpdateUserProfileResponse,
};
use sea_orm::DatabaseConnection;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::auth::AuthKeys;

pub(super) const EMAIL_NORMALIZED_CONSTRAINT: &str = "uq-user-account-email-normalized";
pub(super) const USERNAME_CONSTRAINT: &str = "uq-user-profile-username";
pub(super) const ACTOR_USER_ID_METADATA: &str = "x-user-id";

pub(super) fn to_timestamp(dt: chrono::DateTime<chrono::Utc>) -> prost_types::Timestamp {
    prost_types::Timestamp {
        seconds: dt.timestamp(),
        nanos: dt.timestamp_subsec_nanos() as i32,
    }
}

pub(super) fn payload_value<T: serde::Serialize>(payload: T) -> serde_json::Value {
    serde_json::to_value(payload).expect("event payload should serialize")
}

pub(super) fn actor_user_id<T>(request: &Request<T>) -> Result<Uuid, Status> {
    let raw = request
        .metadata()
        .get(ACTOR_USER_ID_METADATA)
        .ok_or_else(|| Status::unauthenticated("missing authenticated actor context"))?;

    let raw = raw
        .to_str()
        .map_err(|_| Status::unauthenticated("invalid authenticated actor context"))?;

    Uuid::parse_str(raw).map_err(|_| Status::unauthenticated("invalid authenticated actor context"))
}

pub struct Handler {
    pub(crate) connection: DatabaseConnection,
    pub(crate) auth: AuthKeys,
}

impl Handler {
    pub fn new(connection: DatabaseConnection, auth: AuthKeys) -> Self {
        Self { connection, auth }
    }

    pub fn into_server(self) -> IdentityServiceServer<Self> {
        IdentityServiceServer::new(self)
    }
}

#[tonic::async_trait]
impl IdentityService for Handler {
    async fn register_user(
        &self,
        request: Request<RegisterUserRequest>,
    ) -> Result<Response<RegisterUserResponse>, Status> {
        self.register_user(request).await
    }

    async fn authenticate_password(
        &self,
        request: Request<AuthenticatePasswordRequest>,
    ) -> Result<Response<TokenPairResponse>, Status> {
        self.authenticate_password(request).await
    }

    async fn refresh_session(
        &self,
        request: Request<RefreshSessionRequest>,
    ) -> Result<Response<TokenPairResponse>, Status> {
        self.refresh_session(request).await
    }

    async fn revoke_session(
        &self,
        request: Request<RevokeSessionRequest>,
    ) -> Result<Response<RevokeSessionResponse>, Status> {
        self.revoke_session(request).await
    }

    async fn redeem_email_verification_token(
        &self,
        request: Request<RedeemEmailVerificationTokenRequest>,
    ) -> Result<Response<TokenPairResponse>, Status> {
        self.redeem_email_verification_token(request).await
    }

    async fn resend_verification_email(
        &self,
        request: Request<ResendVerificationEmailRequest>,
    ) -> Result<Response<ResendVerificationEmailResponse>, Status> {
        self.resend_verification_email(request).await
    }

    async fn update_user_profile(
        &self,
        request: Request<UpdateUserProfileRequest>,
    ) -> Result<Response<UpdateUserProfileResponse>, Status> {
        self.update_user_profile(request).await
    }

    async fn get_user_profile(
        &self,
        request: Request<GetUserProfileRequest>,
    ) -> Result<Response<GetUserProfileResponse>, Status> {
        self.get_user_profile(request).await
    }

    async fn get_users_by_ids(
        &self,
        request: Request<GetUsersByIdsRequest>,
    ) -> Result<Response<GetUsersByIdsResponse>, Status> {
        self.get_users_by_ids(request).await
    }
}
