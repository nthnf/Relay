use chrono::DateTime;
use serde::Serialize;
use tonic::{Request, Status};
use uuid::Uuid;

pub const ACTOR_USER_ID_METADATA: &str = "x-user-id";

pub fn payload_value<T: Serialize>(payload: T) -> Result<serde_json::Value, Status> {
    serde_json::to_value(payload).map_err(|e| {
        tracing::error!(error = %e, "event payload serialization failed");
        Status::internal("internal server error")
    })
}

pub fn actor_user_id<T>(request: &Request<T>) -> Result<Uuid, Status> {
    let raw = request
        .metadata()
        .get(ACTOR_USER_ID_METADATA)
        .ok_or_else(|| Status::unauthenticated("missing authenticated actor context"))?;

    let raw = raw
        .to_str()
        .map_err(|_| Status::unauthenticated("invalid authenticated actor context"))?;

    Uuid::parse_str(raw).map_err(|_| Status::unauthenticated("invalid authenticated actor context"))
}

pub fn to_timestamp(dt: DateTime<chrono::Utc>) -> prost_types::Timestamp {
    prost_types::Timestamp {
        seconds: dt.timestamp(),
        nanos: dt.timestamp_subsec_nanos() as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let ts = to_timestamp(
            chrono::DateTime::parse_from_rfc3339("2026-04-14T12:34:56Z")
                .expect("datetime")
                .with_timezone(&chrono::Utc),
        );
        assert_eq!(ts.seconds, 1_776_170_096);
    }
}
