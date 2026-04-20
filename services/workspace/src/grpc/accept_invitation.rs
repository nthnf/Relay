use chrono::Utc;
use relay_proto::workspace::{AcceptInvitationRequest, AcceptInvitationResponse};
use sea_orm::{
    ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set, TransactionError, TransactionTrait,
};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::{
    entity::{
        outbox_event, workspace, workspace_invitation, workspace_member, workspace_member_role,
        workspace_role,
    },
    events::{WorkspaceInvitationAcceptedPayload, WorkspaceMemberAddedPayload},
};

use super::handler::Handler;
use relay_types::{actor_user_id, payload_value, to_timestamp};

impl Handler {
    pub(super) async fn accept_invitation(
        &self,
        request: Request<AcceptInvitationRequest>,
    ) -> Result<Response<AcceptInvitationResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let AcceptInvitationRequest {
            workspace_invitation_id,
        } = request.into_inner();

        let workspace_invitation_id = Uuid::parse_str(&workspace_invitation_id)
            .map_err(|_| Status::invalid_argument("Invalid workspace invitation id"))?;

        let response = self
            .connection
            .transaction::<_, Response<AcceptInvitationResponse>, Status>(|txn| {
                Box::pin(async move {
                    let now = Utc::now();

                    // Get the workspace invitation
                    let invitation = workspace_invitation::Entity::find_by_id(workspace_invitation_id)
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace invitation lookup failed");
                            Status::internal("Internal Server Error")
                        })?
                        .ok_or_else(|| Status::not_found("Workspace invitation not found"))?;

                    if invitation.issued_to_user_id != actor_user_id {
                        return Err(Status::permission_denied(
                            "Only the invited user may accept the invitation",
                        ));
                    }

                    if invitation.status != "pending" {
                        return Err(Status::failed_precondition("Invitation not pending"));
                    }

                    let invitation_expires_at: chrono::DateTime<Utc> = invitation.expires_at.into();
                    if invitation_expires_at <= now {
                        return Err(Status::failed_precondition("Invitation expired"));
                    }

                    // Get the workspace
                    let workspace = workspace::Entity::find_by_id(invitation.workspace_id)
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

                    // Get the target member
                    let target_member = workspace_member::Entity::find()
                        .filter(workspace_member::Column::WorkspaceId.eq(invitation.workspace_id))
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

                    let issued_by_user_id = invitation.issued_by_user_id;
                    let workspace_id = invitation.workspace_id;

                    match target_member {
                        Some(target_member) => {
                            let mut active = target_member.into_active_model();
                            active.membership_status = Set("active".to_string());
                            active.joined_at = Set(now.into());
                            active.removed_at = Set(None);
                            active.added_by_user_id = Set(Some(issued_by_user_id));

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
                                added_by_user_id: Set(Some(issued_by_user_id)),
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
                        })?;
                    let Some(member_role) = member_role else {
                        return Err(Status::internal("Internal Server Error"));
                    };

                    let member_role = workspace_member_role::ActiveModel {
                        user_id: Set(actor_user_id),
                        workspace_id: Set(workspace_id),
                        role_id: Set(member_role.id),
                        assigned_at: Set(now.into()),
                        assigned_by_user_id: Set(Some(issued_by_user_id)),
                    };
                    workspace_member_role::Entity::insert(member_role)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace member role insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    let mut invitation_active = invitation.into_active_model();
                    invitation_active.status = Set("accepted".to_string());
                    invitation_active.accepted_at = Set(Some(now.into()));
                    workspace_invitation::Entity::update(invitation_active)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace invitation update failed");
                            Status::internal("Internal Server Error")
                        })?;

                    // Insert workspace invitation accepted event
                    let accepted_event = outbox_event::ActiveModel {
                        event_id: Set(Uuid::new_v4()),
                        aggregate_type: Set("workspace_invitation".to_string()),
                        aggregate_id: Set(workspace_invitation_id),
                        event_type: Set("WorkspaceInvitationAccepted".to_string()),
                        created_at: Set(now.into()),
                        available_at: Set(now.into()),
                        occurred_at: Set(now.into()),
                        claimed_at: Set(None),
                        claimed_by: Set(None),
                        published_at: Set(None),
                        last_error: Set(None),
                        publish_attempts: Set(0),
                        status: Set("pending".to_string()),
                        payload: Set(payload_value(WorkspaceInvitationAcceptedPayload {
                            workspace_invitation_id: workspace_invitation_id.to_string(),
                            workspace_id: workspace_id.to_string(),
                            user_id: actor_user_id.to_string(),
                            accepted_at: now.to_rfc3339(),
                            joined_at: now.to_rfc3339(),
                        })?),
                    };
                    outbox_event::Entity::insert(accepted_event)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace invitation accepted event insert failed");
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
                            added_by_user_id: issued_by_user_id.to_string(),
                            source: "invitation_accept".to_string(),
                        })?),
                    };
                    outbox_event::Entity::insert(member_added_event)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace member added event insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    Ok(Response::new(AcceptInvitationResponse {
                        workspace_id: workspace_id.to_string(),
                        workspace_invitation_id: workspace_invitation_id.to_string(),
                        user_id: actor_user_id.to_string(),
                        joined_at: Some(to_timestamp(now)),
                        accepted_at: Some(to_timestamp(now)),
                        added_by_user_id: issued_by_user_id.to_string(),
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "Workspace invitation acceptance transaction connection failure");
                    Status::internal("Internal Server Error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}
