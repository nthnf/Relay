use relay_proto::bootstrap::{
    GetWorkspaceBootstrapRequest, GetWorkspaceBootstrapResponse, WorkspaceBootstrapChannel,
    WorkspaceBootstrapHeader,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use super::handler::Handler;
use crate::entity::{workspace_channel_projection, workspace_projection};
use relay_types::actor_user_id;

impl Handler {
    pub(super) async fn get_workspace_bootstrap(
        &self,
        request: Request<GetWorkspaceBootstrapRequest>,
    ) -> Result<Response<GetWorkspaceBootstrapResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let GetWorkspaceBootstrapRequest { workspace_id } = request.into_inner();
        let workspace_id = Uuid::parse_str(&workspace_id)
            .map_err(|_| Status::invalid_argument("Invalid workspace ID"))?;

        let workspace = workspace_projection::Entity::find_by_id((actor_user_id, workspace_id))
            .one(&self.connection)
            .await
            .map_err(|e| {
                error!(error = %e, "GetWorkspaceBootstrap workspace lookup failed");
                Status::internal("Internal Server Error")
            })?
            .ok_or_else(|| Status::not_found("Workspace not found"))?;

        let channels = workspace_channel_projection::Entity::find()
            .filter(workspace_channel_projection::Column::UserId.eq(actor_user_id))
            .filter(workspace_channel_projection::Column::WorkspaceId.eq(workspace_id))
            .filter(workspace_channel_projection::Column::ConversationId.is_not_null())
            .order_by_asc(workspace_channel_projection::Column::Position)
            .order_by_asc(workspace_channel_projection::Column::ChannelId)
            .all(&self.connection)
            .await
            .map_err(|e| {
                error!(error = %e, "GetWorkspaceBootstrap channel lookup failed");
                Status::internal("Internal Server Error")
            })?
            .into_iter()
            .filter_map(|channel| {
                channel
                    .conversation_id
                    .map(|conversation_id| WorkspaceBootstrapChannel {
                        channel_id: channel.channel_id.to_string(),
                        conversation_id: conversation_id.to_string(),
                        name: channel.channel_name,
                        channel_kind: channel.channel_kind,
                        position: channel.position,
                        unread_count: channel.unread_count,
                    })
            })
            .collect();

        Ok(Response::new(GetWorkspaceBootstrapResponse {
            workspace: Some(WorkspaceBootstrapHeader {
                workspace_id: workspace.workspace_id.to_string(),
                name: workspace.workspace_name,
                icon_url: workspace.workspace_icon_url,
                member_count: workspace.member_count,
                unread_count: workspace.unread_count,
            }),
            channels,
        }))
    }
}
