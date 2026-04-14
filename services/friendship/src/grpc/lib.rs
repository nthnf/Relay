use tonic::{Request, Status};
use uuid::Uuid;

use super::ACTOR_USER_ID_METADATA;

pub(super) fn actor_user_id<T>(request: &Request<T>) -> Result<Uuid, Status> {
    let raw = request
        .metadata()
        .get(ACTOR_USER_ID_METADATA)
        .ok_or_else(|| Status::unauthenticated("missing authenticated actor context"))?;

    let raw = raw
        .to_str()
        .map_err(|_| Status::unauthenticated("invalid authenticated actor context"))?;

    Uuid::parse_str(raw).map_err(|_| Status::unauthenticated("invalid authenticated actor context"))
}

pub(super) fn payload_value<T: serde::Serialize>(payload: T) -> serde_json::Value {
    serde_json::to_value(payload).expect("event payload should serialize")
}

pub(super) fn to_timestamp(dt: chrono::DateTime<chrono::Utc>) -> prost_types::Timestamp {
    prost_types::Timestamp {
        seconds: dt.timestamp(),
        nanos: dt.timestamp_subsec_nanos() as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actor_user_id_reads_metadata() {
        let user_id = Uuid::new_v4();
        let mut request = Request::new(());
        request.metadata_mut().insert(
            ACTOR_USER_ID_METADATA,
            user_id.to_string().parse().expect("valid metadata"),
        );

        assert_eq!(actor_user_id(&request).expect("actor id"), user_id);
    }

    #[test]
    fn actor_user_id_rejects_missing_metadata() {
        let request = Request::new(());

        let error = actor_user_id(&request).expect_err("missing actor context");
        assert_eq!(error.code(), tonic::Code::Unauthenticated);
    }

    #[test]
    fn payload_value_serializes_json() {
        let value = payload_value(serde_json::json!({"hello": "world"}));
        assert_eq!(value["hello"], "world");
    }

    #[test]
    fn to_timestamp_keeps_seconds() {
        let dt = chrono::DateTime::parse_from_rfc3339("2026-04-14T12:34:56Z")
            .expect("datetime")
            .with_timezone(&chrono::Utc);

        let ts = to_timestamp(dt);
        assert_eq!(ts.seconds, 1_776_170_096);
        assert_eq!(ts.nanos, 0);
    }
}
