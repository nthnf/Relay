use relay_proto::bootstrap::{DmThreadItem, GetDmBootstrapRequest, GetDmBootstrapResponse};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use tonic::{Request, Response, Status};
use tracing::error;

use super::handler::Handler;
use crate::entity::dm_projection;
use relay_types::{actor_user_id, to_timestamp};

impl Handler {
    pub(super) async fn get_dm_bootstrap(
        &self,
        request: Request<GetDmBootstrapRequest>,
    ) -> Result<Response<GetDmBootstrapResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;

        let items = dm_projection::Entity::find()
            .filter(dm_projection::Column::UserId.eq(actor_user_id))
            .filter(dm_projection::Column::ConversationId.is_not_null())
            .order_by_desc(dm_projection::Column::LastActivityAt)
            .order_by_asc(dm_projection::Column::ConversationId)
            .all(&self.connection)
            .await
            .map_err(|e| {
                error!(error = %e, "GetDmBootstrap lookup failed");
                Status::internal("Internal Server Error")
            })?
            .into_iter()
            .filter_map(|item| {
                item.conversation_id.map(|conversation_id| DmThreadItem {
                    conversation_id: conversation_id.to_string(),
                    dm_pair_id: item.dm_pair_id.to_string(),
                    peer_user_id: item.peer_user_id.to_string(),
                    peer_username: item.peer_username,
                    peer_display_name: item.peer_display_name,
                    peer_avatar_url: item.peer_avatar_url,
                    unread_count: item.unread_count,
                    last_message_preview: item.last_message_preview.unwrap_or_default(),
                    last_activity_at: item
                        .last_activity_at
                        .map(|last_activity_at| to_timestamp(last_activity_at.into())),
                })
            })
            .collect();

        Ok(Response::new(GetDmBootstrapResponse { items }))
    }
}
