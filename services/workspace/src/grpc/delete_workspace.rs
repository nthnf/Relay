use chrono::Utc;
use relay_proto::workspace::{DeleteWorkspaceRequest, DeleteWorkspaceResponse};
use sea_orm::{
    ActiveModelTrait, EntityTrait, IntoActiveModel, Set, TransactionError, TransactionTrait,
};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::{
    entity::{outbox_event, workspace},
    events::WorkspaceDeletedPayload,
};

use super::handler::Handler;
use relay_types::{actor_user_id, payload_value, to_timestamp};

impl Handler {
    pub(super) async fn delete_workspace(
        &self,
        request: Request<DeleteWorkspaceRequest>,
    ) -> Result<Response<DeleteWorkspaceResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let DeleteWorkspaceRequest { workspace_id } = request.into_inner();
        let workspace_id =
            Uuid::parse_str(&workspace_id).map_err(|_| Status::invalid_argument("Invalid UUID"))?;

        let response = self
            .connection
            .transaction::<_, Response<DeleteWorkspaceResponse>, Status>(|txn| {
                Box::pin(async move {
                    let now = Utc::now();
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
                    if workspace.owner_user_id != actor_user_id {
                        return Err(Status::permission_denied(
                            "Only workspace owner can delete workspace",
                        ));
                    }

                    let mut active = workspace.into_active_model();
                    active.archived_at = Set(Some(now.into()));
                    active.updated_at = Set(now.into());
                    active.update(txn).await.map_err(|e| {
                        error!(error = %e, "Workspace delete failed");
                        Status::internal("Internal Server Error")
                    })?;

                    outbox_event::Entity::insert(outbox_event::ActiveModel {
                        event_id: Set(Uuid::new_v4()),
                        aggregate_type: Set("workspace".to_string()),
                        aggregate_id: Set(workspace_id),
                        event_type: Set("WorkspaceDeleted".to_string()),
                        payload: Set(payload_value(WorkspaceDeletedPayload {
                            workspace_id: workspace_id.to_string(),
                            deleted_by_user_id: actor_user_id.to_string(),
                            deleted_at: now.to_rfc3339(),
                        })?),
                        status: Set("pending".to_string()),
                        publish_attempts: Set(0),
                        occurred_at: Set(now.into()),
                        available_at: Set(now.into()),
                        claimed_by: Set(None),
                        claimed_at: Set(None),
                        published_at: Set(None),
                        last_error: Set(None),
                        created_at: Set(now.into()),
                    })
                    .exec(txn)
                    .await
                    .map_err(|e| {
                        error!(error = %e, "Workspace deleted event insert failed");
                        Status::internal("Internal Server Error")
                    })?;

                    Ok(Response::new(DeleteWorkspaceResponse {
                        workspace_id: workspace_id.to_string(),
                        deleted: true,
                        deleted_at: Some(to_timestamp(now)),
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "Delete workspace transaction connection failure");
                    Status::internal("Internal Server Error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}
