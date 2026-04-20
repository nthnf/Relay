use chrono::Utc;
use relay_proto::workspace::{
    ListWorkspacesForUserRequest, ListWorkspacesForUserResponse, WorkspaceSummary,
};
use sea_orm::{
    ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
    TransactionError, TransactionTrait,
};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::entity::{workspace, workspace_channel, workspace_member};

use super::handler::Handler;
use relay_types::{actor_user_id, to_timestamp};

impl Handler {
    pub(super) async fn list_workspaces_for_user(
        &self,
        request: Request<ListWorkspacesForUserRequest>,
    ) -> Result<Response<ListWorkspacesForUserResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let ListWorkspacesForUserRequest {
            page_size,
            page_token,
        } = request.into_inner();

        let page_size = page_size.unwrap_or(20).clamp(1, 50) as u64;

        let response = self
            .connection
            .transaction::<_, Response<ListWorkspacesForUserResponse>, Status>(|txn| {
                Box::pin(async move {
                    let mut query = workspace_member::Entity::find()
                        .find_also_related(workspace::Entity)
                        .filter(workspace_member::Column::UserId.eq(actor_user_id))
                        .filter(workspace_member::Column::MembershipStatus.eq("active"))
                        .order_by_desc(workspace_member::Column::JoinedAt)
                        .order_by_desc(workspace_member::Column::WorkspaceId)
                        .limit(page_size + 1);

                    if let Some(page_token) = page_token.as_deref() {
                        let cursor = decode_page_token(page_token)?;
                        query = query.filter(
                            sea_orm::Condition::any()
                                .add(workspace_member::Column::JoinedAt.lt(cursor.joined_at))
                                .add(
                                    sea_orm::Condition::all()
                                        .add(
                                            workspace_member::Column::JoinedAt.eq(cursor.joined_at),
                                        )
                                        .add(
                                            workspace_member::Column::WorkspaceId
                                                .lt(cursor.workspace_id),
                                        ),
                                ),
                        );
                    }

                    let mut rows: Vec<(workspace_member::Model, Option<workspace::Model>)> =
                        query.all(txn).await.map_err(|e| {
                            error!(error = %e, "List workspace query failed");
                            Status::internal("Internal Server Error")
                        })?;

                    let has_more = rows.len() > page_size as usize;
                    if has_more {
                        rows.pop();
                    }

                    let next_page_token = if has_more {
                        rows.last().map(|(member, _)| encode_page_token(member))
                    } else {
                        None
                    };

                    let mut workspaces = Vec::with_capacity(rows.len());
                    for (member, workspace) in rows {
                        let Some(workspace) = workspace else {
                            return Err(Status::internal("Internal Server Error"));
                        };

                        let member_count = workspace_member::Entity::find()
                            .filter(workspace_member::Column::WorkspaceId.eq(member.workspace_id))
                            .filter(workspace_member::Column::MembershipStatus.eq("active"))
                            .count(txn)
                            .await
                            .map_err(|e| {
                                error!(error = %e, "Workspace member count failed");
                                Status::internal("Internal Server Error")
                            })? as i32;

                        let channel_count = workspace_channel::Entity::find()
                            .filter(workspace_channel::Column::WorkspaceId.eq(member.workspace_id))
                            .count(txn)
                            .await
                            .map_err(|e| {
                                error!(error = %e, "Workspace channel count failed");
                                Status::internal("Internal Server Error")
                            })? as i32;

                        workspaces.push(WorkspaceSummary {
                            workspace_id: workspace.id.to_string(),
                            name: workspace.name,
                            member_count,
                            channel_count,
                            joined_at: Some(to_timestamp(member.joined_at.into())),
                        });
                    }

                    Ok(Response::new(ListWorkspacesForUserResponse {
                        workspaces,
                        next_page_token,
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "List friends transaction connection failure");
                    Status::internal("Internal Server Error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}

struct WorkspaceCursor {
    joined_at: chrono::DateTime<Utc>,
    workspace_id: Uuid,
}

fn encode_page_token(cursor: &workspace_member::Model) -> String {
    format!(
        "{}|{}",
        cursor.joined_at.with_timezone(&Utc).to_rfc3339(),
        cursor.workspace_id
    )
}

fn decode_page_token(page_token: &str) -> Result<WorkspaceCursor, Status> {
    let (joined_at, workspace_id) = page_token
        .split_once('|')
        .ok_or_else(|| Status::invalid_argument("Invalid page token"))?;

    let joined_at = chrono::DateTime::parse_from_rfc3339(joined_at)
        .map_err(|_| Status::invalid_argument("Invalid page token"))?
        .with_timezone(&Utc);

    let workspace_id = Uuid::parse_str(workspace_id)
        .map_err(|_| Status::invalid_argument("Invalid page token"))?;

    Ok(WorkspaceCursor {
        joined_at,
        workspace_id,
    })
}
