use chrono::Utc;
use relay_proto::identity::{RegisterUserRequest, RegisterUserResponse};
use sea_orm::{
    ColumnTrait, EntityTrait, QueryFilter, Set, SqlErr, TransactionError, TransactionTrait,
};
use tonic::{Request, Response, Status};
use tracing::error;
use uuid::Uuid;

use crate::auth::{hash_password, hash_token};
use crate::entity::{
    email_verification_token, outbox_event, user_account, user_credential_password, user_profile,
};
use crate::events::{UserRegisteredPayload, VerificationEmailRequestedPayload};

use super::handler::{
    EMAIL_NORMALIZED_CONSTRAINT, Handler, USERNAME_CONSTRAINT, payload_value, to_timestamp,
};

impl Handler {
    pub(super) async fn register_user(
        &self,
        request: Request<RegisterUserRequest>,
    ) -> Result<Response<RegisterUserResponse>, Status> {
        let RegisterUserRequest {
            email,
            password,
            username,
            display_name,
            avatar_url,
        } = request.into_inner();

        let email_normalized = email.to_lowercase();
        if username.contains('#') {
            return Err(Status::invalid_argument("username must not contain #"));
        }
        let username = format!("{}#{}", username, random_discriminator());

        let existing_user = user_account::Entity::find()
            .filter(user_account::Column::EmailNormalized.eq(&email_normalized))
            .one(&self.connection)
            .await
            .map_err(|e| {
                error!(error = %e, "identity registration lookup failed");
                Status::internal("internal server error")
            })?;

        if existing_user.is_some() {
            return Err(Status::already_exists(
                "A user with this email already exists",
            ));
        }

        let user_id = Uuid::new_v4();
        let now = Utc::now();
        let verification_token = Uuid::new_v4().to_string();
        let verification_token_hash = hash_token(&verification_token);
        let verification_token_id = Uuid::new_v4();

        let response = self
            .connection
            .transaction::<_, Response<RegisterUserResponse>, Status>(|txn| {
                Box::pin(async move {
                    let account = user_account::ActiveModel {
                        user_id: Set(user_id),
                        email: Set(email.clone()),
                        email_normalized: Set(email_normalized.clone()),
                        email_verified_at: Set(None),
                        account_status: Set("active".to_string()),
                        created_at: Set(now.into()),
                        updated_at: Set(now.into()),
                    };
                    user_account::Entity::insert(account)
                        .exec(txn)
                        .await
                        .map_err(|e| match e.sql_err() {
                            Some(SqlErr::UniqueConstraintViolation(message))
                                if message.contains(EMAIL_NORMALIZED_CONSTRAINT)
                                    || message.contains(USERNAME_CONSTRAINT) =>
                            {
                                Status::already_exists("email or username already exists")
                            }
                            _ => {
                                let message = e.to_string();
                                if message.contains(EMAIL_NORMALIZED_CONSTRAINT)
                                    || message.contains(USERNAME_CONSTRAINT)
                                {
                                    Status::already_exists("email or username already exists")
                                } else {
                                    error!(error = %e, "identity registration account insert failed");
                                    Status::internal("internal server error")
                                }
                            }
                        })?;

                    let profile = user_profile::ActiveModel {
                        user_id: Set(user_id),
                        username: Set(username.clone()),
                        display_name: Set(display_name.clone()),
                        avatar_url: Set(avatar_url.clone()),
                        created_at: Set(now.into()),
                        updated_at: Set(now.into()),
                    };
                    user_profile::Entity::insert(profile)
                        .exec(txn)
                        .await
                        .map_err(|e| match e.sql_err() {
                            Some(SqlErr::UniqueConstraintViolation(message))
                                if message.contains(EMAIL_NORMALIZED_CONSTRAINT)
                                    || message.contains(USERNAME_CONSTRAINT) =>
                            {
                                Status::already_exists("email or username already exists")
                            }
                            _ => {
                                let message = e.to_string();
                                if message.contains(EMAIL_NORMALIZED_CONSTRAINT)
                                    || message.contains(USERNAME_CONSTRAINT)
                                {
                                    Status::already_exists("email or username already exists")
                                } else {
                                    error!(error = %e, "identity registration profile insert failed");
                                    Status::internal("internal server error")
                                }
                            }
                        })?;

                    let credential = user_credential_password::ActiveModel {
                        user_id: Set(user_id),
                        password_hash: Set(hash_password(&password).map_err(|e| {
                            error!(error = %e, "identity password hashing failed");
                            Status::internal("internal server error")
                        })?),
                        password_updated_at: Set(now.into()),
                        failed_attempt_count: Set(0),
                        created_at: Set(now.into()),
                        updated_at: Set(now.into()),
                    };
                    user_credential_password::Entity::insert(credential)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "identity registration credential insert failed");
                            Status::internal("internal server error")
                        })?;

                    let verification_token_model = email_verification_token::ActiveModel {
                        token_id: Set(verification_token_id),
                        token_hash: Set(verification_token_hash.clone()),
                        user_id: Set(user_id),
                        created_at: Set(now.into()),
                        expires_at: Set((now + chrono::Duration::hours(6)).into()),
                        consumed_at: Set(None),
                    };
                    email_verification_token::Entity::insert(verification_token_model)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "identity registration verification token insert failed");
                            Status::internal("internal server error")
                        })?;

                    let user_registered_event = outbox_event::ActiveModel {
                        event_id: Set(Uuid::new_v4()),
                        aggregate_type: Set("user_account".to_string()),
                        aggregate_id: Set(user_id),
                        event_type: Set("UserRegistered".to_string()),
                        created_at: Set(now.into()),
                        available_at: Set(now.into()),
                        occurred_at: Set(now.into()),
                        claimed_at: Set(None),
                        claimed_by: Set(None),
                        published_at: Set(None),
                        last_error: Set(None),
                        publish_attempts: Set(0),
                        status: Set("pending".to_string()),
                        payload: Set(payload_value(UserRegisteredPayload {
                            user_id: user_id.to_string(),
                            email: email.clone(),
                            email_verified: false,
                            username: username.clone(),
                            display_name: display_name.clone(),
                            avatar_url: avatar_url.clone(),
                            registered_at: now.to_rfc3339(),
                        })?),
                    };
                    outbox_event::Entity::insert(user_registered_event)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "identity registration UserRegistered outbox insert failed");
                            Status::internal("internal server error")
                        })?;

                    let email_verification_request = outbox_event::ActiveModel {
                        event_id: Set(Uuid::new_v4()),
                        aggregate_type: Set("user_account".to_string()),
                        aggregate_id: Set(user_id),
                        event_type: Set("VerificationEmailRequested".to_string()),
                        created_at: Set(now.into()),
                        available_at: Set(now.into()),
                        occurred_at: Set(now.into()),
                        claimed_at: Set(None),
                        claimed_by: Set(None),
                        published_at: Set(None),
                        last_error: Set(None),
                        publish_attempts: Set(0),
                        status: Set("pending".to_string()),
                        payload: Set(payload_value(VerificationEmailRequestedPayload {
                            user_id: user_id.to_string(),
                            email: email.clone(),
                            verification_token: verification_token.clone(),
                            verification_token_expires_at: (now + chrono::Duration::hours(6))
                                .to_rfc3339(),
                            verification_token_id: verification_token_id.to_string(),
                            reason: "registration".to_string(),
                            requested_at: now.to_rfc3339(),
                        })?),
                    };
                    outbox_event::Entity::insert(email_verification_request)
                        .exec(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "identity registration VerificationEmailRequested outbox insert failed");
                            Status::internal("internal server error")
                        })?;

                    Ok(Response::new(RegisterUserResponse {
                        user_id: user_id.to_string(),
                        email,
                        verification_email_requested_at: Some(to_timestamp(now)),
                    }))
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "identity registration transaction connection failure");
                    Status::internal("internal server error")
                }
                TransactionError::Transaction(status) => status,
            })?;

        Ok(response)
    }
}

fn random_discriminator() -> String {
    let bytes = *Uuid::new_v4().as_bytes();
    let value = u16::from_be_bytes([bytes[0], bytes[1]]) % 10_000;
    format!("{value:04}")
}

#[cfg(test)]
mod tests {
    use super::random_discriminator;

    #[test]
    fn generated_discriminator_is_four_digits() {
        let discriminator = random_discriminator();

        assert_eq!(discriminator.len(), 4);
        assert!(discriminator.chars().all(|c| c.is_ascii_digit()));
    }
}
