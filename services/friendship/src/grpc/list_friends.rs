use chrono::Utc;
use relay_proto::friendship::{ListFriendsRequest, ListFriendsResponse};
use sea_orm::{
    ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, TransactionError,
    TransactionTrait,
};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::entity::friendship_edge;

use super::handler::Handler;
use relay_types::{actor_user_id, to_timestamp};

impl Handler {
    pub(super) async fn list_friends(
        &self,
        request: Request<ListFriendsRequest>,
    ) -> Result<Response<ListFriendsResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;

        let ListFriendsRequest {
            page_size,
            page_token,
        } = request.into_inner();

        let page_size = page_size.unwrap_or(20).clamp(1, 50) as u64;

        let response = self
            .connection
            .transaction::<_, Response<ListFriendsResponse>, Status>(|txn| {
                Box::pin(async move {
                    let mut query = friendship_edge::Entity::find()
                        .filter(friendship_edge::Column::UserId.eq(actor_user_id))
                        .order_by_desc(friendship_edge::Column::AcceptedAt)
                        .order_by_desc(friendship_edge::Column::FriendUserId)
                        .limit(page_size + 1);

                    if let Some(page_token) = page_token.as_deref() {
                        let cursor = decode_page_token(page_token)?;
                        query = query.filter(
                            sea_orm::Condition::any()
                                .add(friendship_edge::Column::AcceptedAt.lt(cursor.accepted_at))
                                .add(
                                    sea_orm::Condition::all()
                                        .add(
                                            friendship_edge::Column::AcceptedAt
                                                .eq(cursor.accepted_at),
                                        )
                                        .add(
                                            friendship_edge::Column::FriendUserId
                                                .lt(cursor.friend_user_id),
                                        ),
                                ),
                        );
                    }

                    let mut rows = query.all(txn).await.map_err(|e| {
                        error!(error = %e, "List friends query failed");
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

                    Ok(Response::new(ListFriendsResponse {
                        friends: rows
                            .into_iter()
                            .map(|row| relay_proto::friendship::FriendshipEdge {
                                friend_user_id: row.friend_user_id.to_string(),
                                accepted_at: Some(to_timestamp(
                                    row.accepted_at.with_timezone(&Utc),
                                )),
                            })
                            .collect(),
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

struct FriendCursor {
    accepted_at: chrono::DateTime<Utc>,
    friend_user_id: Uuid,
}

fn encode_page_token(cursor: &friendship_edge::Model) -> String {
    format!(
        "{}|{}",
        cursor.accepted_at.with_timezone(&Utc).to_rfc3339(),
        cursor.friend_user_id
    )
}

fn decode_page_token(page_token: &str) -> Result<FriendCursor, Status> {
    let (accepted_at, friend_user_id) = page_token
        .split_once('|')
        .ok_or_else(|| Status::invalid_argument("Invalid page token"))?;

    let accepted_at = chrono::DateTime::parse_from_rfc3339(accepted_at)
        .map_err(|_| Status::invalid_argument("Invalid page token"))?
        .with_timezone(&Utc);

    let friend_user_id = Uuid::parse_str(friend_user_id)
        .map_err(|_| Status::invalid_argument("Invalid page token"))?;

    Ok(FriendCursor {
        accepted_at,
        friend_user_id,
    })
}
