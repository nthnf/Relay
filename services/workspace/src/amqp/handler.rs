use chrono::Utc;
use lapin::message::Delivery;
use sea_orm::{
    ActiveModelTrait, DatabaseConnection, EntityTrait, IntoActiveModel, Set, TransactionError,
    TransactionTrait,
};
use std::error::Error;
use std::fmt::Display;
use tracing::error;
use uuid::Uuid;

use crate::{
    entity::user_snapshot,
    events::{UserEmailVerifiedPayload, UserProfileUpdatedPayload, UserRegisteredPayload},
};

#[derive(Debug)]
pub enum AmqpError {
    Permanent(String),
    Transient(String),
}

impl Display for AmqpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AmqpError::Permanent(message) | AmqpError::Transient(message) => f.write_str(message),
        }
    }
}

impl Error for AmqpError {}

enum IdentityEvent {
    UserRegistered(UserRegisteredPayload),
    UserEmailVerified(UserEmailVerifiedPayload),
    UserProfileUpdated(UserProfileUpdatedPayload),
}

#[derive(Clone)]
pub struct Handler {
    db: DatabaseConnection,
}

impl Handler {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn handle_delivery(&self, delivery: &Delivery) -> Result<(), AmqpError> {
        let event = self.parse_event(delivery)?;
        self.handle_event(event).await
    }

    fn parse_event(&self, delivery: &Delivery) -> Result<IdentityEvent, AmqpError> {
        match delivery.routing_key.as_str() {
            "identity.UserRegistered" => {
                let payload: UserRegisteredPayload = serde_json::from_slice(&delivery.data)
                    .map_err(|e| {
                        AmqpError::Permanent(format!("failed to parse user registered event: {e}"))
                    })?;
                Ok(IdentityEvent::UserRegistered(payload))
            }
            "identity.UserEmailVerified" => {
                let payload: UserEmailVerifiedPayload = serde_json::from_slice(&delivery.data)
                    .map_err(|e| {
                        AmqpError::Permanent(format!(
                            "failed to parse user email verified event: {e}"
                        ))
                    })?;
                Ok(IdentityEvent::UserEmailVerified(payload))
            }
            "identity.UserProfileUpdated" => {
                let payload: UserProfileUpdatedPayload = serde_json::from_slice(&delivery.data)
                    .map_err(|e| {
                        AmqpError::Permanent(format!(
                            "failed to parse user profile updated event: {e}"
                        ))
                    })?;
                Ok(IdentityEvent::UserProfileUpdated(payload))
            }
            other => Err(AmqpError::Permanent(format!(
                "unknown routing key: {other}"
            ))),
        }
    }

    async fn handle_event(&self, event: IdentityEvent) -> Result<(), AmqpError> {
        match event {
            IdentityEvent::UserRegistered(payload) => self.handle_user_registered(payload).await,
            IdentityEvent::UserEmailVerified(payload) => {
                self.handle_user_email_verified(payload).await
            }
            IdentityEvent::UserProfileUpdated(payload) => {
                self.handle_user_profile_updated(payload).await
            }
        }
    }

    async fn handle_user_registered(
        &self,
        payload: UserRegisteredPayload,
    ) -> Result<(), AmqpError> {
        let UserRegisteredPayload {
            user_id,
            username,
            display_name,
            avatar_url,
            ..
        } = payload;
        let user_id = Uuid::parse_str(&user_id)
            .map_err(|_| AmqpError::Permanent("Invalid UUID".to_string()))?;
        let now = Utc::now();

        self.db
            .transaction::<_, (), AmqpError>(|txn| {
                Box::pin(async move {
                    let existing = user_snapshot::Entity::find_by_id(user_id)
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "User account lookup failed");
                            AmqpError::Transient("User account lookup failed".to_string())
                        })?;

                    match existing {
                        Some(existing) => {
                            let mut active = existing.into_active_model();
                            active.username = Set(username);
                            active.display_name = Set(display_name);
                            active.avatar_url = Set(avatar_url);
                            active.updated_at = Set(now.into());
                            active.update(txn).await.map_err(|e| {
                                error!(error = %e, "User account update failed");
                                AmqpError::Transient("User account update failed".to_string())
                            })?;
                            Ok(())
                        }
                        None => {
                            let user = user_snapshot::ActiveModel {
                                user_id: Set(user_id),
                                email_verified: Set(false),
                                username: Set(username),
                                display_name: Set(display_name),
                                avatar_url: Set(avatar_url),
                                created_at: Set(now.into()),
                                updated_at: Set(now.into()),
                            };
                            user_snapshot::Entity::insert(user)
                                .exec(txn)
                                .await
                                .map_err(|e| {
                                    error!(error = %e, "User account insert failed");
                                    AmqpError::Transient("User account insert failed".to_string())
                                })?;

                            Ok(())
                        }
                    }
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "User registration connection failure");
                    AmqpError::Transient("User registration connection failure".to_string())
                }
                TransactionError::Transaction(db_err) => {
                    error!(error = %db_err, "User registration transaction failure");
                    AmqpError::Transient("User registration transaction failure".to_string())
                }
            })?;

        Ok(())
    }

    async fn handle_user_email_verified(
        &self,
        payload: UserEmailVerifiedPayload,
    ) -> Result<(), AmqpError> {
        let UserEmailVerifiedPayload { user_id, .. } = payload;
        let user_id = Uuid::parse_str(&user_id)
            .map_err(|_| AmqpError::Permanent("Invalid UUID".to_string()))?;
        let now = Utc::now();

        self.db
            .transaction::<_, (), AmqpError>(|txn| {
                Box::pin(async move {
                    let existing = user_snapshot::Entity::find_by_id(user_id)
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "User email verified lookup failed");
                            AmqpError::Transient("User email verified lookup failed".to_string())
                        })?;

                    match existing {
                        Some(existing) => {
                            if !existing.email_verified {
                                let mut active = existing.into_active_model();
                                active.email_verified = Set(true);
                                active.updated_at = Set(now.into());
                                active.update(txn).await.map_err(|e| {
                                    error!(error = %e, "User email verified update failed");
                                    AmqpError::Transient(
                                        "User email verified update failed".to_string(),
                                    )
                                })?;
                            }

                            Ok(())
                        }
                        None => {
                            let user = user_snapshot::ActiveModel {
                                user_id: Set(user_id),
                                email_verified: Set(true),
                                username: Set(String::new()),
                                display_name: Set(String::new()),
                                avatar_url: Set(None),
                                created_at: Set(now.into()),
                                updated_at: Set(now.into()),
                            };

                            user_snapshot::Entity::insert(user)
                                .exec(txn)
                                .await
                                .map_err(|e| {
                                    error!(error = %e, "User email verified insert failed");
                                    AmqpError::Transient(
                                        "User email verified insert failed".to_string(),
                                    )
                                })?;

                            Ok(())
                        }
                    }
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "User email verified connection failure");
                    AmqpError::Transient("User email verified connection failure".to_string())
                }
                TransactionError::Transaction(db_err) => {
                    error!(error = %db_err, "User email verified transaction failure");
                    AmqpError::Transient("User email verified transaction failure".to_string())
                }
            })?;

        Ok(())
    }

    async fn handle_user_profile_updated(
        &self,
        payload: UserProfileUpdatedPayload,
    ) -> Result<(), AmqpError> {
        let UserProfileUpdatedPayload {
            user_id,
            username,
            display_name,
            avatar_url,
            ..
        } = payload;
        let user_id = Uuid::parse_str(&user_id)
            .map_err(|_| AmqpError::Permanent("Invalid UUID".to_string()))?;
        let now = Utc::now();

        self.db
            .transaction::<_, (), AmqpError>(|txn| {
                Box::pin(async move {
                    let existing = user_snapshot::Entity::find_by_id(user_id)
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "User profile updated lookup failed");
                            AmqpError::Transient("User profile updated lookup failed".to_string())
                        })?;

                    match existing {
                        Some(existing) => {
                            let mut active = existing.into_active_model();
                            active.username = Set(username);
                            active.display_name = Set(display_name);
                            active.avatar_url = Set(avatar_url);
                            active.updated_at = Set(now.into());
                            active.update(txn).await.map_err(|e| {
                                error!(error = %e, "User profile updated failed");
                                AmqpError::Transient("User profile updated failed".to_string())
                            })?;
                            Ok(())
                        }
                        None => {
                            let user = user_snapshot::ActiveModel {
                                user_id: Set(user_id),
                                email_verified: Set(false),
                                username: Set(username),
                                display_name: Set(display_name),
                                avatar_url: Set(avatar_url),
                                created_at: Set(now.into()),
                                updated_at: Set(now.into()),
                            };

                            user_snapshot::Entity::insert(user)
                                .exec(txn)
                                .await
                                .map_err(|e| {
                                    error!(error = %e, "User profile updated insert failed");
                                    AmqpError::Transient(
                                        "User profile updated insert failed".to_string(),
                                    )
                                })?;

                            Ok(())
                        }
                    }
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => {
                    error!(error = %db_err, "User profile updated connection failure");
                    AmqpError::Transient("User profile updated connection failure".to_string())
                }
                TransactionError::Transaction(db_err) => {
                    error!(error = %db_err, "User profile updated transaction failure");
                    AmqpError::Transient("User profile updated transaction failure".to_string())
                }
            })?;

        Ok(())
    }
}
