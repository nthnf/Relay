use sea_orm::{ConnectionTrait, EntityTrait};
use tonic::Status;
use uuid::Uuid;

use crate::entity::user_account;

pub(super) async fn user_account_exists<C>(db: &C, user_id: Uuid) -> Result<bool, Status>
where
    C: ConnectionTrait,
{
    let account = user_account::Entity::find_by_id(user_id)
        .one(db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "friendship user account lookup failed");
            Status::internal("Internal Server Error")
        })?;

    Ok(account.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use relay_types::{ACTOR_USER_ID_METADATA, actor_user_id, payload_value, to_timestamp};
    use tonic::Request;

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
        let value = payload_value(serde_json::json!({"hello": "world"})).expect("serialize");
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
