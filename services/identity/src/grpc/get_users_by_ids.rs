use std::collections::HashMap;

use relay_proto::identity::{GetUsersByIdsRequest, GetUsersByIdsResponse, UserProfile};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::entity::user_profile;

use super::handler::Handler;

impl Handler {
    pub(super) async fn get_users_by_ids(
        &self,
        request: Request<GetUsersByIdsRequest>,
    ) -> Result<Response<GetUsersByIdsResponse>, Status> {
        let GetUsersByIdsRequest { user_ids } = request.into_inner();
        let parsed_user_ids = user_ids
            .iter()
            .map(|user_id| {
                Uuid::parse_str(user_id)
                    .map_err(|_| Status::invalid_argument(format!("invalid user_id: {}", user_id)))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let profiles = if parsed_user_ids.is_empty() {
            Vec::new()
        } else {
            user_profile::Entity::find()
                .filter(user_profile::Column::UserId.is_in(parsed_user_ids.clone()))
                .all(&self.connection)
                .await
                .map_err(|e| {
                    error!(error = %e, "identity get users by ids lookup failed");
                    Status::internal("internal server error")
                })?
        };

        let profiles_by_id = profiles
            .into_iter()
            .map(|profile| (profile.user_id, profile))
            .collect::<HashMap<_, _>>();

        let missing_ids = parsed_user_ids
            .iter()
            .filter(|user_id| !profiles_by_id.contains_key(user_id))
            .map(ToString::to_string)
            .collect::<Vec<_>>();

        if !missing_ids.is_empty() {
            return Err(Status::not_found(format!(
                "user profile not found for user_ids: {}",
                missing_ids.join(", ")
            )));
        }

        let users = parsed_user_ids
            .into_iter()
            .filter_map(|user_id| profiles_by_id.get(&user_id))
            .map(|profile| UserProfile {
                user_id: profile.user_id.to_string(),
                username: profile.username.clone(),
                display_name: profile.display_name.clone(),
                avatar_url: profile.avatar_url.clone(),
            })
            .collect();

        Ok(Response::new(GetUsersByIdsResponse { users }))
    }
}
