use chrono::Utc;
use relay_proto::workspace::{CreateInviteLinkRequest, CreateInviteLinkResponse};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set, TransactionError, TransactionTrait};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::{
    entity::{
        outbox_event, workspace, workspace_invite_link, workspace_member, workspace_member_role,
        workspace_role,
    },
    events::WorkspaceInviteLinkCreatedPayload,
};

use super::handler::Handler;
use super::lib::permission;
use relay_types::{actor_user_id, payload_value, to_timestamp};

impl Handler {
    pub(super) async fn create_invite_link(
        &self,
        request: Request<CreateInviteLinkRequest>,
    ) -> Result<Response<CreateInviteLinkResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let CreateInviteLinkRequest {
            workspace_id,
            max_uses,
            expires_at,
        } = request.into_inner();

        let max_uses = max_uses
            .map(|n| {
                if n < 0 {
                    Err(Status::invalid_argument("max_uses cannot be negative"))
                } else {
                    Ok(n)
                }
            })
            .transpose()?;

        let response = self
            .connection
            .transaction::<_, Response<CreateInviteLinkResponse>, Status>(|txn| {
                Box::pin(async move {
                    let now = Utc::now();

                    let workspace_id = Uuid::parse_str(&workspace_id)
                        .map_err(|_| Status::invalid_argument("Invalid workspace id"))?;

                    // Get the workspace
                    let workspace = workspace::Entity::find_by_id(workspace_id)
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
                        if permission::has(perms, permission::INVITE_LINK_CREATE) {
                            allowed = true;
                            break;
                        }
                    }

                    if !allowed {
                        return Err(Status::permission_denied("Insufficient permissions"));
                    };

                    // Generate code
                    let code = Uuid::new_v4().to_string();

                    // Create invite link
                    let expires_at = match expires_at {
                        Some(ts) => {
                            let expires_at = chrono::DateTime::<Utc>::from_timestamp(
                                ts.seconds,
                                ts.nanos as u32,
                            )
                            .ok_or_else(|| Status::invalid_argument("Invalid expires at"))?;

                            if expires_at <= now {
                                return Err(Status::invalid_argument(
                                    "expires_at must be in the future",
                                ));
                            }

                            Some(expires_at)
                        }
                        None => None,
                    };
                    let invite_link_id = Uuid::new_v4();
                    let invite_link = workspace_invite_link::ActiveModel {
                        id: Set(invite_link_id),
                        workspace_id: Set(workspace_id),
                        code: Set(code.clone()),
                        created_by_user_id: Set(actor_user_id),
                        max_uses: Set(max_uses),
                        status: Set("active".to_string()),
                        expires_at: Set(expires_at.map(Into::into)),
                        use_count: Set(0),
                        created_at: Set(now.into()),
                        revoked_at: Set(None),
                    };

                    workspace_invite_link::Entity::insert(invite_link)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace invite link insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    // Insert event
                    let event = outbox_event::ActiveModel {
                        event_id: Set(Uuid::new_v4()),
                        aggregate_type: Set("workspace_invite_link".to_string()),
                        aggregate_id: Set(invite_link_id),
                        event_type: Set("WorkspaceInviteLinkCreated".to_string()),
                        created_at: Set(now.into()),
                        available_at: Set(now.into()),
                        occurred_at: Set(now.into()),
                        claimed_at: Set(None),
                        claimed_by: Set(None),
                        published_at: Set(None),
                        last_error: Set(None),
                        publish_attempts: Set(0),
                        status: Set("pending".to_string()),
                        payload: Set(payload_value(WorkspaceInviteLinkCreatedPayload {
                            workspace_invite_link_id: invite_link_id.to_string(),
                            workspace_id: workspace_id.to_string(),
                            code: code.clone(),
                            created_by_user_id: actor_user_id.to_string(),
                            status: "active".to_string(),
                            expires_at: expires_at.map(|dt| dt.to_rfc3339()),
                            max_uses,
                            use_count: 0,
                            created_at: now.to_rfc3339(),
                        })?),
                    };
                    outbox_event::Entity::insert(event)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace invite link event insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    Ok(Response::new(CreateInviteLinkResponse {
                        workspace_invite_link_id: invite_link_id.to_string(),
                        workspace_id: workspace_id.to_string(),
                        code,
                        status: "active".to_string(),
                        expires_at: expires_at.map(to_timestamp),
                        max_uses,
                        use_count: 0,
                        created_at: Some(to_timestamp(now)),
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "Workspace invite link transaction connection failure");
                    Status::internal("Internal Server Error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}
