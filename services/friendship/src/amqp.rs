use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, DatabaseConnection, EntityTrait, IntoActiveModel, Set, TransactionError,
    TransactionTrait,
};
use std::sync::Arc;
use tracing::error;
use uuid::Uuid;

use crate::{
    entity::user_snapshot,
    events::{UserEmailVerifiedPayload, UserProfileUpdatedPayload, UserRegisteredPayload},
};
use relay_amqp::{
    DeliveryContext, EventHandleError, EventHandleResult, RegisteredSubscriber,
    RegistersAmqpRoutes, route,
};

#[derive(Clone)]
pub struct Handler {
    db: DatabaseConnection,
}

pub use Handler as AmqpHandler;

impl Handler {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn handle_user_registered(
        self: Arc<Self>,
        _delivery: DeliveryContext,
        payload: UserRegisteredPayload,
    ) -> EventHandleResult {
        let UserRegisteredPayload {
            user_id,
            username,
            display_name,
            avatar_url,
            ..
        } = payload;
        let user_id = Uuid::parse_str(&user_id)
            .map_err(|_| EventHandleError::Permanent("Invalid UUID".to_string()))?;
        let now = Utc::now();

        self.db
            .transaction::<_, (), EventHandleError>(|txn| {
                Box::pin(async move {
                    let existing = user_snapshot::Entity::find_by_id(user_id)
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "User account lookup failed");
                            EventHandleError::Transient("User account lookup failed".to_string())
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
                                EventHandleError::Transient(
                                    "User account update failed".to_string(),
                                )
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
                                    EventHandleError::Transient(
                                        "User account insert failed".to_string(),
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
                    error!(error = %db_err, "User registration connection failure");
                    EventHandleError::Transient("User registration connection failure".to_string())
                }
                TransactionError::Transaction(db_err) => {
                    error!(error = %db_err, "User registration transaction failure");
                    EventHandleError::Transient("User registration transaction failure".to_string())
                }
            })?;

        Ok(())
    }

    pub async fn handle_user_email_verified(
        self: Arc<Self>,
        _delivery: DeliveryContext,
        payload: UserEmailVerifiedPayload,
    ) -> EventHandleResult {
        let UserEmailVerifiedPayload { user_id, .. } = payload;
        let user_id = Uuid::parse_str(&user_id)
            .map_err(|_| EventHandleError::Permanent("Invalid UUID".to_string()))?;
        let now = Utc::now();

        self.db
            .transaction::<_, (), EventHandleError>(|txn| {
                Box::pin(async move {
                    let existing = user_snapshot::Entity::find_by_id(user_id)
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "User email verified lookup failed");
                            EventHandleError::Transient(
                                "User email verified lookup failed".to_string(),
                            )
                        })?;

                    match existing {
                        Some(existing) => {
                            if !existing.email_verified {
                                let mut active = existing.into_active_model();
                                active.email_verified = Set(true);
                                active.updated_at = Set(now.into());
                                active.update(txn).await.map_err(|e| {
                                    error!(error = %e, "User email verified update failed");
                                    EventHandleError::Transient(
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
                                    EventHandleError::Transient(
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
                    EventHandleError::Transient(
                        "User email verified connection failure".to_string(),
                    )
                }
                TransactionError::Transaction(db_err) => {
                    error!(error = %db_err, "User email verified transaction failure");
                    EventHandleError::Transient(
                        "User email verified transaction failure".to_string(),
                    )
                }
            })?;

        Ok(())
    }

    pub async fn handle_user_profile_updated(
        self: Arc<Self>,
        _delivery: DeliveryContext,
        payload: UserProfileUpdatedPayload,
    ) -> EventHandleResult {
        let UserProfileUpdatedPayload {
            user_id,
            username,
            display_name,
            avatar_url,
            ..
        } = payload;
        let user_id = Uuid::parse_str(&user_id)
            .map_err(|_| EventHandleError::Permanent("Invalid UUID".to_string()))?;
        let now = Utc::now();

        self.db
            .transaction::<_, (), EventHandleError>(|txn| {
                Box::pin(async move {
                    let existing = user_snapshot::Entity::find_by_id(user_id)
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!(error = %e, "User profile updated lookup failed");
                            EventHandleError::Transient(
                                "User profile updated lookup failed".to_string(),
                            )
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
                                EventHandleError::Transient(
                                    "User profile updated failed".to_string(),
                                )
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
                                    EventHandleError::Transient(
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
                    EventHandleError::Transient(
                        "User profile updated connection failure".to_string(),
                    )
                }
                TransactionError::Transaction(db_err) => {
                    error!(error = %db_err, "User profile updated transaction failure");
                    EventHandleError::Transient(
                        "User profile updated transaction failure".to_string(),
                    )
                }
            })?;

        Ok(())
    }
}

impl RegistersAmqpRoutes for Handler {
    fn register(subscriber: RegisteredSubscriber<Self>) -> RegisteredSubscriber<Self> {
        subscriber
            .event(
                "identity.UserRegistered",
                route(Self::handle_user_registered),
            )
            .event(
                "identity.UserEmailVerified",
                route(Self::handle_user_email_verified),
            )
            .event(
                "identity.UserProfileUpdated",
                route(Self::handle_user_profile_updated),
            )
    }
}
