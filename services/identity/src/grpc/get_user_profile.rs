use relay_proto::identity::{GetUserProfileRequest, GetUserProfileResponse};
use sea_orm::EntityTrait;
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::entity::user_profile;

use super::handler::{Handler, actor_user_id};

impl Handler {
    pub(super) async fn get_user_profile(
        &self,
        request: Request<GetUserProfileRequest>,
    ) -> Result<Response<GetUserProfileResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let GetUserProfileRequest { user_id } = request.into_inner();
        let user_id = match user_id {
            Some(user_id) => Uuid::parse_str(&user_id)
                .map_err(|_| Status::invalid_argument("invalid user_id"))?,
            None => actor_user_id,
        };

        if user_id != actor_user_id {
            return Err(Status::permission_denied(
                "cross-user profile lookup is not allowed on this route",
            ));
        }

        let profile = user_profile::Entity::find_by_id(user_id)
            .one(&self.connection)
            .await
            .map_err(|e| {
                error!(error = %e, "identity get user profile lookup failed");
                Status::internal("internal server error")
            })?
            .ok_or_else(|| Status::not_found("user profile not found"))?;

        Ok(Response::new(GetUserProfileResponse {
            user_id: profile.user_id.to_string(),
            username: profile.username,
            display_name: profile.display_name,
            avatar_url: profile.avatar_url,
        }))
    }
}
