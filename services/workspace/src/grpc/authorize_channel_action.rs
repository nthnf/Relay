use relay_proto::workspace::{
    AuthorizeChannelActionRequest, AuthorizeChannelActionResponse, ChannelAction,
};
use tonic::{Request, Response, Status};
use uuid::Uuid;

use super::handler::Handler;
use super::lib::{actor_can_access_channel, channel_action_permission};
use relay_types::actor_user_id;

impl Handler {
    pub(super) async fn authorize_channel_action(
        &self,
        request: Request<AuthorizeChannelActionRequest>,
    ) -> Result<Response<AuthorizeChannelActionResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let AuthorizeChannelActionRequest {
            workspace_id,
            channel_id,
            action,
        } = request.into_inner();

        let workspace_id =
            Uuid::parse_str(&workspace_id).map_err(|_| Status::invalid_argument("Invalid UUID"))?;
        let channel_id =
            Uuid::parse_str(&channel_id).map_err(|_| Status::invalid_argument("Invalid UUID"))?;
        let action = ChannelAction::try_from(action)
            .map_err(|_| Status::invalid_argument("Invalid channel action"))?;
        let required_permission = channel_action_permission(action)?;

        let allowed = actor_can_access_channel(
            &self.connection,
            actor_user_id,
            workspace_id,
            channel_id,
            required_permission,
        )
        .await?;

        Ok(Response::new(AuthorizeChannelActionResponse { allowed }))
    }
}
