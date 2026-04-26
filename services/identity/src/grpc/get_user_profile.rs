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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthKeys;
    use sea_orm::{DbBackend, MockDatabase};
    use tonic::Request;
    use uuid::Uuid;

    fn get_user_profile_request(
        actor_user_id: Option<Uuid>,
        user_id: Option<Uuid>,
    ) -> Request<GetUserProfileRequest> {
        let mut request = Request::new(GetUserProfileRequest {
            user_id: user_id.map(|user_id| user_id.to_string()),
        });

        if let Some(actor_user_id) = actor_user_id {
            request.metadata_mut().insert(
                relay_types::ACTOR_USER_ID_METADATA,
                actor_user_id
                    .to_string()
                    .parse()
                    .expect("user id metadata should be valid"),
            );
        }

        request
    }

    fn test_service() -> Handler {
        Handler {
            connection: MockDatabase::new(DbBackend::Postgres).into_connection(),
            auth: AuthKeys::from_shared_secret(b"test-secret-key"),
        }
    }

    #[tokio::test]
    async fn get_user_profile_rejects_cross_user_lookup_on_actor_route() {
        let actor_user_id = Uuid::new_v4();
        let other_user_id = Uuid::new_v4();

        let error = test_service()
            .get_user_profile(get_user_profile_request(
                Some(actor_user_id),
                Some(other_user_id),
            ))
            .await
            .expect_err("cross-user profile lookup should be denied");

        assert_eq!(error.code(), tonic::Code::PermissionDenied);
        assert_eq!(
            error.message(),
            "cross-user profile lookup is not allowed on this route"
        );
    }
}
