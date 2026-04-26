use relay_proto::bootstrap::{
    AppWorkspaceItem, GetAppBootstrapRequest, GetAppBootstrapResponse, UserCard,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use tonic::{Request, Response, Status};
use tracing::error;

use super::handler::Handler;
use crate::entity::{user_app_projection, workspace_projection};
use relay_types::actor_user_id;

impl Handler {
    pub(super) async fn get_app_bootstrap(
        &self,
        request: Request<GetAppBootstrapRequest>,
    ) -> Result<Response<GetAppBootstrapResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;

        let viewer = user_app_projection::Entity::find_by_id(actor_user_id)
            .one(&self.connection)
            .await
            .map_err(|e| {
                error!(error = %e, "GetAppBootstrap viewer lookup failed");
                Status::internal("Internal Server Error")
            })?;

        let workspaces = workspace_projection::Entity::find()
            .filter(workspace_projection::Column::UserId.eq(actor_user_id))
            .order_by_asc(workspace_projection::Column::WorkspaceName)
            .order_by_asc(workspace_projection::Column::WorkspaceId)
            .all(&self.connection)
            .await
            .map_err(|e| {
                error!(error = %e, "GetAppBootstrap workspace lookup failed");
                Status::internal("Internal Server Error")
            })?;

        let pending_friend_request_count = viewer
            .as_ref()
            .map(|viewer| viewer.pending_friend_request_count)
            .unwrap_or_default();

        let viewer = viewer.map_or_else(
            || UserCard {
                user_id: actor_user_id.to_string(),
                username: String::new(),
                display_name: String::new(),
                avatar_url: None,
            },
            |viewer| UserCard {
                user_id: viewer.user_id.to_string(),
                username: viewer.username,
                display_name: viewer.display_name,
                avatar_url: viewer.avatar_url,
            },
        );

        let workspaces = workspaces
            .into_iter()
            .map(|workspace| AppWorkspaceItem {
                workspace_id: workspace.workspace_id.to_string(),
                name: workspace.workspace_name,
                icon_url: workspace.workspace_icon_url,
                unread_count: workspace.unread_count,
            })
            .collect();

        Ok(Response::new(GetAppBootstrapResponse {
            viewer: Some(viewer),
            workspaces,
            pending_friend_request_count,
        }))
    }
}
