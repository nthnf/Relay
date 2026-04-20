use chrono::Utc;
use relay_proto::workspace::{JoinWorkspaceByInviteLinkRequest, JoinWorkspaceByInviteLinkResponse};
use sea_orm::{
    ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set, TransactionError, TransactionTrait,
};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::{
    entity::{
        outbox_event, workspace_invite_link, workspace_member, workspace_member_role,
        workspace_role,
    },
    events::WorkspaceMemberAddedPayload,
};

use super::handler::Handler;
use relay_types::{actor_user_id, payload_value, to_timestamp};

impl Handler {
    pub(super) async fn join_workspace_by_invite_link(
        &self,
        request: Request<JoinWorkspaceByInviteLinkRequest>,
    ) -> Result<Response<JoinWorkspaceByInviteLinkResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let JoinWorkspaceByInviteLinkRequest { code } = request.into_inner();

        let response = self
            .connection
            .transaction::<_, Response<JoinWorkspaceByInviteLinkResponse>, Status>(|txn| {
                Box::pin(async move {
                    let now = Utc::now();

                    // Get invite link
                    let invite_link = workspace_invite_link::Entity::find()
                        .filter(workspace_invite_link::Column::Code.eq(code))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace invite link lookup failed");
                            Status::internal("Internal Server Error")
                        })?
                        .ok_or_else(|| Status::not_found("Invite link not found"))?;

                    if invite_link.status != "active" {
                        return Err(Status::failed_precondition("Invite link not active"));
                    }

                    if let Some(expires_at) = invite_link.expires_at {
                        let expires_at: chrono::DateTime<Utc> = expires_at.into();
                        if expires_at < now {
                            return Err(Status::failed_precondition("Invite link expired"));
                        }
                    }

                    if let Some(max_uses) = invite_link.max_uses && invite_link.use_count >= max_uses {
                            return Err(Status::failed_precondition("Invite link exhausted"));
                    }

                    // Get workspace
                    let workspace_id = invite_link.workspace_id;
                    let workspace_invite_link_id = invite_link.id;

                    let workspace = crate::entity::workspace::Entity::find_by_id(workspace_id)
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace lookup failed");
                            Status::internal("Internal Server Error")
                        })?
                        .ok_or_else(|| Status::not_found("Workspace not found"))?;

                    if workspace.archived_at.is_some() {
                        return Err(Status::failed_precondition("Invalid workspace"));
                    }

                    // Get target member
                    let target_member = workspace_member::Entity::find()
                        .filter(workspace_member::Column::WorkspaceId.eq(workspace_id))
                        .filter(workspace_member::Column::UserId.eq(actor_user_id))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace member lookup failed");
                            Status::internal("Internal Server Error")
                        })?;

                    if matches!(
                        target_member.as_ref().map(|member| member.membership_status.as_str()),
                        Some("active")
                    ) {
                        return Err(Status::already_exists("Already a member"));
                    }

                    // Set member
                    match target_member {
                        Some(target_member) => {
                            let mut active = target_member.into_active_model();
                            active.membership_status = Set("active".to_string());
                            active.joined_at = Set(now.into());
                            active.removed_at = Set(None);
                            active.added_by_user_id = Set(Some(actor_user_id));

                            workspace_member::Entity::update(active)
                                .exec(txn)
                                .await
                                .map_err(|e| {
                                    error!(error = %e, "Workspace member update failed");
                                    Status::internal("Internal Server Error")
                                })?;
                        }
                        None => {
                            let member = workspace_member::ActiveModel {
                                workspace_id: Set(workspace_id),
                                user_id: Set(actor_user_id),
                                membership_status: Set("active".to_string()),
                                joined_at: Set(now.into()),
                                removed_at: Set(None),
                                added_by_user_id: Set(Some(actor_user_id)),
                            };

                            workspace_member::Entity::insert(member)
                                .exec(txn)
                                .await
                                .map_err(|e| {
                                    error!(error = %e, "Workspace member insert failed");
                                    Status::internal("Internal Server Error")
                                })?;
                        }
                    }

                    // Delete stale member roles
                    workspace_member_role::Entity::delete_many()
                        .filter(workspace_member_role::Column::WorkspaceId.eq(workspace_id))
                        .filter(workspace_member_role::Column::UserId.eq(actor_user_id))
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace member role cleanup failed");
                            Status::internal("Internal Server Error")
                        })?;

                    // Get "Member" role
                    let member_role = workspace_role::Entity::find()
                        .filter(workspace_role::Column::WorkspaceId.eq(workspace_id))
                        .filter(workspace_role::Column::Name.eq("Member".to_string()))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace role lookup failed");
                            Status::internal("Internal Server Error")
                        })?
                        .ok_or_else(|| Status::internal("Internal Server Error"))?;

                    // Insert member role
                    workspace_member_role::Entity::insert(workspace_member_role::ActiveModel {
                        workspace_id: Set(workspace_id),
                        user_id: Set(actor_user_id),
                        role_id: Set(member_role.id),
                        assigned_at: Set(now.into()),
                        assigned_by_user_id: Set(Some(actor_user_id)),
                    })
                    .exec(txn)
                    .await
                    .map_err(|e| {
                        error!(error = %e, "Workspace member role insert failed");
                        Status::internal("Internal Server Error")
                    })?;

                    // Increment use count
                    let use_count = invite_link.use_count + 1;
                    let status = if let Some(max_uses) = invite_link.max_uses {
                        if use_count >= max_uses {
                            "expired"
                        } else {
                            "active"
                        }
                    } else {
                        "active"
                    };

                    let mut invite_link_active = invite_link.into_active_model();
                    invite_link_active.use_count = Set(use_count);
                    invite_link_active.status = Set(status.to_string());

                    workspace_invite_link::Entity::update(invite_link_active)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace invite link update failed");
                            Status::internal("Internal Server Error")
                        })?;

                    // Insert member added event
                    let member_added_event = outbox_event::ActiveModel {
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
                            user_id: actor_user_id.to_string(),
                            joined_at: now.to_rfc3339(),
                            added_by_user_id: actor_user_id.to_string(),
                            source: "invite_link".to_string(),
                        })?),
                    };
                    outbox_event::Entity::insert(member_added_event)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace member added event insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    Ok(Response::new(JoinWorkspaceByInviteLinkResponse {
                        workspace_id: workspace_id.to_string(),
                        workspace_invite_link_id: workspace_invite_link_id.to_string(),
                        user_id: actor_user_id.to_string(),
                        joined_at: Some(to_timestamp(now)),
                        added_by_user_id: actor_user_id.to_string(),
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "Workspace invite link join transaction connection failure");
                    Status::internal("Internal Server Error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}
