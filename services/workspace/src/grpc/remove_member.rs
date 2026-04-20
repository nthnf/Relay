use chrono::Utc;
use relay_proto::workspace::{RemoveMemberRequest, RemoveMemberResponse};
use sea_orm::{
    ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set, TransactionError, TransactionTrait,
};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::{
    entity::{outbox_event, workspace, workspace_member, workspace_member_role, workspace_role},
    events::WorkspaceMemberRemovedPayload,
};

use super::handler::Handler;
use super::lib::permission;
use relay_types::{actor_user_id, payload_value, to_timestamp};

impl Handler {
    pub(super) async fn remove_member(
        &self,
        request: Request<RemoveMemberRequest>,
    ) -> Result<Response<RemoveMemberResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let RemoveMemberRequest {
            workspace_id,
            target_user_id,
        } = request.into_inner();

        let target_user_id = Uuid::parse_str(&target_user_id)
            .map_err(|_| Status::invalid_argument("Invalid target user id"))?;
        let workspace_id = Uuid::parse_str(&workspace_id)
            .map_err(|_| Status::invalid_argument("Invalid workspace id"))?;

        let response = self
            .connection
            .transaction::<_, Response<RemoveMemberResponse>, Status>(|txn| {
                Box::pin(async move {
                    let now = Utc::now();

                    // Get the workspace
                    let workspace = workspace::Entity::find_by_id(workspace_id)
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace lookup failed");
                            Status::internal("Internal Server Error")
                        })?;

                    match workspace {
                        Some(workspace) => {
                            if workspace.archived_at.is_some() {
                                return Err(Status::failed_precondition("Invalid workspace"));
                            }
                            if workspace.owner_user_id == target_user_id {
                                return Err(Status::failed_precondition("Cannot remove owner"));
                            }
                        }
                        None => {
                            return Err(Status::not_found("Workspace not found"));
                        }
                    };

                    // Check actor is active workspace member.
                    let actor_member = workspace_member::Entity::find()
                        .filter(workspace_member::Column::UserId.eq(actor_user_id))
                        .filter(workspace_member::Column::WorkspaceId.eq(workspace_id))
                        .filter(workspace_member::Column::MembershipStatus.eq("active"))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace member lookup failed");
                            Status::internal("Internal Server Error")
                        })?;
                    if actor_member.is_none() {
                        return Err(Status::not_found("Workspace not found"));
                    }

                    // Check actor is not the target user
                    if actor_user_id != target_user_id {
                        // Get actor workspace roles.
                        let member_role_joins = workspace_member_role::Entity::find()
                            .find_also_related(workspace_role::Entity)
                            .filter(workspace_member_role::Column::UserId.eq(actor_user_id))
                            .filter(workspace_member_role::Column::WorkspaceId.eq(workspace_id))
                            .all(txn)
                            .await
                            .map_err(|e| {
                                error!(error = %e, "Workspace member role lookup failed");
                                Status::internal("Internal Server Error")
                            })?;
                        if member_role_joins.is_empty() {
                            return Err(Status::not_found("Member role not found"));
                        }

                        if !member_role_joins.iter().any(|(_, role)| {
                            role.as_ref()
                                .map(|role| {
                                    permission::has(role.permissions, permission::MEMBER_REMOVE)
                                })
                                .unwrap_or(false)
                        }) {
                            return Err(Status::permission_denied("Insufficient permissions"));
                        };
                    }

                    // Get target member
                    let target_member = workspace_member::Entity::find()
                        .filter(workspace_member::Column::UserId.eq(target_user_id))
                        .filter(workspace_member::Column::WorkspaceId.eq(workspace_id))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace member lookup failed");
                            Status::internal("Internal Server Error")
                        })?;

                    let target_member = match target_member {
                        Some(target_member) => {
                            if target_member.membership_status != "active" {
                                return Ok(Response::new(RemoveMemberResponse {
                                    removed: false,
                                    workspace_id: workspace_id.to_string(),
                                    user_id: target_user_id.to_string(),
                                    removed_at: None,
                                }));
                            }
                            target_member
                        }
                        None => {
                            return Ok(Response::new(RemoveMemberResponse {
                                removed: false,
                                workspace_id: workspace_id.to_string(),
                                user_id: target_user_id.to_string(),
                                removed_at: None,
                            }));
                        }
                    };

                    // Soft delete target member
                    let mut target_member = target_member.into_active_model();
                    target_member.removed_at = Set(Some(now.into()));
                    target_member.membership_status = Set("removed".to_string());
                    workspace_member::Entity::update(target_member)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace member insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    // Delete target member role
                    workspace_member_role::Entity::delete_many()
                        .filter(workspace_member_role::Column::UserId.eq(target_user_id))
                        .filter(workspace_member_role::Column::WorkspaceId.eq(workspace_id))
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace member role delete failed");
                            Status::internal("Internal Server Error")
                        })?;

                    // Insert workspace member removed event
                    let event = outbox_event::ActiveModel {
                        event_id: Set(Uuid::new_v4()),
                        aggregate_type: Set("workspace_member".to_string()),
                        aggregate_id: Set(workspace_id),
                        event_type: Set("WorkspaceMemberRemoved".to_string()),
                        created_at: Set(now.into()),
                        available_at: Set(now.into()),
                        occurred_at: Set(now.into()),
                        claimed_at: Set(None),
                        claimed_by: Set(None),
                        published_at: Set(None),
                        last_error: Set(None),
                        publish_attempts: Set(0),
                        status: Set("pending".to_string()),
                        payload: Set(payload_value(WorkspaceMemberRemovedPayload {
                            workspace_id: workspace_id.to_string(),
                            user_id: target_user_id.to_string(),
                            removed_at: now.to_rfc3339(),
                            removed_by_user_id: actor_user_id.to_string(),
                            reason: "removed".to_string(),
                        })?),
                    };
                    outbox_event::Entity::insert(event)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace member removed event insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    Ok(Response::new(RemoveMemberResponse {
                        removed: true,
                        user_id: target_user_id.to_string(),
                        workspace_id: workspace_id.to_string(),
                        removed_at: Some(to_timestamp(now)),
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "Workspace remove member transaction connection failure");
                    Status::internal("Internal Server Error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}
