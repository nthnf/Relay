use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter,
    Set,
};
use std::sync::Arc;
use tracing::error;
use uuid::Uuid;

use crate::{
    entity::{user_snapshot, workspace_channel_snapshot, workspace_snapshot},
    events::{
        UserEmailVerifiedPayload, UserProfileUpdatedPayload, UserRegisteredPayload,
        WorkspaceChannelCreatedPayload, WorkspaceCreatedPayload, WorkspaceDeletedPayload,
    },
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
        self.upsert_user_snapshot(&payload.user_id).await
    }

    pub async fn handle_user_email_verified(
        self: Arc<Self>,
        _delivery: DeliveryContext,
        payload: UserEmailVerifiedPayload,
    ) -> EventHandleResult {
        self.upsert_user_snapshot(&payload.user_id).await
    }

    pub async fn handle_user_profile_updated(
        self: Arc<Self>,
        _delivery: DeliveryContext,
        payload: UserProfileUpdatedPayload,
    ) -> EventHandleResult {
        self.upsert_user_snapshot(&payload.user_id).await
    }

    pub async fn handle_workspace_created(
        self: Arc<Self>,
        _delivery: DeliveryContext,
        payload: WorkspaceCreatedPayload,
    ) -> EventHandleResult {
        let workspace_id = parse_uuid(&payload.workspace_id, "workspace_id")?;
        let now = Utc::now();

        let existing = workspace_snapshot::Entity::find_by_id(workspace_id)
            .one(&self.db)
            .await
            .map_err(db_lookup_error("Workspace snapshot lookup failed"))?;

        match existing {
            Some(existing) => {
                let mut active = existing.into_active_model();
                active.updated_at = Set(now.into());
                active
                    .update(&self.db)
                    .await
                    .map_err(db_write_error("Workspace snapshot update failed"))?;
            }
            None => {
                workspace_snapshot::ActiveModel {
                    workspace_id: Set(workspace_id),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(&self.db)
                .await
                .map_err(db_write_error("Workspace snapshot insert failed"))?;
            }
        }

        Ok(())
    }

    pub async fn handle_workspace_channel_created(
        self: Arc<Self>,
        _delivery: DeliveryContext,
        payload: WorkspaceChannelCreatedPayload,
    ) -> EventHandleResult {
        let workspace_channel_id = parse_uuid(&payload.channel_id, "channel_id")?;
        let workspace_id = parse_uuid(&payload.workspace_id, "workspace_id")?;
        let now = Utc::now();

        let existing = workspace_channel_snapshot::Entity::find_by_id(workspace_channel_id)
            .one(&self.db)
            .await
            .map_err(db_lookup_error("Workspace channel snapshot lookup failed"))?;

        match existing {
            Some(existing) => {
                let mut active = existing.into_active_model();
                active.workspace_id = Set(workspace_id);
                active.channel_kind = Set(payload.channel_kind);
                active.updated_at = Set(now.into());
                active
                    .update(&self.db)
                    .await
                    .map_err(db_write_error("Workspace channel snapshot update failed"))?;
            }
            None => {
                workspace_channel_snapshot::ActiveModel {
                    workspace_channel_id: Set(workspace_channel_id),
                    workspace_id: Set(workspace_id),
                    channel_kind: Set(payload.channel_kind),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(&self.db)
                .await
                .map_err(db_write_error("Workspace channel snapshot insert failed"))?;
            }
        }

        Ok(())
    }

    pub async fn handle_workspace_deleted(
        self: Arc<Self>,
        _delivery: DeliveryContext,
        payload: WorkspaceDeletedPayload,
    ) -> EventHandleResult {
        let workspace_id = parse_uuid(&payload.workspace_id, "workspace_id")?;

        workspace_channel_snapshot::Entity::delete_many()
            .filter(workspace_channel_snapshot::Column::WorkspaceId.eq(workspace_id))
            .exec(&self.db)
            .await
            .map_err(db_write_error("Workspace channel snapshot delete failed"))?;
        workspace_snapshot::Entity::delete_by_id(workspace_id)
            .exec(&self.db)
            .await
            .map_err(db_write_error("Workspace snapshot delete failed"))?;

        Ok(())
    }

    async fn upsert_user_snapshot(&self, user_id: &str) -> EventHandleResult {
        let user_id = parse_uuid(user_id, "user_id")?;
        let now = Utc::now();

        let existing = user_snapshot::Entity::find_by_id(user_id)
            .one(&self.db)
            .await
            .map_err(db_lookup_error("User snapshot lookup failed"))?;

        match existing {
            Some(existing) => {
                let mut active = existing.into_active_model();
                active.updated_at = Set(now.into());
                active
                    .update(&self.db)
                    .await
                    .map_err(db_write_error("User snapshot update failed"))?;
            }
            None => {
                user_snapshot::ActiveModel {
                    user_id: Set(user_id),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(&self.db)
                .await
                .map_err(db_write_error("User snapshot insert failed"))?;
            }
        }

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
            .event(
                "workspace.WorkspaceCreated",
                route(Self::handle_workspace_created),
            )
            .event(
                "workspace.WorkspaceDeleted",
                route(Self::handle_workspace_deleted),
            )
            .event(
                "workspace.WorkspaceChannelCreated",
                route(Self::handle_workspace_channel_created),
            )
    }
}

fn parse_uuid(value: &str, field: &str) -> Result<Uuid, EventHandleError> {
    Uuid::parse_str(value).map_err(|e| EventHandleError::Permanent(format!("invalid {field}: {e}")))
}

fn db_lookup_error(message: &'static str) -> impl Fn(sea_orm::DbErr) -> EventHandleError {
    move |error| {
        error!(error = %error, "{message}");
        EventHandleError::Transient(message.to_string())
    }
}

fn db_write_error(message: &'static str) -> impl Fn(sea_orm::DbErr) -> EventHandleError {
    move |error| {
        error!(error = %error, "{message}");
        EventHandleError::Transient(message.to_string())
    }
}
