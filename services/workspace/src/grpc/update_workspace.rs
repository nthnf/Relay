use chrono::Utc;
use relay_proto::workspace::{UpdateWorkspaceRequest, UpdateWorkspaceResponse};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set,
    TransactionError, TransactionTrait,
};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::{
    entity::{outbox_event, workspace, workspace_member, workspace_member_role, workspace_role},
    events::WorkspaceUpdatedPayload,
};

use super::{handler::Handler, lib::permission};
use relay_types::{actor_user_id, payload_value, to_timestamp};

impl Handler {
    pub(super) async fn update_workspace(
        &self,
        request: Request<UpdateWorkspaceRequest>,
    ) -> Result<Response<UpdateWorkspaceResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let UpdateWorkspaceRequest {
            workspace_id,
            name,
            icon_url,
        } = request.into_inner();
        let workspace_id =
            Uuid::parse_str(&workspace_id).map_err(|_| Status::invalid_argument("Invalid UUID"))?;

        let name = name.map(|value| value.trim().to_string());
        if matches!(name.as_deref(), Some("")) {
            return Err(Status::invalid_argument("Workspace name is required"));
        }

        let response = self
            .connection
            .transaction::<_, Response<UpdateWorkspaceResponse>, Status>(|txn| {
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
                        workspace_member::Entity::find()
                            .filter(workspace_member::Column::UserId.eq(actor_user_id))
                            .filter(workspace_member::Column::WorkspaceId.eq(workspace_id))
                            .filter(workspace_member::Column::MembershipStatus.eq("active"))
                            .one(txn)
                            .await
                            .map_err(|e| {
                                error!(error = %e, "Workspace member lookup failed");
                                Status::internal("Internal Server Error")
                            })?
                            .ok_or_else(|| Status::not_found("Workspace not found"))?;

                        let role_joins = workspace_member_role::Entity::find()
                            .find_also_related(workspace_role::Entity)
                            .filter(workspace_member_role::Column::UserId.eq(actor_user_id))
                            .filter(workspace_member_role::Column::WorkspaceId.eq(workspace_id))
                            .all(txn)
                            .await
                            .map_err(|e| {
                                error!(error = %e, "Workspace member role lookup failed");
                                Status::internal("Internal Server Error")
                            })?;

                        let mut allowed = false;
                        for (_, role) in role_joins {
                            let Some(role) = role else {
                                continue;
                            };
                            let perms = permission::from_db(role.permissions)?;
                            if permission::has(perms, permission::WORKSPACE_EDIT) {
                                allowed = true;
                                break;
                            }
                        }

                        if !allowed {
                            return Err(Status::permission_denied("Insufficient permissions"));
                        }
                    }

                    let next_name = name.unwrap_or_else(|| workspace.name.clone());
                    let next_icon_url = icon_url.or(workspace.icon_url.clone());

                    let mut active = workspace.clone().into_active_model();
                    active.name = Set(next_name.clone());
                    active.icon_url = Set(next_icon_url.clone());
                    active.updated_at = Set(now.into());
                    active.update(txn).await.map_err(|e| {
                        error!(error = %e, "Workspace update failed");
                        Status::internal("Internal Server Error")
                    })?;

                    outbox_event::Entity::insert(outbox_event::ActiveModel {
                        event_id: Set(Uuid::new_v4()),
                        aggregate_type: Set("workspace".to_string()),
                        aggregate_id: Set(workspace_id),
                        event_type: Set("WorkspaceUpdated".to_string()),
                        payload: Set(payload_value(WorkspaceUpdatedPayload {
                            workspace_id: workspace_id.to_string(),
                            name: next_name.clone(),
                            icon_url: next_icon_url.clone(),
                            updated_by_user_id: actor_user_id.to_string(),
                            updated_at: now.to_rfc3339(),
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
                        error!(error = %e, "Workspace updated event insert failed");
                        Status::internal("Internal Server Error")
                    })?;

                    Ok(Response::new(UpdateWorkspaceResponse {
                        workspace_id: workspace_id.to_string(),
                        name: next_name,
                        owner_user_id: workspace.owner_user_id.to_string(),
                        icon_url: next_icon_url,
                        updated_at: Some(to_timestamp(now)),
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "Update workspace transaction connection failure");
                    Status::internal("Internal Server Error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}
