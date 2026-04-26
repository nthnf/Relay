use std::sync::Arc;

use crate::{
    entity::friend_request_snapshot,
    events::{
        FriendRequestAcceptedPayload, FriendRequestCanceledByBlockPayload,
        FriendRequestCreatedPayload, FriendRequestRejectedPayload,
    },
};
use relay_amqp::{DeliveryContext, EventHandleResult, RegisteredSubscriber, route};
use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, Set, TransactionTrait};

use super::handler::{ComposeWork, Handler};

impl Handler {
    pub async fn handle_friend_request_created(
        self: Arc<Self>,
        delivery: DeliveryContext,
        payload: FriendRequestCreatedPayload,
    ) -> EventHandleResult {
        let FriendRequestCreatedPayload {
            friend_request_id,
            requester_user_id,
            addressee_user_id,
            status: _,
            created_at,
        } = payload;
        self.upsert_friend_request(
            delivery,
            friend_request_id,
            requester_user_id,
            addressee_user_id,
            "pending",
            created_at,
        )
        .await
    }

    pub async fn handle_friend_request_accepted(
        self: Arc<Self>,
        delivery: DeliveryContext,
        payload: FriendRequestAcceptedPayload,
    ) -> EventHandleResult {
        let FriendRequestAcceptedPayload {
            friend_request_id,
            requester_user_id,
            addressee_user_id,
            accepted_at,
        } = payload;
        self.upsert_friend_request(
            delivery,
            friend_request_id,
            requester_user_id,
            addressee_user_id,
            "accepted",
            accepted_at,
        )
        .await
    }

    pub async fn handle_friend_request_rejected(
        self: Arc<Self>,
        delivery: DeliveryContext,
        payload: FriendRequestRejectedPayload,
    ) -> EventHandleResult {
        let FriendRequestRejectedPayload {
            friend_request_id,
            requester_user_id,
            addressee_user_id,
            rejected_at,
        } = payload;
        self.upsert_friend_request(
            delivery,
            friend_request_id,
            requester_user_id,
            addressee_user_id,
            "rejected",
            rejected_at,
        )
        .await
    }

    pub async fn handle_friend_request_canceled_by_block(
        self: Arc<Self>,
        delivery: DeliveryContext,
        payload: FriendRequestCanceledByBlockPayload,
    ) -> EventHandleResult {
        let FriendRequestCanceledByBlockPayload {
            friend_request_id,
            requester_user_id,
            addressee_user_id,
            blocked_by_user_id: _,
            canceled_at,
            status: _,
        } = payload;
        self.upsert_friend_request(
            delivery,
            friend_request_id,
            requester_user_id,
            addressee_user_id,
            "canceled_by_block",
            canceled_at,
        )
        .await
    }

    async fn upsert_friend_request(
        &self,
        delivery: DeliveryContext,
        friend_request_id: String,
        requester_user_id: String,
        addressee_user_id: String,
        status: &'static str,
        updated_at: String,
    ) -> EventHandleResult {
        let friend_request_id = Self::parse_uuid("friend_request_id", &friend_request_id)?;
        let requester_user_id = Self::parse_uuid("requester_user_id", &requester_user_id)?;
        let addressee_user_id = Self::parse_uuid("addressee_user_id", &addressee_user_id)?;
        let updated_at = Self::parse_timestamp("updated_at", &updated_at)?;

        self.db
            .transaction::<_, (), relay_amqp::EventHandleError>(|txn| {
                Box::pin(async move {
                    if !Self::mark_event_processed(txn, &delivery, &friend_request_id.to_string())
                        .await?
                    {
                        return Ok(());
                    }

                    match friend_request_snapshot::Entity::find_by_id(friend_request_id)
                        .one(txn)
                        .await
                        .map_err(|error| Self::db_error("friend request snapshot lookup", error))?
                    {
                        Some(existing) => {
                            let mut active = existing.into_active_model();
                            active.requester_user_id = Set(requester_user_id);
                            active.addressee_user_id = Set(addressee_user_id);
                            active.status = Set(status.to_string());
                            active.updated_at = Set(updated_at);
                            active.update(txn).await.map_err(|error| {
                                Self::db_error("friend request snapshot update", error)
                            })?;
                        }
                        None => {
                            friend_request_snapshot::ActiveModel {
                                friend_request_id: Set(friend_request_id),
                                requester_user_id: Set(requester_user_id),
                                addressee_user_id: Set(addressee_user_id),
                                status: Set(status.to_string()),
                                updated_at: Set(updated_at),
                            }
                            .insert(txn)
                            .await
                            .map_err(|error| {
                                Self::db_error("friend request snapshot insert", error)
                            })?;
                        }
                    }

                    Self::enqueue_compose(txn, ComposeWork::user_app(addressee_user_id)).await?;

                    Ok(())
                })
            })
            .await
            .map_err(|error| Self::tx_error("friend request transaction", error))?;

        Ok(())
    }
}

pub(super) fn register(subscriber: RegisteredSubscriber<Handler>) -> RegisteredSubscriber<Handler> {
    subscriber
        .event(
            "friendship.FriendRequestCreated",
            route(Handler::handle_friend_request_created),
        )
        .event(
            "friendship.FriendRequestAccepted",
            route(Handler::handle_friend_request_accepted),
        )
        .event(
            "friendship.FriendRequestRejected",
            route(Handler::handle_friend_request_rejected),
        )
        .event(
            "friendship.FriendRequestCanceledByBlock",
            route(Handler::handle_friend_request_canceled_by_block),
        )
}
