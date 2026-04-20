use chrono::Utc;
use relay_proto::workspace::{CreateWorkspaceRequest, CreateWorkspaceResponse};
use sea_orm::{EntityTrait, Set, TransactionError, TransactionTrait};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::{
    entity::{
        outbox_event, workspace, workspace_channel, workspace_member, workspace_member_role,
        workspace_role,
    },
    events::{
        WorkspaceChannelCreatedPayload, WorkspaceCreatedPayload, WorkspaceMemberAddedPayload,
    },
};

use super::handler::Handler;
use super::lib::permission;
use relay_types::{actor_user_id, payload_value, to_timestamp};

impl Handler {
    pub(super) async fn create_workspace(
        &self,
        request: Request<CreateWorkspaceRequest>,
    ) -> Result<Response<CreateWorkspaceResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;

        let CreateWorkspaceRequest {
            name,
            first_channel_name,
        } = request.into_inner();

        if name.trim() == "" || first_channel_name.trim() == "" {
            return Err(Status::invalid_argument(
                "Invalid workspace name or channel name",
            ));
        }

        let response = self
            .connection
            .transaction::<_, Response<CreateWorkspaceResponse>, Status>(|txn| {
                Box::pin(async move {
                    let now = Utc::now();
                    let workspace_id = Uuid::new_v4();

                    // Create workspace
                    let workspace = workspace::ActiveModel {
                        id: Set(workspace_id),
                        owner_user_id: Set(actor_user_id),
                        name: Set(name.clone()),
                        created_at: Set(now.into()),
                        updated_at: Set(now.into()),
                        archived_at: Set(None),
                        icon_url: Set(None),
                    };
                    workspace::Entity::insert(workspace)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    // Create workspace channel
                    let channel_id = Uuid::new_v4();
                    let channel = workspace_channel::ActiveModel {
                        channel_id: Set(channel_id),
                        workspace_id: Set(workspace_id),
                        name: Set(first_channel_name.to_string()),
                        channel_kind: Set("text".to_string()),
                        position: Set(1),
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

                    // Seed workspace role
                    let owner_role_id = Uuid::new_v4();
                    let owner_role = workspace_role::ActiveModel {
                        id: Set(owner_role_id),
                        is_system_role: Set(true),
                        created_at: Set(now.into()),
                        workspace_id: Set(workspace_id),
                        name: Set("Owner".to_string()),
                        permissions: Set(
                            permission::to_db(permission::owner())
                                .expect("owner permissions fit in i32"),
                        ),
                    };
                    let admin_role = workspace_role::ActiveModel {
                        id: Set(Uuid::new_v4()),
                        is_system_role: Set(true),
                        created_at: Set(now.into()),
                        workspace_id: Set(workspace_id),
                        name: Set("Admin".to_string()),
                        permissions: Set(
                            permission::to_db(permission::admin())
                                .expect("admin permissions fit in i32"),
                        ),
                    };
                    let member_role = workspace_role::ActiveModel {
                        id: Set(Uuid::new_v4()),
                        is_system_role: Set(true),
                        created_at: Set(now.into()),
                        workspace_id: Set(workspace_id),
                        name: Set("Member".to_string()),
                        permissions: Set(
                            permission::to_db(permission::member())
                                .expect("member permissions fit in i32"),
                        ),
                    };
                    workspace_role::Entity::insert_many(vec![owner_role, admin_role, member_role])
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace role insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    // Create workspace member
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

                    // Create workspace member role
                    let member_role = workspace_member_role::ActiveModel {
                        workspace_id: Set(workspace_id),
                        user_id: Set(actor_user_id),
                        role_id: Set(owner_role_id),
                        assigned_at: Set(now.into()),
                        assigned_by_user_id: Set(None),
                    };
                    workspace_member_role::Entity::insert(member_role)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace member role insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    // Create workspace member added event
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
                            source: "workspace_create".to_string(),
                        })?),
                    };
                    outbox_event::Entity::insert(member_added_event)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace member added event insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    // Create workspace channel created event
                    let channel_created_event = outbox_event::ActiveModel {
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
                            name: first_channel_name.to_string(),
                            channel_kind: "text".to_string(),
                            position: 1,
                            created_by_user_id: actor_user_id.to_string(),
                            created_at: now.to_rfc3339(),
                        })?),
                    };
                    outbox_event::Entity::insert(channel_created_event)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace channel created event insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    // Create workspace created event
                    let event = outbox_event::ActiveModel {
                        event_id: Set(Uuid::new_v4()),
                        aggregate_type: Set("workspace".to_string()),
                        aggregate_id: Set(workspace_id),
                        event_type: Set("WorkspaceCreated".to_string()),
                        created_at: Set(now.into()),
                        available_at: Set(now.into()),
                        occurred_at: Set(now.into()),
                        claimed_at: Set(None),
                        claimed_by: Set(None),
                        published_at: Set(None),
                        last_error: Set(None),
                        publish_attempts: Set(0),
                        status: Set("pending".to_string()),
                        payload: Set(payload_value(WorkspaceCreatedPayload {
                            workspace_id: workspace_id.to_string(),
                            name: name.to_string(),
                            owner_user_id: actor_user_id.to_string(),
                            created_at: now.to_rfc3339(),
                            initial_member_user_id: actor_user_id.to_string(),
                        })?),
                    };
                    outbox_event::Entity::insert(event)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace created event insert failed");
                            Status::internal("Internal Server Error")
                        })?;

                    Ok(Response::new(CreateWorkspaceResponse {
                        workspace_id: workspace_id.to_string(),
                        name: name.to_string(),
                        owner_user_id: actor_user_id.to_string(),
                        created_at: Some(to_timestamp(now)),
                        first_channel_id: channel_id.to_string(),
                        initial_member_user_id: actor_user_id.to_string(),
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
