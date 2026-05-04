use relay_proto::workspace::{
    ListWorkspaceMembersRequest, ListWorkspaceMembersResponse, WorkspaceMemberSummary,
};
use sea_orm::{
    ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, TransactionError,
    TransactionTrait,
};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::entity::{user_snapshot, workspace, workspace_member};

use super::handler::Handler;
use relay_types::{actor_user_id, to_timestamp};

impl Handler {
    pub(super) async fn list_workspace_members(
        &self,
        request: Request<ListWorkspaceMembersRequest>,
    ) -> Result<Response<ListWorkspaceMembersResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let ListWorkspaceMembersRequest {
            workspace_id,
            page_size,
            page_token,
        } = request.into_inner();
        let workspace_id = Uuid::parse_str(&workspace_id)
            .map_err(|_| Status::invalid_argument("Invalid workspace id"))?;
        let page_size = page_size.unwrap_or(100).clamp(1, 200) as u64;

        let response = self
            .connection
            .transaction::<_, Response<ListWorkspaceMembersResponse>, Status>(|txn| {
                Box::pin(async move {
                    let workspace = workspace::Entity::find_by_id(workspace_id)
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "Workspace lookup failed");
                            Status::internal("Internal Server Error")
                        })?;

                    if workspace.as_ref().is_none_or(|workspace| workspace.archived_at.is_some()) {
                        return Err(Status::not_found("Workspace not found"));
                    }

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

                    let mut query = workspace_member::Entity::find()
                        .filter(workspace_member::Column::WorkspaceId.eq(workspace_id))
                        .filter(workspace_member::Column::MembershipStatus.eq("active"))
                        .order_by_asc(workspace_member::Column::JoinedAt)
                        .order_by_asc(workspace_member::Column::UserId)
                        .limit(page_size + 1);

                    if let Some(page_token) = page_token.as_deref() {
                        let cursor = decode_page_token(page_token)?;
                        query = query.filter(
                            sea_orm::Condition::any()
                                .add(workspace_member::Column::JoinedAt.gt(cursor.joined_at))
                                .add(
                                    sea_orm::Condition::all()
                                        .add(workspace_member::Column::JoinedAt.eq(cursor.joined_at))
                                        .add(workspace_member::Column::UserId.gt(cursor.user_id)),
                                ),
                        );
                    }

                    let mut rows = query.all(txn).await.map_err(|e| {
                        error!(error = %e, "List workspace members query failed");
                        Status::internal("Internal Server Error")
                    })?;

                    let has_more = rows.len() > page_size as usize;
                    if has_more {
                        rows.pop();
                    }

                    let next_page_token = if has_more { rows.last().map(encode_page_token) } else { None };
                    let mut members = Vec::with_capacity(rows.len());

                    for row in rows {
                        let profile = user_snapshot::Entity::find_by_id(row.user_id)
                            .one(txn)
                            .await
                            .map_err(|e| {
                                error!(error = %e, "Workspace member snapshot lookup failed");
                                Status::internal("Internal Server Error")
                            })?;

                        let (username, display_name, avatar_url) = profile
                            .map(|profile| (profile.username, profile.display_name, profile.avatar_url))
                            .unwrap_or_else(|| (row.user_id.to_string(), row.user_id.to_string(), None));

                        members.push(WorkspaceMemberSummary {
                            user_id: row.user_id.to_string(),
                            username,
                            display_name,
                            avatar_url,
                            joined_at: Some(to_timestamp(row.joined_at.into())),
                            added_by_user_id: row.added_by_user_id.map(|user_id| user_id.to_string()),
                        });
                    }

                    Ok(Response::new(ListWorkspaceMembersResponse { members, next_page_token }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "List workspace members transaction connection failure");
                    Status::internal("Internal Server Error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}

struct MemberCursor {
    joined_at: chrono::DateTime<chrono::FixedOffset>,
    user_id: Uuid,
}

fn encode_page_token(cursor: &workspace_member::Model) -> String {
    format!("{}|{}", cursor.joined_at.to_rfc3339(), cursor.user_id)
}

fn decode_page_token(page_token: &str) -> Result<MemberCursor, Status> {
    let (joined_at, user_id) = page_token
        .split_once('|')
        .ok_or_else(|| Status::invalid_argument("Invalid page token"))?;
    let joined_at = chrono::DateTime::parse_from_rfc3339(joined_at)
        .map_err(|_| Status::invalid_argument("Invalid page token"))?;
    let user_id =
        Uuid::parse_str(user_id).map_err(|_| Status::invalid_argument("Invalid page token"))?;

    Ok(MemberCursor { joined_at, user_id })
}
