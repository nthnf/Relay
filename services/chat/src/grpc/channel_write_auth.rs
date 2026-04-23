use relay_proto::workspace::{AuthorizeChannelActionRequest, ChannelAction};
use sea_orm::{ConnectionTrait, EntityTrait};
use tonic::{metadata::MetadataValue, Request, Status};
use uuid::Uuid;

use crate::entity::{conversation, workspace_channel_snapshot, workspace_snapshot};

use relay_types::ACTOR_USER_ID_METADATA;

#[derive(Clone, Copy)]
pub(super) struct ChannelWriteContext {
    pub workspace_id: Uuid,
    pub workspace_channel_id: Uuid,
}

pub(super) async fn authorize_channel_read(
    connection: &impl ConnectionTrait,
    workspace_client: &mut relay_proto::workspace::workspace_service_client::WorkspaceServiceClient<tonic::transport::Channel>,
    actor_user_id: Uuid,
    conversation: &conversation::Model,
) -> Result<ChannelWriteContext, Status> {
    authorize_channel_action(
        connection,
        workspace_client,
        actor_user_id,
        conversation,
        ChannelAction::Read,
    )
    .await
}

pub(super) async fn authorize_channel_write(
    connection: &impl ConnectionTrait,
    workspace_client: &mut relay_proto::workspace::workspace_service_client::WorkspaceServiceClient<tonic::transport::Channel>,
    actor_user_id: Uuid,
    conversation: &conversation::Model,
) -> Result<ChannelWriteContext, Status> {
    authorize_channel_action(
        connection,
        workspace_client,
        actor_user_id,
        conversation,
        ChannelAction::Write,
    )
    .await
}

async fn authorize_channel_action(
    connection: &impl ConnectionTrait,
    workspace_client: &mut relay_proto::workspace::workspace_service_client::WorkspaceServiceClient<tonic::transport::Channel>,
    actor_user_id: Uuid,
    conversation: &conversation::Model,
    action: ChannelAction,
) -> Result<ChannelWriteContext, Status> {
    let workspace_channel_id = conversation
        .workspace_channel_id
        .ok_or_else(|| Status::not_found("Conversation not found"))?;

    let workspace_channel_snapshot = workspace_channel_snapshot::Entity::find_by_id(workspace_channel_id)
        .one(connection)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Workspace channel snapshot lookup failed");
            Status::internal("Internal Server Error")
        })?
        .ok_or_else(|| Status::not_found("Workspace channel not found"))?;

    let workspace_snapshot = workspace_snapshot::Entity::find_by_id(workspace_channel_snapshot.workspace_id)
        .one(connection)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Workspace snapshot lookup failed");
            Status::internal("Internal Server Error")
        })?
        .ok_or_else(|| Status::not_found("Workspace not found"))?;

    let mut authorize_request = Request::new(AuthorizeChannelActionRequest {
        workspace_id: workspace_snapshot.workspace_id.to_string(),
        channel_id: workspace_channel_id.to_string(),
        action: action as i32,
    });
    authorize_request.metadata_mut().insert(
        ACTOR_USER_ID_METADATA,
        MetadataValue::try_from(actor_user_id.to_string())
            .map_err(|_| Status::internal("Internal Server Error"))?,
    );

    let authorize = workspace_client
        .authorize_channel_action(authorize_request)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Workspace authorize_channel_action failed");
            Status::internal("Internal Server Error")
        })?
        .into_inner();

    if !authorize.allowed {
        return Err(Status::permission_denied("Permission denied"));
    }

    Ok(ChannelWriteContext {
        workspace_id: workspace_snapshot.workspace_id,
        workspace_channel_id,
    })
}
