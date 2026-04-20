use chrono::Utc;
use relay_proto::workspace::{RevokeInviteLinkRequest, RevokeInviteLinkResponse};
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
    events::WorkspaceInviteLinkRevokedPayload,
};

use super::handler::Handler;
use super::lib::permission;
use relay_types::{actor_user_id, payload_value, to_timestamp};

impl Handler {
    pub(super) async fn revoke_invite_link(
        &self,
        request: Request<RevokeInviteLinkRequest>,
    ) -> Result<Response<RevokeInviteLinkResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let RevokeInviteLinkRequest {
            workspace_invite_link_id,
        } = request.into_inner();

        let workspace_invite_link_id = Uuid::parse_str(&workspace_invite_link_id)
            .map_err(|_| Status::invalid_argument("Invalid workspace invite link id"))?;

        let response = self
            .connection
            .transaction::<_, Response<RevokeInviteLinkResponse>, Status>(|txn| {
                Box::pin(async move {
                    let now = Utc::now();

                    // Get invite link
                    let invite_link = workspace_invite_link::Entity::find_by_id(workspace_invite_link_id)
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace invite link lookup failed");
                            Status::internal("Internal Server Error")
                        })?
                        .ok_or_else(|| Status::not_found("Invite link not found"))?;

                    let workspace_id = invite_link.workspace_id;

                    // Get workspace
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

                    // Check actor is active workspace member
                    let actor_member = workspace_member::Entity::find()
                        .filter(workspace_member::Column::WorkspaceId.eq(workspace_id))
                        .filter(workspace_member::Column::UserId.eq(actor_user_id))
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

                    // Get actor workspace roles
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
                        if permission::has(perms, permission::INVITE_LINK_REVOKE) {
                            allowed = true;
                            break;
                        }
                    }

                    if !allowed {
                        return Err(Status::permission_denied("Insufficient permissions"));
                    }

                    // Check invite link is not revoked
                    if invite_link.status == "revoked" {
                        return Ok(Response::new(RevokeInviteLinkResponse {
                            workspace_invite_link_id: workspace_invite_link_id.to_string(),
                            workspace_id: workspace_id.to_string(),
                            status: invite_link.status,
                            revoked_at: invite_link.revoked_at.map(|dt| to_timestamp(dt.into())),
                        }));
                    }

                    // Check invite link is not active
                    if invite_link.status != "active" {
                        return Err(Status::failed_precondition("Invite link not active"));
                    }

                    // Revoke invite link
                    let revoked_at = now;
                    let mut active = invite_link.into_active_model();
                    active.status = Set("revoked".to_string());
                    active.revoked_at = Set(Some(revoked_at.into()));
                    workspace_invite_link::Entity::update(active)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace invite link update failed");
                            Status::internal("Internal Server Error")
                        })?;

                    // Insert event
                    let event = outbox_event::ActiveModel {
                        event_id: Set(Uuid::new_v4()),
                        aggregate_type: Set("workspace_invite_link".to_string()),
                        aggregate_id: Set(workspace_invite_link_id),
                        event_type: Set("WorkspaceInviteLinkRevoked".to_string()),
                        created_at: Set(now.into()),
                        available_at: Set(now.into()),
                        occurred_at: Set(now.into()),
                        claimed_at: Set(None),
                        claimed_by: Set(None),
                        published_at: Set(None),
                        last_error: Set(None),
                        publish_attempts: Set(0),
                        status: Set("pending".to_string()),
                        payload: Set(payload_value(WorkspaceInviteLinkRevokedPayload {
                            workspace_invite_link_id: workspace_invite_link_id.to_string(),
                            workspace_id: workspace_id.to_string(),
                            status: "revoked".to_string(),
                            revoked_at: revoked_at.to_rfc3339(),
                        })?),
                    };
                    outbox_event::Entity::insert(event)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace invite link revoked event insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    Ok(Response::new(RevokeInviteLinkResponse {
                        workspace_invite_link_id: workspace_invite_link_id.to_string(),
                        workspace_id: workspace_id.to_string(),
                        status: "revoked".to_string(),
                        revoked_at: Some(to_timestamp(revoked_at)),
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "Workspace invite link revoke transaction connection failure");
                    Status::internal("Internal Server Error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}
