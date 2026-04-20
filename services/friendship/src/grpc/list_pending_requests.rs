use chrono::Utc;
use relay_proto::friendship::{
    FriendRequestRecord, ListPendingRequestsRequest, ListPendingRequestsResponse,
};
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect, TransactionError,
    TransactionTrait,
};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::entity::friend_request;

use super::handler::Handler;
use relay_types::{actor_user_id, to_timestamp};

impl Handler {
    pub(super) async fn list_pending_requests(
        &self,
        request: Request<ListPendingRequestsRequest>,
    ) -> Result<Response<ListPendingRequestsResponse>, Status> {
        let actor_user_id = actor_user_id(&request)?;

        let ListPendingRequestsRequest {
            direction,
            page_size,
            page_token,
        } = request.into_inner();

        let page_size = page_size.unwrap_or(20).clamp(1, 50) as u64;
        let direction = direction.unwrap_or_else(|| "incoming".to_string());

        let response = self
            .connection
            .transaction::<_, Response<ListPendingRequestsResponse>, Status>(|txn| {
                Box::pin(async move {
                    let mut query = friend_request::Entity::find()
                        .filter(friend_request::Column::Status.eq("pending"))
                        .order_by_desc(friend_request::Column::CreatedAt)
                        .order_by_desc(friend_request::Column::RequestId)
                        .limit(page_size + 1);

                    match direction.as_str() {
                        "incoming" => {
                            query = query
                                .filter(friend_request::Column::AddresseeUserId.eq(actor_user_id));
                        }
                        "outgoing" => {
                            query = query
                                .filter(friend_request::Column::RequesterUserId.eq(actor_user_id));
                        }
                        "all" => {
                            query = query.filter(
                                Condition::any()
                                    .add(friend_request::Column::RequesterUserId.eq(actor_user_id))
                                    .add(friend_request::Column::AddresseeUserId.eq(actor_user_id)),
                            );
                        }
                        _ => return Err(Status::invalid_argument("Invalid direction")),
                    }

                    if let Some(page_token) = page_token.as_deref() {
                        let cursor = decode_page_token(page_token)?;
                        query = query.filter(
                            Condition::any()
                                .add(friend_request::Column::CreatedAt.lt(cursor.created_at))
                                .add(
                                    Condition::all()
                                        .add(
                                            friend_request::Column::CreatedAt.eq(cursor.created_at),
                                        )
                                        .add(
                                            friend_request::Column::RequestId.lt(cursor.request_id),
                                        ),
                                ),
                        );
                    }

                    let mut rows = query.all(txn).await.map_err(|e| {
                        error!(error = %e, "List pending request query failed");
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

                    Ok(Response::new(ListPendingRequestsResponse {
                        requests: rows
                            .into_iter()
                            .map(|row| FriendRequestRecord {
                                friend_request_id: row.request_id.to_string(),
                                requester_user_id: row.requester_user_id.to_string(),
                                addressee_user_id: row.addressee_user_id.to_string(),
                                status: row.status,
                                created_at: Some(to_timestamp(row.created_at.with_timezone(&Utc))),
                            })
                            .collect(),
                        next_page_token,
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "List pending request transaction connection failure");
                    Status::internal("Internal Server Error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}

struct RequestCursor {
    created_at: chrono::DateTime<Utc>,
    request_id: Uuid,
}

fn encode_page_token(cursor: &friend_request::Model) -> String {
    format!(
        "{}|{}",
        cursor.created_at.with_timezone(&Utc).to_rfc3339(),
        cursor.request_id
    )
}

fn decode_page_token(page_token: &str) -> Result<RequestCursor, Status> {
        let (created_at, request_id) = page_token
        .split_once('|')
        .ok_or_else(|| Status::invalid_argument("Invalid page token"))?;

    let created_at = chrono::DateTime::parse_from_rfc3339(created_at)
        .map_err(|_| Status::invalid_argument("Invalid page token"))?
        .with_timezone(&Utc);

        let request_id = Uuid::parse_str(request_id)
            .map_err(|_| Status::invalid_argument("Invalid page token"))?;

    Ok(RequestCursor {
        created_at,
        request_id,
    })
}
