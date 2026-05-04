use chrono::Utc;
use relay_proto::friendship::{
    BlockedUserSummary, ListBlockedUsersRequest, ListBlockedUsersResponse,
};
use sea_orm::{
    ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, TransactionError,
    TransactionTrait,
};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::entity::{user_block, user_snapshot};

use super::{handler::Handler, lib::user_summary};
use relay_types::{actor_user_id, to_timestamp};

impl Handler {
    pub(super) async fn list_blocked_users(
        &self,
        request: Request<ListBlockedUsersRequest>,
    ) -> Result<Response<ListBlockedUsersResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;
        let ListBlockedUsersRequest {
            page_size,
            page_token,
        } = request.into_inner();
        let page_size = page_size.unwrap_or(50).clamp(1, 100) as u64;

        let response = self
            .connection
            .transaction::<_, Response<ListBlockedUsersResponse>, Status>(|txn| {
                Box::pin(async move {
                    let mut query = user_block::Entity::find()
                        .filter(user_block::Column::BlockerUserId.eq(actor_user_id))
                        .order_by_desc(user_block::Column::CreatedAt)
                        .order_by_desc(user_block::Column::BlockedUserId)
                        .limit(page_size + 1);

                    if let Some(page_token) = page_token.as_deref() {
                        let cursor = decode_page_token(page_token)?;
                        query = query.filter(
                            sea_orm::Condition::any()
                                .add(user_block::Column::CreatedAt.lt(cursor.created_at))
                                .add(
                                    sea_orm::Condition::all()
                                        .add(user_block::Column::CreatedAt.eq(cursor.created_at))
                                        .add(
                                            user_block::Column::BlockedUserId
                                                .lt(cursor.blocked_user_id),
                                        ),
                                ),
                        );
                    }

                    let mut rows = query.all(txn).await.map_err(|e| {
                        error!(error = %e, "List blocked users query failed");
                        Status::internal("Internal Server Error")
                    })?;

                    let has_more = rows.len() > page_size as usize;
                    if has_more {
                        rows.pop();
                    }

                    let next_page_token = if has_more {
                        rows.last().map(encode_page_token)
                    } else {
                        None
                    };
                    let mut blocked_users = Vec::with_capacity(rows.len());

                    for row in rows {
                        let target = user_snapshot::Entity::find_by_id(row.blocked_user_id)
                            .one(txn)
                            .await
                            .map_err(|e| {
                                error!(error = %e, "Blocked user snapshot lookup failed");
                                Status::internal("Internal Server Error")
                            })?;

                        blocked_users.push(BlockedUserSummary {
                            target_user_id: row.blocked_user_id.to_string(),
                            blocked_at: Some(to_timestamp(row.created_at.with_timezone(&Utc))),
                            target: target.as_ref().map(user_summary),
                        });
                    }

                    Ok(Response::new(ListBlockedUsersResponse {
                        blocked_users,
                        next_page_token,
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "List blocked users transaction connection failure");
                    Status::internal("Internal Server Error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}

struct BlockedCursor {
    created_at: chrono::DateTime<Utc>,
    blocked_user_id: Uuid,
}

fn encode_page_token(cursor: &user_block::Model) -> String {
    format!(
        "{}|{}",
        cursor.created_at.with_timezone(&Utc).to_rfc3339(),
        cursor.blocked_user_id
    )
}

fn decode_page_token(page_token: &str) -> Result<BlockedCursor, Status> {
    let (created_at, blocked_user_id) = page_token
        .split_once('|')
        .ok_or_else(|| Status::invalid_argument("Invalid page token"))?;
    let created_at = chrono::DateTime::parse_from_rfc3339(created_at)
        .map_err(|_| Status::invalid_argument("Invalid page token"))?
        .with_timezone(&Utc);
    let blocked_user_id = Uuid::parse_str(blocked_user_id)
        .map_err(|_| Status::invalid_argument("Invalid page token"))?;

    Ok(BlockedCursor {
        created_at,
        blocked_user_id,
    })
}
