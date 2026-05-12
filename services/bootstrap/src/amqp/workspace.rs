use std::sync::Arc;

use crate::{
    entity::{workspace_channel_snapshot, workspace_member_snapshot, workspace_snapshot},
    events::{
        WorkspaceChannelCreatedPayload, WorkspaceCreatedPayload, WorkspaceDeletedPayload,
        WorkspaceMemberAddedPayload, WorkspaceMemberRemovedPayload, WorkspaceUpdatedPayload,
    },
};
use relay_amqp::{DeliveryContext, EventHandleResult, RegisteredSubscriber, route};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set, TransactionTrait,
};

use super::handler::{ComposeWork, Handler};

impl Handler {
    pub async fn handle_workspace_created(
        self: Arc<Self>,
        delivery: DeliveryContext,
        payload: WorkspaceCreatedPayload,
    ) -> EventHandleResult {
        let WorkspaceCreatedPayload {
            workspace_id,
            name,
            owner_user_id,
            created_at,
            initial_member_user_id,
        } = payload;
        let parsed_workspace_id = Self::parse_uuid("workspace_id", &workspace_id)?;
        let owner_user_id = Self::parse_uuid("owner_user_id", &owner_user_id)?;
        let initial_member_user_id =
            Self::parse_uuid("initial_member_user_id", &initial_member_user_id)?;
        let created_at = Self::parse_timestamp("created_at", &created_at)?;

        self.db
            .transaction::<_, (), relay_amqp::EventHandleError>(|txn| {
                Box::pin(async move {
                    if !Self::mark_event_processed(txn, &delivery, &workspace_id).await? {
                        return Ok(());
                    }

                    match workspace_snapshot::Entity::find_by_id(parsed_workspace_id)
                        .one(txn)
                        .await
                        .map_err(|error| Self::db_error("workspace snapshot lookup", error))?
                    {
                        Some(existing) => {
                            let mut active = existing.into_active_model();
                            active.name = Set(name);
                            active.icon_url = Set(None);
                            active.owner_user_id = Set(owner_user_id);
                            active.created_at = Set(created_at);
                            active.updated_at = Set(created_at);
                            active.update(txn).await.map_err(|error| {
                                Self::db_error("workspace snapshot update", error)
                            })?;
                        }
                        None => {
                            workspace_snapshot::ActiveModel {
                                workspace_id: Set(parsed_workspace_id),
                                name: Set(name),
                                icon_url: Set(None),
                                owner_user_id: Set(owner_user_id),
                                created_at: Set(created_at),
                                updated_at: Set(created_at),
                            }
                            .insert(txn)
                            .await
                            .map_err(|error| Self::db_error("workspace snapshot insert", error))?;
                        }
                    }

                    Self::enqueue_compose(
                        txn,
                        ComposeWork::workspace(Some(initial_member_user_id), parsed_workspace_id),
                    )
                    .await?;
                    Self::enqueue_compose(txn, ComposeWork::workspace(None, parsed_workspace_id))
                        .await?;
                    Self::enqueue_compose(
                        txn,
                        ComposeWork::workspace_channel(None, Some(parsed_workspace_id), None, None),
                    )
                    .await?;

                    Ok(())
                })
            })
            .await
            .map_err(|error| Self::tx_error("workspace created transaction", error))?;

        Ok(())
    }

    pub async fn handle_workspace_member_added(
        self: Arc<Self>,
        delivery: DeliveryContext,
        payload: WorkspaceMemberAddedPayload,
    ) -> EventHandleResult {
        let WorkspaceMemberAddedPayload {
            workspace_id,
            user_id,
            joined_at,
            added_by_user_id: _,
            source: _,
        } = payload;
        let parsed_workspace_id = Self::parse_uuid("workspace_id", &workspace_id)?;
        let parsed_user_id = Self::parse_uuid("user_id", &user_id)?;
        let joined_at = Self::parse_timestamp("joined_at", &joined_at)?;

        self.db
            .transaction::<_, (), relay_amqp::EventHandleError>(|txn| {
                Box::pin(async move {
                    let source_id = format!("{}:{}", workspace_id, user_id);
                    if !Self::mark_event_processed(txn, &delivery, &source_id).await? {
                        return Ok(());
                    }

                    match workspace_member_snapshot::Entity::find_by_id((
                        parsed_workspace_id,
                        parsed_user_id,
                    ))
                    .one(txn)
                    .await
                    .map_err(|error| Self::db_error("workspace member snapshot lookup", error))?
                    {
                        Some(existing) => {
                            let mut active = existing.into_active_model();
                            active.status = Set("active".to_string());
                            active.joined_at = Set(Some(joined_at));
                            active.removed_at = Set(None);
                            active.updated_at = Set(joined_at);
                            active.update(txn).await.map_err(|error| {
                                Self::db_error("workspace member snapshot update", error)
                            })?;
                        }
                        None => {
                            workspace_member_snapshot::ActiveModel {
                                workspace_id: Set(parsed_workspace_id),
                                user_id: Set(parsed_user_id),
                                status: Set("active".to_string()),
                                joined_at: Set(Some(joined_at)),
                                removed_at: Set(None),
                                updated_at: Set(joined_at),
                            }
                            .insert(txn)
                            .await
                            .map_err(|error| {
                                Self::db_error("workspace member snapshot insert", error)
                            })?;
                        }
                    }

                    Self::enqueue_compose(
                        txn,
                        ComposeWork::workspace(Some(parsed_user_id), parsed_workspace_id),
                    )
                    .await?;
                    Self::enqueue_compose(txn, ComposeWork::workspace(None, parsed_workspace_id))
                        .await?;
                    Self::enqueue_compose(
                        txn,
                        ComposeWork::workspace_channel(
                            Some(parsed_user_id),
                            Some(parsed_workspace_id),
                            None,
                            None,
                        ),
                    )
                    .await?;

                    Ok(())
                })
            })
            .await
            .map_err(|error| Self::tx_error("workspace member added transaction", error))?;

        Ok(())
    }

    pub async fn handle_workspace_updated(
        self: Arc<Self>,
        delivery: DeliveryContext,
        payload: WorkspaceUpdatedPayload,
    ) -> EventHandleResult {
        let WorkspaceUpdatedPayload {
            workspace_id,
            name,
            icon_url,
            updated_by_user_id: _,
            updated_at,
        } = payload;
        let parsed_workspace_id = Self::parse_uuid("workspace_id", &workspace_id)?;
        let updated_at = Self::parse_timestamp("updated_at", &updated_at)?;

        self.db
            .transaction::<_, (), relay_amqp::EventHandleError>(|txn| {
                Box::pin(async move {
                    if !Self::mark_event_processed(txn, &delivery, &workspace_id).await? {
                        return Ok(());
                    }

                    match workspace_snapshot::Entity::find_by_id(parsed_workspace_id)
                        .one(txn)
                        .await
                        .map_err(|error| Self::db_error("workspace snapshot lookup", error))?
                    {
                        Some(existing) => {
                            let mut active = existing.into_active_model();
                            active.name = Set(name);
                            active.icon_url = Set(icon_url);
                            active.updated_at = Set(updated_at);
                            active.update(txn).await.map_err(|error| {
                                Self::db_error("workspace snapshot update", error)
                            })?;
                        }
                        None => return Ok(()),
                    }

                    Self::enqueue_compose(txn, ComposeWork::workspace(None, parsed_workspace_id))
                        .await?;

                    Ok(())
                })
            })
            .await
            .map_err(|error| Self::tx_error("workspace updated transaction", error))?;

        Ok(())
    }

    pub async fn handle_workspace_deleted(
        self: Arc<Self>,
        delivery: DeliveryContext,
        payload: WorkspaceDeletedPayload,
    ) -> EventHandleResult {
        let WorkspaceDeletedPayload {
            workspace_id,
            deleted_by_user_id: _,
            deleted_at: _,
        } = payload;
        let parsed_workspace_id = Self::parse_uuid("workspace_id", &workspace_id)?;

        self.db
            .transaction::<_, (), relay_amqp::EventHandleError>(|txn| {
                Box::pin(async move {
                    if !Self::mark_event_processed(txn, &delivery, &workspace_id).await? {
                        return Ok(());
                    }

                    workspace_channel_snapshot::Entity::delete_many()
                        .filter(
                            workspace_channel_snapshot::Column::WorkspaceId.eq(parsed_workspace_id),
                        )
                        .exec(txn)
                        .await
                        .map_err(|error| {
                            Self::db_error("workspace channel snapshot delete", error)
                        })?;
                    workspace_snapshot::Entity::delete_by_id(parsed_workspace_id)
                        .exec(txn)
                        .await
                        .map_err(|error| Self::db_error("workspace snapshot delete", error))?;

                    Self::enqueue_compose(txn, ComposeWork::workspace(None, parsed_workspace_id))
                        .await?;
                    Self::enqueue_compose(
                        txn,
                        ComposeWork::workspace_channel(None, Some(parsed_workspace_id), None, None),
                    )
                    .await?;

                    Ok(())
                })
            })
            .await
            .map_err(|error| Self::tx_error("workspace deleted transaction", error))?;

        Ok(())
    }

    pub async fn handle_workspace_member_removed(
        self: Arc<Self>,
        delivery: DeliveryContext,
        payload: WorkspaceMemberRemovedPayload,
    ) -> EventHandleResult {
        let WorkspaceMemberRemovedPayload {
            workspace_id,
            user_id,
            removed_at,
            removed_by_user_id: _,
            reason: _,
        } = payload;
        let parsed_workspace_id = Self::parse_uuid("workspace_id", &workspace_id)?;
        let parsed_user_id = Self::parse_uuid("user_id", &user_id)?;
        let removed_at = Self::parse_timestamp("removed_at", &removed_at)?;

        self.db
            .transaction::<_, (), relay_amqp::EventHandleError>(|txn| {
                Box::pin(async move {
                    let source_id = format!("{}:{}", workspace_id, user_id);
                    if !Self::mark_event_processed(txn, &delivery, &source_id).await? {
                        return Ok(());
                    }

                    match workspace_member_snapshot::Entity::find_by_id((
                        parsed_workspace_id,
                        parsed_user_id,
                    ))
                    .one(txn)
                    .await
                    .map_err(|error| Self::db_error("workspace member snapshot lookup", error))?
                    {
                        Some(existing) => {
                            let mut active = existing.into_active_model();
                            active.status = Set("removed".to_string());
                            active.removed_at = Set(Some(removed_at));
                            active.updated_at = Set(removed_at);
                            active.update(txn).await.map_err(|error| {
                                Self::db_error("workspace member snapshot update", error)
                            })?;
                        }
                        None => {
                            workspace_member_snapshot::ActiveModel {
                                workspace_id: Set(parsed_workspace_id),
                                user_id: Set(parsed_user_id),
                                status: Set("removed".to_string()),
                                joined_at: Set(None),
                                removed_at: Set(Some(removed_at)),
                                updated_at: Set(removed_at),
                            }
                            .insert(txn)
                            .await
                            .map_err(|error| {
                                Self::db_error("workspace member snapshot insert", error)
                            })?;
                        }
                    }

                    Self::enqueue_compose(
                        txn,
                        ComposeWork::workspace(Some(parsed_user_id), parsed_workspace_id),
                    )
                    .await?;
                    Self::enqueue_compose(txn, ComposeWork::workspace(None, parsed_workspace_id))
                        .await?;
                    Self::enqueue_compose(
                        txn,
                        ComposeWork::workspace_channel(
                            Some(parsed_user_id),
                            Some(parsed_workspace_id),
                            None,
                            None,
                        ),
                    )
                    .await?;

                    Ok(())
                })
            })
            .await
            .map_err(|error| Self::tx_error("workspace member removed transaction", error))?;

        Ok(())
    }

    pub async fn handle_workspace_channel_created(
        self: Arc<Self>,
        delivery: DeliveryContext,
        payload: WorkspaceChannelCreatedPayload,
    ) -> EventHandleResult {
        let WorkspaceChannelCreatedPayload {
            channel_id,
            workspace_id,
            name,
            channel_kind,
            position,
            created_by_user_id: _,
            created_at,
        } = payload;
        let parsed_channel_id = Self::parse_uuid("channel_id", &channel_id)?;
        let parsed_workspace_id = Self::parse_uuid("workspace_id", &workspace_id)?;
        let created_at = Self::parse_timestamp("created_at", &created_at)?;

        self.db
            .transaction::<_, (), relay_amqp::EventHandleError>(|txn| {
                Box::pin(async move {
                    if !Self::mark_event_processed(txn, &delivery, &channel_id).await? {
                        return Ok(());
                    }

                    match workspace_channel_snapshot::Entity::find_by_id(parsed_channel_id)
                        .one(txn)
                        .await
                        .map_err(|error| {
                            Self::db_error("workspace channel snapshot lookup", error)
                        })? {
                        Some(existing) => {
                            let mut active = existing.into_active_model();
                            active.workspace_id = Set(parsed_workspace_id);
                            active.name = Set(name);
                            active.channel_kind = Set(channel_kind);
                            active.position = Set(position);
                            active.created_at = Set(created_at);
                            active.updated_at = Set(created_at);
                            active.update(txn).await.map_err(|error| {
                                Self::db_error("workspace channel snapshot update", error)
                            })?;
                        }
                        None => {
                            workspace_channel_snapshot::ActiveModel {
                                channel_id: Set(parsed_channel_id),
                                workspace_id: Set(parsed_workspace_id),
                                name: Set(name),
                                channel_kind: Set(channel_kind),
                                position: Set(position),
                                created_at: Set(created_at),
                                updated_at: Set(created_at),
                            }
                            .insert(txn)
                            .await
                            .map_err(|error| {
                                Self::db_error("workspace channel snapshot insert", error)
                            })?;
                        }
                    }

                    Self::enqueue_compose(
                        txn,
                        ComposeWork::workspace_channel(
                            None,
                            Some(parsed_workspace_id),
                            Some(parsed_channel_id),
                            None,
                        ),
                    )
                    .await?;

                    Ok(())
                })
            })
            .await
            .map_err(|error| Self::tx_error("workspace channel created transaction", error))?;

        Ok(())
    }
}

pub(super) fn register(subscriber: RegisteredSubscriber<Handler>) -> RegisteredSubscriber<Handler> {
    subscriber
        .event(
            "workspace.WorkspaceCreated",
            route(Handler::handle_workspace_created),
        )
        .event(
            "workspace.WorkspaceUpdated",
            route(Handler::handle_workspace_updated),
        )
        .event(
            "workspace.WorkspaceDeleted",
            route(Handler::handle_workspace_deleted),
        )
        .event(
            "workspace.WorkspaceMemberAdded",
            route(Handler::handle_workspace_member_added),
        )
        .event(
            "workspace.WorkspaceMemberRemoved",
            route(Handler::handle_workspace_member_removed),
        )
        .event(
            "workspace.WorkspaceChannelCreated",
            route(Handler::handle_workspace_channel_created),
        )
}
