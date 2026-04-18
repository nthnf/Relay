use super::handler::Handler;
use relay_proto::realtime::{
    DeliverMessageRequest, DeliverMessageResponse, DeliverTargetKind, deliver_message_request,
};
use serde_json::json;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::store::TargetKind;

impl Handler {
    pub(super) async fn deliver_message(
        &self,
        request: Request<DeliverMessageRequest>,
    ) -> Result<Response<DeliverMessageResponse>, Status> {
        let DeliverMessageRequest {
            delivery_id,
            target_kind,
            target_id,
            occurred_at,
            payload,
        } = request.into_inner();

        let delivery_id = Uuid::parse_str(&delivery_id)
            .map_err(|_| Status::invalid_argument("Invalid delivery_id UUID"))?;

        let target_id = Uuid::parse_str(&target_id)
            .map_err(|_| Status::invalid_argument("Invalid target_id UUID"))?;

        let target_kind = match DeliverTargetKind::try_from(target_kind)
            .map_err(|_| Status::invalid_argument("Invalid target kind"))?
        {
            DeliverTargetKind::DirectMessage => TargetKind::DirectMessage,
            DeliverTargetKind::WorkspaceChannel => TargetKind::WorkspaceChannel,
            DeliverTargetKind::TargetKindUnspecified => {
                return Err(Status::invalid_argument("Invalid target kind"));
            }
        };

        let to_rfc3339 = |timestamp: Option<prost_types::Timestamp>, field: &'static str| {
            timestamp
                .and_then(|ts| {
                    chrono::DateTime::<chrono::Utc>::from_timestamp(ts.seconds, ts.nanos as u32)
                })
                .map(|dt| dt.to_rfc3339())
                .ok_or_else(|| Status::invalid_argument(format!("{field} is required")))
        };

        let occurred_at = to_rfc3339(occurred_at, "occurred_at")?;

        let payload = match payload {
            Some(deliver_message_request::Payload::MessageCreated(payload)) => json!({
                "type": "message_created",
                "message_id": payload.message_id,
                "author_user_id": payload.author_user_id,
                "body": payload.body,
                "target_message_seq": payload.target_message_seq,
                "created_at": to_rfc3339(payload.created_at, "created_at")?,
            }),
            Some(deliver_message_request::Payload::MessageEdited(payload)) => json!({
                "type": "message_edited",
                "message_id": payload.message_id,
                "editor_user_id": payload.editor_user_id,
                "body": payload.body,
                "edit_version": payload.edit_version,
                "target_message_seq": payload.target_message_seq,
                "edited_at": to_rfc3339(payload.edited_at, "edited_at")?,
            }),
            Some(deliver_message_request::Payload::MessageDeleted(payload)) => json!({
                "type": "message_deleted",
                "message_id": payload.message_id,
                "deleted_by_user_id": payload.deleted_by_user_id,
                "target_message_seq": payload.target_message_seq,
                "deleted_at": to_rfc3339(payload.deleted_at, "deleted_at")?,
            }),
            None => return Err(Status::invalid_argument("payload is required")),
        };

        let Some(subscription) = self.store.get_subscription(&target_kind, &target_id) else {
            return Ok(Response::new(DeliverMessageResponse {
                accepted: true,
                attempted_recipient_count: 0,
                delivered_session_count: 0,
            }));
        };

        let envelope = json!({
            "delivery_id": delivery_id.to_string(),
            "target_kind": match target_kind {
                TargetKind::DirectMessage => "direct_message",
                TargetKind::WorkspaceChannel => "workspace_channel",
            },
            "target_id": target_id.to_string(),
            "occurred_at": occurred_at,
            "payload": payload,
        });

        let envelope_text = serde_json::to_string(&envelope)
            .map_err(|_| Status::internal("failed to serialize delivery envelope"))?;

        let mut attempted_recipient_count = 0;
        let mut delivered_session_count = 0;
        let session_ids: Vec<Uuid> = subscription
            .session_ids
            .iter()
            .map(|entry| *entry.key())
            .collect();

        for session_id in session_ids {
            attempted_recipient_count += 1;

            let Some(session) = self.store.get_session(&session_id) else {
                self.store.remove_session_subscriptions(&session_id);
                self.store.remove_session(&session_id);
                continue;
            };

            if session
                .sender
                .send(axum::extract::ws::Message::Text(
                    envelope_text.clone().into(),
                ))
                .await
                .is_ok()
            {
                delivered_session_count += 1;
                continue;
            }

            self.store.remove_session_subscriptions(&session_id);
            self.store.remove_session(&session_id);
        }

        Ok(Response::new(DeliverMessageResponse {
            accepted: true,
            attempted_recipient_count,
            delivered_session_count,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::handler::Handler;
    use crate::store::Store;
    use chrono::TimeZone;
    use prost_types::Timestamp;
    use relay_proto::realtime::deliver_message_request;
    use serde_json::Value;
    use std::sync::Arc;

    fn ts(seconds: i64) -> Timestamp {
        Timestamp { seconds, nanos: 0 }
    }

    fn dt(seconds: i64) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc
            .timestamp_opt(seconds, 0)
            .single()
            .expect("valid timestamp")
    }

    #[tokio::test]
    async fn deliver_message_sends_envelope_to_session_sender() {
        let store = Arc::new(Store::new());
        let handler = Handler::new(store.clone());

        let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
        let session_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();
        let delivery_id = Uuid::new_v4();

        store.create_session(session_id, actor_id, dt(1_700_000_000), sender);
        store.create_session_subscription(
            TargetKind::WorkspaceChannel,
            target_id,
            session_id,
            actor_id,
            dt(1_700_000_001),
        );

        let request = DeliverMessageRequest {
            delivery_id: delivery_id.to_string(),
            target_kind: DeliverTargetKind::WorkspaceChannel as i32,
            target_id: target_id.to_string(),
            occurred_at: Some(ts(1_700_000_002)),
            payload: Some(deliver_message_request::Payload::MessageCreated(
                relay_proto::realtime::MessageCreatedPayload {
                    message_id: Uuid::new_v4().to_string(),
                    author_user_id: actor_id.to_string(),
                    body: "hello".to_owned(),
                    target_message_seq: 42,
                    created_at: Some(ts(1_700_000_003)),
                },
            )),
        };

        let response = handler
            .deliver_message(tonic::Request::new(request))
            .await
            .expect("deliver_message failed")
            .into_inner();

        assert!(response.accepted);
        assert_eq!(response.attempted_recipient_count, 1);
        assert_eq!(response.delivered_session_count, 1);

        let outbound = receiver.recv().await.expect("missing outbound message");
        let text = match outbound {
            axum::extract::ws::Message::Text(text) => text.to_string(),
            other => panic!("unexpected outbound message: {other:?}"),
        };

        let json: Value = serde_json::from_str(&text).expect("invalid envelope json");
        assert_eq!(json["delivery_id"], delivery_id.to_string());
        assert_eq!(json["target_kind"], "workspace_channel");
        assert_eq!(json["target_id"], target_id.to_string());
        assert_eq!(json["payload"]["type"], "message_created");
        assert_eq!(json["payload"]["message_id"].as_str().unwrap().len(), 36);
        assert_eq!(json["payload"]["author_user_id"], actor_id.to_string());
        assert_eq!(json["payload"]["body"], "hello");
        assert_eq!(json["payload"]["target_message_seq"], 42);
    }
}
