use super::handler::Handler;
use chrono::{DateTime, TimeZone, Utc};
use prost_types::Timestamp;
use relay_proto::realtime::{DisconnectActorSessionsRequest, DisconnectActorSessionsResponse};
use tonic::{Request, Response, Status};
use uuid::Uuid;

impl Handler {
    pub(super) async fn disconnect_actor_sessions(
        &self,
        request: Request<DisconnectActorSessionsRequest>,
    ) -> Result<Response<DisconnectActorSessionsResponse>, Status> {
        let DisconnectActorSessionsRequest {
            actor_user_id,
            reason_code: _,
            disconnect_before,
        } = request.into_inner();

        let actor_user_id = Uuid::parse_str(&actor_user_id)
            .map_err(|_| Status::invalid_argument("Invalid actor_user_id UUID"))?;

        let disconnect_before = to_dt(&disconnect_before);
        let session_ids: Vec<Uuid> = self
            .store
            .session_registry
            .iter()
            .filter_map(|entry| {
                let payload = entry.value();
                let actor_match = payload.actor_id == actor_user_id;
                let before_cutoff = disconnect_before
                    .map(|cutoff| payload.connected_at < cutoff)
                    .unwrap_or(true);

                if actor_match && before_cutoff {
                    Some(*entry.key())
                } else {
                    None
                }
            })
            .collect();

        let mut disconnected_session_count = 0;
        for session_id in session_ids {
            let Some(session) = self.store.remove_session(&session_id) else {
                continue;
            };

            self.store.remove_session_subscriptions(&session_id);
            let _ = session
                .sender
                .send(axum::extract::ws::Message::Close(None))
                .await;
            disconnected_session_count += 1;
        }

        Ok(Response::new(DisconnectActorSessionsResponse {
            accepted: true,
            disconnected_session_count,
        }))
    }
}

fn to_dt(ts: &Option<Timestamp>) -> Option<DateTime<Utc>> {
    ts.as_ref()
        .and_then(|t| Utc.timestamp_opt(t.seconds, t.nanos as u32).single())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::handler::Handler;
    use crate::store::Store;
    use chrono::Utc;
    use relay_proto::realtime::DisconnectActorSessionsRequest;
    use std::sync::Arc;

    #[tokio::test]
    async fn disconnect_actor_sessions_closes_all_actor_sessions() {
        let store = Arc::new(Store::new());
        let handler = Handler::new(store.clone());

        let actor_id = Uuid::new_v4();
        let other_actor_id = Uuid::new_v4();
        let session_one = Uuid::new_v4();
        let session_two = Uuid::new_v4();
        let other_session = Uuid::new_v4();
        let connected_at = Utc::now();

        let (sender_one, mut receiver_one) = tokio::sync::mpsc::channel(1);
        let (sender_two, mut receiver_two) = tokio::sync::mpsc::channel(1);
        let (sender_other, mut receiver_other) = tokio::sync::mpsc::channel(1);

        store.create_session(session_one, actor_id, connected_at, sender_one);
        store.create_session(session_two, actor_id, connected_at, sender_two);
        store.create_session(other_session, other_actor_id, connected_at, sender_other);

        let request = DisconnectActorSessionsRequest {
            actor_user_id: actor_id.to_string(),
            reason_code: "session_revoked".to_owned(),
            disconnect_before: None,
        };

        let response = handler
            .disconnect_actor_sessions(tonic::Request::new(request))
            .await
            .expect("disconnect_actor_sessions failed")
            .into_inner();

        assert!(response.accepted);
        assert_eq!(response.disconnected_session_count, 2);
        assert!(store.get_session(&session_one).is_none());
        assert!(store.get_session(&session_two).is_none());
        assert!(store.get_session(&other_session).is_some());

        assert!(matches!(
            receiver_one.recv().await,
            Some(axum::extract::ws::Message::Close(_))
        ));
        assert!(matches!(
            receiver_two.recv().await,
            Some(axum::extract::ws::Message::Close(_))
        ));
        assert!(receiver_other.try_recv().is_err());
    }
}
