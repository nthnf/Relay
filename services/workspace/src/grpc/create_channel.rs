use chrono::Utc;
use relay_proto::workspace::{CreateChannelRequest, CreateChannelResponse};
use sea_orm::{
    ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect, Set, TransactionError,
    TransactionTrait,
};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::{
    entity::{
        outbox_event, workspace, workspace_channel, workspace_member, workspace_member_role,
        workspace_role,
    },
    events::WorkspaceChannelCreatedPayload,
};

use super::handler::Handler;
use super::lib::permission;
use relay_types::{actor_user_id, payload_value, to_timestamp};

impl Handler {
    pub(super) async fn create_channel(
        &self,
        request: Request<CreateChannelRequest>,
    ) -> Result<Response<CreateChannelResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let CreateChannelRequest {
            workspace_id,
            name,
            channel_kind,
            position,
        } = request.into_inner();

        if name.trim() == "" {
            return Err(Status::invalid_argument("Invalid channel name"));
        }

        match channel_kind.as_str() {
            "text" => {}
            _ => {
                return Err(Status::invalid_argument("Invalid channel kind"));
            }
        };

        let workspace_id =
            Uuid::parse_str(&workspace_id).map_err(|_| Status::invalid_argument("Invalid UUID"))?;

        let response = self
            .connection
            .transaction::<_, Response<CreateChannelResponse>, Status>(|txn| {
                Box::pin(async move {
                    // Check if its member
                    let member = workspace_member::Entity::find()
                        .filter(workspace_member::Column::UserId.eq(actor_user_id))
                        .filter(workspace_member::Column::WorkspaceId.eq(workspace_id))
                        .filter(workspace_member::Column::MembershipStatus.eq("active"))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace member lookup failed");
                            Status::internal("Internal Server Error")
                        })?;
                    if member.is_none() {
                        return Err(Status::not_found("Workspace not found"));
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

                    // Get workspace member role
                    let member_role_join = workspace_member_role::Entity::find()
                        .find_also_related(workspace_role::Entity)
                        .filter(workspace_member_role::Column::UserId.eq(actor_user_id))
                        .filter(workspace_member_role::Column::WorkspaceId.eq(workspace_id))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace member role lookup failed");
                            Status::internal("Internal Server Error")
                        })?;
                    let Some(member_role_join) = member_role_join else {
                        return Err(Status::not_found("Member role not found"));
                    };

                    // Check member role permissions
                    let (_, role) = member_role_join;
                    let Some(role) = role else {
                        return Err(Status::internal("Internal Server Error"));
                    };

                    if !permission::has(role.permissions, permission::CHANNEL_CREATE) {
                        return Err(Status::permission_denied("Insufficient permissions"));
                    };

                    // Determine next position
                    let position = if let Some(position) = position {
                        if position < 1 {
                            return Err(Status::invalid_argument("Invalid position"));
                        }

                        let occupied = workspace_channel::Entity::find()
                            .filter(workspace_channel::Column::WorkspaceId.eq(workspace_id))
                            .filter(workspace_channel::Column::ArchivedAt.is_null())
                            .filter(workspace_channel::Column::Position.eq(position))
                            .count(txn)
                            .await
                            .map_err(|e| {
                                error!(error = %e, "Workspace channel position lookup failed");
                                Status::internal("Internal Server Error")
                            })?;

                        if occupied > 0 {
                            return Err(Status::invalid_argument("Invalid position"));
                        }

                        position
                    } else {
                        let max_position = workspace_channel::Entity::find()
                            .filter(workspace_channel::Column::WorkspaceId.eq(workspace_id))
                            .filter(workspace_channel::Column::ArchivedAt.is_null())
                            .select_only()
                            .column_as(workspace_channel::Column::Position.max(), "max_position")
                            .into_tuple::<Option<i32>>()
                            .one(txn)
                            .await
                            .map_err(|e| {
                                error!(error = %e, "Workspace channel max position lookup failed");
                                Status::internal("Internal Server Error")
                            })?
                            .flatten()
                            .unwrap_or(0);

                        max_position + 1
                    };

                    // Create channel
                    let channel_id = Uuid::new_v4();
                    let now = Utc::now();
                    let channel = workspace_channel::ActiveModel {
                        workspace_id: Set(workspace_id),
                        channel_id: Set(channel_id),
                        name: Set(name.to_string()),
                        channel_kind: Set(channel_kind.to_string()),
                        position: Set(position),
                        created_by_user_id: Set(actor_user_id),
                        created_at: Set(now.into()),
                        archived_at: Set(None),
                    };
                    workspace_channel::Entity::insert(channel)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace channel insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    // Insert channel created event
                    let event = outbox_event::ActiveModel {
                        event_id: Set(Uuid::new_v4()),
                        aggregate_type: Set("workspace_channel".to_string()),
                        aggregate_id: Set(channel_id),
                        event_type: Set("WorkspaceChannelCreated".to_string()),
                        created_at: Set(now.into()),
                        available_at: Set(now.into()),
                        occurred_at: Set(now.into()),
                        claimed_at: Set(None),
                        claimed_by: Set(None),
                        published_at: Set(None),
                        last_error: Set(None),
                        publish_attempts: Set(0),
                        status: Set("pending".to_string()),
                        payload: Set(payload_value(WorkspaceChannelCreatedPayload {
                            channel_id: channel_id.to_string(),
                            workspace_id: workspace_id.to_string(),
                            name: name.to_string(),
                            channel_kind: "text".to_string(),
                            position,
                            created_by_user_id: actor_user_id.to_string(),
                            created_at: now.to_rfc3339(),
                        })?),
                    };
                    outbox_event::Entity::insert(event)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace channel created event insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    Ok(Response::new(CreateChannelResponse {
                        channel_id: channel_id.to_string(),
                        workspace_id: workspace_id.to_string(),
                        name,
                        channel_kind,
                        position,
                        created_at: Some(to_timestamp(now)),
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "Create workspace transaction connection failure");
                    Status::internal("Internal Server Error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}
