use relay_proto::realtime::{GetUserPresenceRequest, GetUserPresenceResponse, UserPresenceSummary};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use super::handler::Handler;

impl Handler {
    pub(super) async fn get_user_presence(
        &self,
        request: Request<GetUserPresenceRequest>,
    ) -> Result<Response<GetUserPresenceResponse>, Status> {
        let redis = self
            .redis
            .as_ref()
            .ok_or_else(|| Status::failed_precondition("presence store unavailable"))?;
        let GetUserPresenceRequest { user_ids } = request.into_inner();
        let mut users = Vec::with_capacity(user_ids.len());

        for user_id in user_ids {
            let parsed_user_id = Uuid::parse_str(&user_id)
                .map_err(|_| Status::invalid_argument(format!("invalid user_id: {user_id}")))?;
            let presence = redis.get_presence(parsed_user_id).await.map_err(|e| {
                error!(error = %e, %parsed_user_id, "realtime presence lookup failed");
                Status::internal("internal server error")
            })?;

            users.push(UserPresenceSummary {
                user_id,
                online: presence.online,
                last_seen_at: presence.last_seen_at.map(to_timestamp),
            });
        }

        Ok(Response::new(GetUserPresenceResponse { users }))
    }
}

fn to_timestamp(value: chrono::DateTime<chrono::Utc>) -> prost_types::Timestamp {
    prost_types::Timestamp {
        seconds: value.timestamp(),
        nanos: value.timestamp_subsec_nanos() as i32,
    }
}
