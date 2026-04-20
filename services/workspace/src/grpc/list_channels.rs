use relay_proto::workspace::{ChannelSummary, ListChannelsRequest, ListChannelsResponse};
use sea_orm::{
    ColumnTrait, EntityTrait, QueryFilter, QueryOrder, TransactionError, TransactionTrait,
};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::entity::{workspace, workspace_channel, workspace_member};

use super::handler::Handler;
use relay_types::{actor_user_id, to_timestamp};

impl Handler {
    pub(super) async fn list_channels(
        &self,
        request: Request<ListChannelsRequest>,
    ) -> Result<Response<ListChannelsResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let ListChannelsRequest { workspace_id } = request.into_inner();

        let workspace_id =
            Uuid::parse_str(&workspace_id).map_err(|_| Status::invalid_argument("Invalid UUID"))?;

        let response = self
            .connection
            .transaction::<_, Response<ListChannelsResponse>, Status>(|txn| {
                Box::pin(async move {
                    // Check for valid workspace
                    let workspace = workspace::Entity::find_by_id(workspace_id)
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace lookup failed");
                            Status::internal("Internal Server Error")
                        })?;
                    if workspace.is_none() {
                        return Err(Status::not_found("Workspace not found"));
                    };

                    // Check if workspace member
                    let member = workspace_member::Entity::find()
                        .filter(workspace_member::Column::UserId.eq(actor_user_id))
                        .filter(workspace_member::Column::WorkspaceId.eq(workspace_id))
                        .filter(workspace_member::Column::MembershipStatus.eq("active"))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace member lookup failed");
                            Status::internal("Internal Server Error")
                        })?;
                    if member.is_none() {
                        return Err(Status::not_found("Workspace not found"));
                    };

                    let channels = workspace_channel::Entity::find()
                        .filter(workspace_channel::Column::WorkspaceId.eq(workspace_id))
                        .filter(workspace_channel::Column::ArchivedAt.is_null())
                        .order_by_asc(workspace_channel::Column::Position)
                        .order_by_asc(workspace_channel::Column::ChannelId)
                        .all(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace channel lookup failed");
                            Status::internal("Internal Server Error")
                        })?;

                    let mut summary = Vec::with_capacity(channels.len());
                    for channel in channels {
                        summary.push(ChannelSummary {
                            channel_id: channel.channel_id.to_string(),
                            name: channel.name,
                            channel_kind: channel.channel_kind,
                            position: channel.position,
                            created_at: Some(to_timestamp(channel.created_at.into())),
                        });
                    }

                    Ok(Response::new(ListChannelsResponse { channels: summary }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "List channels transaction connection failure");
                    Status::internal("Internal Server Error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}
