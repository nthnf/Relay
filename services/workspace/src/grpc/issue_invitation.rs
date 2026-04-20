use chrono::{Duration, Utc};
use relay_proto::workspace::{IssueInvitationRequest, IssueInvitationResponse};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set, TransactionError, TransactionTrait};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::{
    entity::{
        workspace, workspace_invitation, workspace_member, workspace_member_role, workspace_role,
    },
    grpc::lib::user_account_exists,
};

use super::handler::Handler;
use super::lib::permission;
use relay_types::actor_user_id;

impl Handler {
    pub(super) async fn issue_invitation(
        &self,
        request: Request<IssueInvitationRequest>,
    ) -> Result<Response<IssueInvitationResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let IssueInvitationRequest {
            workspace_id,
            target_user_id,
            expires_at,
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
            .transaction::<_, Response<IssueInvitationResponse>, Status>(|txn| {
                Box::pin(async move {
                    let now = Utc::now();
                    let expires_at = match expires_at {
                        Some(ts) => {
                            chrono::DateTime::<Utc>::from_timestamp(ts.seconds, ts.nanos as u32)
                                .ok_or_else(|| Status::invalid_argument("Invalid expires at"))?
                        }
                        None => Utc::now() + Duration::days(30),
                    };

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
                        if permission::has(perms, permission::MEMBER_INVITE) {
                            allowed = true;
                            break;
                        }
                    }

                    if !allowed {
                        return Err(Status::permission_denied("Insufficient permissions"));
                    };

                    // Check target if already a member
                    let target_member = workspace_member::Entity::find()
                        .filter(workspace_member::Column::UserId.eq(target_user_id))
                        .filter(workspace_member::Column::WorkspaceId.eq(workspace_id))
                        .filter(workspace_member::Column::MembershipStatus.eq("active"))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace member lookup failed");
                            Status::internal("Internal Server Error")
                        })?;

                    if target_member.is_some() {
                        return Err(Status::already_exists("Already a member"));
                    };

                    // Check pending invitation
                    let pending_invitation = workspace_invitation::Entity::find()
                        .filter(workspace_invitation::Column::WorkspaceId.eq(workspace_id))
                        .filter(workspace_invitation::Column::IssuedToUserId.eq(target_user_id))
                        .filter(workspace_invitation::Column::Status.eq("pending"))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace invitation lookup failed");
                            Status::internal("Internal Server Error")
                        })?;

                    if pending_invitation.is_some() {
                        return Err(Status::already_exists("Invitation already pending"));
                    };

                    // Create workspace invitation
                    let workspace_invitation_id = Uuid::new_v4();
                    let invitation = workspace_invitation::ActiveModel {
                        id: Set(workspace_invitation_id),
                        workspace_id: Set(workspace_id),
                        issued_to_user_id: Set(target_user_id),
                        issued_by_user_id: Set(actor_user_id),
                        status: Set("pending".to_string()),
                        expires_at: Set(expires_at.into()),
                        accepted_at: Set(None),
                        created_at: Set(now.into()),
                    };
                    workspace_invitation::Entity::insert(invitation)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace invitation insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    // Insert workspace invitation issued event
                    // let event = outbox_event::ActiveModel {
                    //     event_id: Set(Uuid::new_v4()),
                    //     aggregate_type: Set("workspace_member".to_string()),
                    //     aggregate_id: Set(workspace_id),
                    //     event_type: Set("WorkspaceMemberAdded".to_string()),
                    //     created_at: Set(now.into()),
                    //     available_at: Set(now.into()),
                    //     occurred_at: Set(now.into()),
                    //     claimed_at: Set(None),
                    //     claimed_by: Set(None),
                    //     published_at: Set(None),
                    //     last_error: Set(None),
                    //     publish_attempts: Set(0),
                    //     status: Set("pending".to_string()),
                    //     payload: Set(payload_value(WorkspaceInvitationIssuedPayload {
                    //         workspace_invitation_id: workspace_invitation_id.to_string(),
                    //         workspace_id: workspace_id.to_string(),
                    //         issued_to_user_id: target_user_id.to_string(),
                    //         issued_by_user_id: actor_user_id.to_string(),
                    //         expires_at: expires_at.to_rfc3339(),
                    //         created_at: now.to_rfc3339(),
                    //         inviter_display_name_snapshot: inviter_display_name_snapshot
                    //             .to_string(),
                    //         workspace_name_snapshot: workspace_name_snapshot.to_string(),
                    //     })?),
                    // };
                    // outbox_event::Entity::insert(event)
                    //     .exec(txn)
                    //     .await
                    //     .map_err(|e| {
                    //         error!(error = %e, "Workspace member added event insert failed");
                    //         Status::internal("Internal Server Error")
                    //     })?;

                    todo!()
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
