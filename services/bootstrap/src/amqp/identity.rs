use std::sync::Arc;

use crate::{
    entity::user_snapshot,
    events::{UserProfileUpdatedPayload, UserRegisteredPayload},
};
use relay_amqp::{DeliveryContext, EventHandleResult, RegisteredSubscriber, route};
use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, Set, TransactionTrait};

use super::handler::{ComposeWork, Handler};

impl Handler {
    pub async fn handle_user_registered(
        self: Arc<Self>,
        delivery: DeliveryContext,
        payload: UserRegisteredPayload,
    ) -> EventHandleResult {
        let UserRegisteredPayload {
            user_id,
            email: _,
            email_verified: _,
            username,
            display_name,
            avatar_url,
            registered_at,
        } = payload;
        let parsed_user_id = Self::parse_uuid("user_id", &user_id)?;
        let updated_at = Self::parse_timestamp("registered_at", &registered_at)?;

        self.db
            .transaction::<_, (), relay_amqp::EventHandleError>(|txn| {
                Box::pin(async move {
                    if !Self::mark_event_processed(txn, &delivery, &user_id).await? {
                        return Ok(());
                    }

                    upsert_user_snapshot(
                        txn,
                        parsed_user_id,
                        username,
                        display_name,
                        avatar_url,
                        updated_at,
                    )
                    .await?;
                    Self::enqueue_compose(txn, ComposeWork::user_app(parsed_user_id)).await?;
                    Self::enqueue_compose(txn, ComposeWork::dm(Some(parsed_user_id), None, None))
                        .await?;

                    Ok(())
                })
            })
            .await
            .map_err(|error| Self::tx_error("user registered transaction", error))?;

        Ok(())
    }

    pub async fn handle_user_profile_updated(
        self: Arc<Self>,
        delivery: DeliveryContext,
        payload: UserProfileUpdatedPayload,
    ) -> EventHandleResult {
        let UserProfileUpdatedPayload {
            user_id,
            username,
            display_name,
            avatar_url,
            updated_at,
        } = payload;
        let parsed_user_id = Self::parse_uuid("user_id", &user_id)?;
        let updated_at = Self::parse_timestamp("updated_at", &updated_at)?;

        self.db
            .transaction::<_, (), relay_amqp::EventHandleError>(|txn| {
                Box::pin(async move {
                    if !Self::mark_event_processed(txn, &delivery, &user_id).await? {
                        return Ok(());
                    }

                    upsert_user_snapshot(
                        txn,
                        parsed_user_id,
                        username,
                        display_name,
                        avatar_url,
                        updated_at,
                    )
                    .await?;
                    Self::enqueue_compose(txn, ComposeWork::user_app(parsed_user_id)).await?;
                    Self::enqueue_compose(txn, ComposeWork::dm(Some(parsed_user_id), None, None))
                        .await?;

                    Ok(())
                })
            })
            .await
            .map_err(|error| Self::tx_error("user profile transaction", error))?;

        Ok(())
    }
}

async fn upsert_user_snapshot(
    txn: &sea_orm::DatabaseTransaction,
    user_id: uuid::Uuid,
    username: String,
    display_name: String,
    avatar_url: Option<String>,
    updated_at: chrono::DateTime<chrono::FixedOffset>,
) -> EventHandleResult {
    match user_snapshot::Entity::find_by_id(user_id)
        .one(txn)
        .await
        .map_err(|error| Handler::db_error("user snapshot lookup", error))?
    {
        Some(existing) => {
            let mut active = existing.into_active_model();
            active.username = Set(username);
            active.display_name = Set(display_name);
            active.avatar_url = Set(avatar_url);
            active.updated_at = Set(updated_at);
            active
                .update(txn)
                .await
                .map_err(|error| Handler::db_error("user snapshot update", error))?;
        }
        None => {
            user_snapshot::ActiveModel {
                user_id: Set(user_id),
                username: Set(username),
                display_name: Set(display_name),
                avatar_url: Set(avatar_url),
                updated_at: Set(updated_at),
            }
            .insert(txn)
            .await
            .map_err(|error| Handler::db_error("user snapshot insert", error))?;
        }
    }

    Ok(())
}

pub(super) fn register(subscriber: RegisteredSubscriber<Handler>) -> RegisteredSubscriber<Handler> {
    subscriber
        .event(
            "identity.UserRegistered",
            route(Handler::handle_user_registered),
        )
        .event(
            "identity.UserProfileUpdated",
            route(Handler::handle_user_profile_updated),
        )
}
