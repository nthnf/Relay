use relay_proto::workspace::{GetWorkspaceRequest, GetWorkspaceResponse};
use sea_orm::{
    ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, TransactionError, TransactionTrait,
};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::entity::{workspace, workspace_channel, workspace_member};

use super::handler::Handler;
use relay_types::{actor_user_id, to_timestamp};

impl Handler {
    pub(super) async fn get_workspace(
        &self,
        request: Request<GetWorkspaceRequest>,
    ) -> Result<Response<GetWorkspaceResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let GetWorkspaceRequest { workspace_id } = request.into_inner();
        let workspace_id =
            Uuid::parse_str(&workspace_id).map_err(|_| Status::invalid_argument("Invalid UUID"))?;

        let response = self
            .connection
            .transaction::<_, Response<GetWorkspaceResponse>, Status>(|txn| {
                Box::pin(async move {
                    // Get workspace
                    let workspace = workspace::Entity::find_by_id(workspace_id)
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace lookup failed");
                            Status::internal("Internal Server Error")
                        })?;

                    let Some(workspace) = workspace else {
                        return Err(Status::not_found("Workspace not found"));
                    };
                    if workspace.archived_at.is_some() {
                        return Err(Status::not_found("Workspace not found"));
                    }

                    // Check actor is member
                    let member = workspace_member::Entity::find()
                        .filter(workspace_member::Column::WorkspaceId.eq(workspace_id))
                        .filter(workspace_member::Column::UserId.eq(actor_user_id))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace member lookup failed");
                            Status::internal("Internal Server Error")
                        })?;

                    // Return not found if not a member
                    let Some(member) = member else {
                        return Err(Status::not_found("Workspace not found"));
                    };
                    if member.membership_status != "active" {
                        return Err(Status::not_found("Workspace not found"));
                    }

                    // Count workspace channels
                    let channel_count = workspace_channel::Entity::find()
                        .filter(workspace_channel::Column::WorkspaceId.eq(workspace_id))
                        .count(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace channel count failed");
                            Status::internal("Internal Server Error")
                        })? as i32;

                    // Count workspace members
                    let member_count = workspace_member::Entity::find()
                        .filter(workspace_member::Column::WorkspaceId.eq(workspace_id))
                        .filter(workspace_member::Column::MembershipStatus.eq("active"))
                        .count(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace member count failed");
                            Status::internal("Internal Server Error")
                        })? as i32;

                    Ok(Response::new(GetWorkspaceResponse {
                        workspace_id: workspace_id.to_string(),
                        name: workspace.name,
                        owner_user_id: workspace.owner_user_id.to_string(),
                        member_count,
                        channel_count,
                        created_at: Some(to_timestamp(workspace.created_at.into())),
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "Get workspace transaction connection failure");
                    Status::internal("Internal Server Error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}
