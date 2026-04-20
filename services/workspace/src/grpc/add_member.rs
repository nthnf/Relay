use chrono::Utc;
use relay_proto::workspace::{AddMemberRequest, AddMemberResponse};
use sea_orm::{
    ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set, TransactionError, TransactionTrait,
};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::{
    entity::{outbox_event, workspace, workspace_member, workspace_member_role, workspace_role},
    events::WorkspaceMemberAddedPayload,
    grpc::lib::user_account_exists,
};

use super::handler::Handler;
use super::lib::permission;
use relay_types::{actor_user_id, payload_value, to_timestamp};

impl Handler {
    pub(super) async fn add_member(
        &self,
        request: Request<AddMemberRequest>,
    ) -> Result<Response<AddMemberResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let AddMemberRequest {
            workspace_id,
            target_user_id,
        } = request.into_inner();

        let target_user_id = Uuid::parse_str(&target_user_id)
            .map_err(|_| Status::invalid_argument("Invalid target user id"))?;
        let workspace_id = Uuid::parse_str(&workspace_id)
            .map_err(|_| Status::invalid_argument("Invalid workspace id"))?;

        if !user_account_exists(&self.connection, target_user_id).await? {
            return Err(Status::not_found("User not found"));
        }

        let response = self
            .connection
            .transaction::<_, Response<AddMemberResponse>, Status>(|txn| {
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

                    let mut allowed = false;
                    for (_, role) in &member_role_joins {
                        let Some(role) = role.as_ref() else {
                            return Err(Status::internal("Internal Server Error"));
                        };
                        let perms = permission::from_db(role.permissions)?;
                        if permission::has(perms, permission::MEMBER_ADD) {
                            allowed = true;
                            break;
                        }
                    }

                    if !allowed {
                        return Err(Status::permission_denied("Insufficient permissions"));
                    };

                    let member_role = workspace_role::Entity::find()
                        .filter(workspace_role::Column::WorkspaceId.eq(workspace_id))
                        .filter(workspace_role::Column::Name.eq("Member".to_string()))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace role lookup failed");
                            Status::internal("Internal Server Error")
                        })?;
                    let Some(member_role) = member_role else {
                        return Err(Status::internal("Internal Server Error"));
                    };

                    // Check target if already a member
                    let target_member = workspace_member::Entity::find()
                        .filter(workspace_member::Column::UserId.eq(target_user_id))
                        .filter(workspace_member::Column::WorkspaceId.eq(workspace_id))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace member lookup failed");
                            Status::internal("Internal Server Error")
                        })?;

                    match target_member {
                        Some(target_member) => {
                            if target_member.membership_status == "active" {
                                return Err(Status::already_exists("Already a member"));
                            }
                            let mut target_member = target_member.into_active_model();
                            target_member.membership_status = Set("active".to_string());
                            target_member.joined_at = Set(now.into());
                            target_member.removed_at = Set(None);
                            target_member.added_by_user_id = Set(Some(actor_user_id));

                            workspace_member::Entity::update(target_member)
                                .exec(txn)
                                .await
                                .map_err(|e| {
                                    error!(error = %e, "Workspace member insert failed");
                                    Status::internal("Internal Server Error")
                                })?;
                        }
                        None => {
                            let member = workspace_member::ActiveModel {
                                user_id: Set(target_user_id),
                                workspace_id: Set(workspace_id),
                                added_by_user_id: Set(Some(actor_user_id)),
                                joined_at: Set(now.into()),
                                removed_at: Set(None),
                                membership_status: Set("active".to_string()),
                            };
                            workspace_member::Entity::insert(member)
                                .exec(txn)
                                .await
                                .map_err(|e| {
                                    error!(error = %e, "Workspace member insert failed");
                                    Status::internal("Internal Server Error")
                                })?;
                        }
                    };

                    // Create workspace member role
                    let existing_member_role = workspace_member_role::Entity::find()
                        .filter(workspace_member_role::Column::WorkspaceId.eq(workspace_id))
                        .filter(workspace_member_role::Column::UserId.eq(target_user_id))
                        .filter(workspace_member_role::Column::RoleId.eq(member_role.id))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace member role lookup failed");
                            Status::internal("Internal Server Error")
                        })?;

                    match existing_member_role {
                        Some(existing_member_role) => {
                            let mut active = existing_member_role.into_active_model();
                            active.assigned_at = Set(now.into());
                            active.assigned_by_user_id = Set(Some(actor_user_id));
                            workspace_member_role::Entity::update(active)
                                .exec(txn)
                                .await
                                .map_err(|e| {
                                    error!(error = %e, "Workspace member role update failed");
                                    Status::internal("Internal Server Error")
                                })?;
                        }
                        None => {
                            let member_role = workspace_member_role::ActiveModel {
                                user_id: Set(target_user_id),
                                workspace_id: Set(workspace_id),
                                role_id: Set(member_role.id),
                                assigned_at: Set(now.into()),
                                assigned_by_user_id: Set(Some(actor_user_id)),
                            };
                            workspace_member_role::Entity::insert(member_role)
                                .exec(txn)
                                .await
                                .map_err(|e| {
                                    error!(error = %e, "Workspace member role insert failed");
                                    Status::internal("Internal Server Error")
                                })?;
                        }
                    }

                    // Insert workspace member added event
                    let event = outbox_event::ActiveModel {
                        event_id: Set(Uuid::new_v4()),
                        aggregate_type: Set("workspace_member".to_string()),
                        aggregate_id: Set(workspace_id),
                        event_type: Set("WorkspaceMemberAdded".to_string()),
                        created_at: Set(now.into()),
                        available_at: Set(now.into()),
                        occurred_at: Set(now.into()),
                        claimed_at: Set(None),
                        claimed_by: Set(None),
                        published_at: Set(None),
                        last_error: Set(None),
                        publish_attempts: Set(0),
                        status: Set("pending".to_string()),
                        payload: Set(payload_value(WorkspaceMemberAddedPayload {
                            workspace_id: workspace_id.to_string(),
                            user_id: target_user_id.to_string(),
                            joined_at: now.to_rfc3339(),
                            added_by_user_id: actor_user_id.to_string(),
                            source: "direct_add".to_string(),
                        })?),
                    };
                    outbox_event::Entity::insert(event)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace member added event insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    Ok(Response::new(AddMemberResponse {
                        workspace_id: workspace_id.to_string(),
                        user_id: target_user_id.to_string(),
                        joined_at: Some(to_timestamp(now)),
                        added_by_user_id: actor_user_id.to_string(),
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "Workspace add member transaction connection failure");
                    Status::internal("Internal Server Error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}
