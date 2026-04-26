use std::{error::Error, time::Duration};

use chrono::{DateTime, FixedOffset, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, IntoActiveModel,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use tokio::time::sleep;
use tracing::{error, warn};
use uuid::Uuid;

use crate::entity::{
    compose_queue, conversation_message_state, conversation_read_state, conversation_snapshot,
    dm_pair_snapshot, dm_projection, dm_unread_projection, friend_request_snapshot,
    user_app_projection, user_snapshot, workspace_channel_projection, workspace_channel_snapshot,
    workspace_channel_unread_projection, workspace_member_snapshot, workspace_projection,
    workspace_snapshot, workspace_unread_projection,
};

#[derive(Clone)]
pub struct Compositor {
    db: DatabaseConnection,
    poll_interval: Duration,
    batch_size: u64,
}

impl Compositor {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            poll_interval: Duration::from_millis(250),
            batch_size: 50,
        }
    }

    pub async fn run(self) -> Result<(), Box<dyn Error + Send + Sync>> {
        loop {
            if let Err(error) = self.run_once().await {
                error!(error = %error, "bootstrap compositor batch failed");
            }

            sleep(self.poll_interval).await;
        }
    }

    pub async fn run_once(&self) -> Result<(), sea_orm::DbErr> {
        let work = compose_queue::Entity::find()
            .filter(
                Condition::any()
                    .add(compose_queue::Column::Status.eq("claimed"))
                    .add(compose_queue::Column::Status.eq("pending")),
            )
            .order_by_asc(compose_queue::Column::AvailableAt)
            .limit(self.batch_size)
            .all(&self.db)
            .await?;

        for item in work {
            if let Err(error) = self.compose_one(&item).await {
                warn!(compose_key = %item.compose_key, error = %error, "composition failed");
                self.mark_failed(item, &error.to_string()).await?;
            } else {
                compose_queue::Entity::delete_by_id(item.compose_key)
                    .exec(&self.db)
                    .await?;
            }
        }

        Ok(())
    }

    async fn compose_one(&self, item: &compose_queue::Model) -> Result<(), sea_orm::DbErr> {
        match item.compose_kind.as_str() {
            "user_app" => {
                if let Some(user_id) = item.user_id {
                    self.compose_user_app(user_id).await?;
                }
            }
            "workspace" => {
                if let Some(workspace_id) = item.workspace_id {
                    self.compose_workspace(item.user_id, workspace_id).await?;
                }
            }
            "workspace_channel" => {
                self.compose_workspace_channel(
                    item.user_id,
                    item.workspace_id,
                    item.channel_id,
                    item.conversation_id,
                )
                .await?;
            }
            "workspace_unread" => {
                self.compose_workspace_unread(
                    item.user_id,
                    item.workspace_id,
                    item.conversation_id,
                )
                .await?;
            }
            "dm" => {
                self.compose_dm(item.user_id, item.dm_pair_id, item.conversation_id)
                    .await?;
            }
            _ => {}
        }

        Ok(())
    }

    async fn mark_failed(
        &self,
        item: compose_queue::Model,
        error: &str,
    ) -> Result<(), sea_orm::DbErr> {
        let attempts = item.attempts.saturating_add(1);
        let mut active = item.into_active_model();
        active.status = Set("failed".to_string());
        active.attempts = Set(attempts);
        active.claimed_at = Set(None);
        active.last_error = Set(Some(error.to_string()));
        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await?;

        Ok(())
    }

    async fn compose_user_app(&self, user_id: Uuid) -> Result<(), sea_orm::DbErr> {
        let Some(user) = user_snapshot::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
        else {
            user_app_projection::Entity::delete_by_id(user_id)
                .exec(&self.db)
                .await?;
            return Ok(());
        };

        let pending_count = friend_request_snapshot::Entity::find()
            .filter(friend_request_snapshot::Column::AddresseeUserId.eq(user_id))
            .filter(friend_request_snapshot::Column::Status.eq("pending"))
            .count(&self.db)
            .await?
            .min(i32::MAX as u64) as i32;

        match user_app_projection::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
        {
            Some(existing) => {
                let mut active = existing.into_active_model();
                active.username = Set(user.username);
                active.display_name = Set(user.display_name);
                active.avatar_url = Set(user.avatar_url);
                active.pending_friend_request_count = Set(pending_count);
                active.updated_at = Set(user.updated_at);
                active.update(&self.db).await?;
            }
            None => {
                user_app_projection::ActiveModel {
                    user_id: Set(user_id),
                    username: Set(user.username),
                    display_name: Set(user.display_name),
                    avatar_url: Set(user.avatar_url),
                    pending_friend_request_count: Set(pending_count),
                    updated_at: Set(user.updated_at),
                }
                .insert(&self.db)
                .await?;
            }
        }

        Ok(())
    }

    async fn compose_workspace(
        &self,
        user_id: Option<Uuid>,
        workspace_id: Uuid,
    ) -> Result<(), sea_orm::DbErr> {
        let members = if let Some(user_id) = user_id {
            workspace_member_snapshot::Entity::find_by_id((workspace_id, user_id))
                .one(&self.db)
                .await?
                .into_iter()
                .collect()
        } else {
            workspace_member_snapshot::Entity::find()
                .filter(workspace_member_snapshot::Column::WorkspaceId.eq(workspace_id))
                .all(&self.db)
                .await?
        };

        for member in members {
            self.compose_workspace_for_member(member.user_id, workspace_id)
                .await?;
        }

        Ok(())
    }

    async fn compose_workspace_for_member(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<(), sea_orm::DbErr> {
        let workspace = workspace_snapshot::Entity::find_by_id(workspace_id)
            .one(&self.db)
            .await?;
        let member = workspace_member_snapshot::Entity::find_by_id((workspace_id, user_id))
            .one(&self.db)
            .await?;

        let should_delete = workspace.is_none()
            || member
                .as_ref()
                .is_none_or(|member| member.status != "active");
        if should_delete {
            workspace_channel_unread_projection::Entity::delete_many()
                .filter(workspace_channel_unread_projection::Column::UserId.eq(user_id))
                .filter(workspace_channel_unread_projection::Column::WorkspaceId.eq(workspace_id))
                .exec(&self.db)
                .await?;
            workspace_unread_projection::Entity::delete_by_id((user_id, workspace_id))
                .exec(&self.db)
                .await?;
            workspace_channel_projection::Entity::delete_many()
                .filter(workspace_channel_projection::Column::UserId.eq(user_id))
                .filter(workspace_channel_projection::Column::WorkspaceId.eq(workspace_id))
                .exec(&self.db)
                .await?;
            workspace_projection::Entity::delete_by_id((user_id, workspace_id))
                .exec(&self.db)
                .await?;
            return Ok(());
        }

        let Some(workspace) = workspace else {
            return Ok(());
        };
        let member_count = workspace_member_snapshot::Entity::find()
            .filter(workspace_member_snapshot::Column::WorkspaceId.eq(workspace_id))
            .filter(workspace_member_snapshot::Column::Status.eq("active"))
            .count(&self.db)
            .await?
            .min(i32::MAX as u64) as i32;
        let unread_count = workspace_unread_projection::Entity::find_by_id((user_id, workspace_id))
            .one(&self.db)
            .await?
            .map_or(0, |row| row.unread_count);
        let updated_at = Utc::now().into();

        match workspace_projection::Entity::find_by_id((user_id, workspace_id))
            .one(&self.db)
            .await?
        {
            Some(existing) => {
                let mut active = existing.into_active_model();
                active.workspace_name = Set(workspace.name);
                active.workspace_icon_url = Set(workspace.icon_url);
                active.member_count = Set(member_count);
                active.unread_count = Set(unread_count);
                active.updated_at = Set(updated_at);
                active.update(&self.db).await?;
            }
            None => {
                workspace_projection::ActiveModel {
                    user_id: Set(user_id),
                    workspace_id: Set(workspace_id),
                    workspace_name: Set(workspace.name),
                    workspace_icon_url: Set(workspace.icon_url),
                    member_count: Set(member_count),
                    unread_count: Set(unread_count),
                    updated_at: Set(updated_at),
                }
                .insert(&self.db)
                .await?;
            }
        }

        Ok(())
    }

    async fn compose_workspace_channel(
        &self,
        user_id: Option<Uuid>,
        workspace_id: Option<Uuid>,
        channel_id: Option<Uuid>,
        conversation_id: Option<Uuid>,
    ) -> Result<(), sea_orm::DbErr> {
        let channel_id = match channel_id {
            Some(channel_id) => Some(channel_id),
            None => match conversation_id {
                Some(conversation_id) => conversation_snapshot::Entity::find_by_id(conversation_id)
                    .one(&self.db)
                    .await?
                    .and_then(|conversation| conversation.workspace_channel_id),
                None => None,
            },
        };

        match (user_id, workspace_id, channel_id) {
            (Some(user_id), Some(workspace_id), Some(channel_id)) => {
                self.compose_channel_for_member(user_id, workspace_id, channel_id)
                    .await?;
            }
            (Some(user_id), Some(workspace_id), None) => {
                let channels = workspace_channel_snapshot::Entity::find()
                    .filter(workspace_channel_snapshot::Column::WorkspaceId.eq(workspace_id))
                    .all(&self.db)
                    .await?;
                for channel in channels {
                    self.compose_channel_for_member(user_id, workspace_id, channel.channel_id)
                        .await?;
                }
            }
            (_, Some(workspace_id), Some(channel_id)) => {
                let members = workspace_member_snapshot::Entity::find()
                    .filter(workspace_member_snapshot::Column::WorkspaceId.eq(workspace_id))
                    .all(&self.db)
                    .await?;
                for member in members {
                    self.compose_channel_for_member(member.user_id, workspace_id, channel_id)
                        .await?;
                }
            }
            (_, _, Some(channel_id)) => {
                if let Some(channel) = workspace_channel_snapshot::Entity::find_by_id(channel_id)
                    .one(&self.db)
                    .await?
                {
                    let members = workspace_member_snapshot::Entity::find()
                        .filter(
                            workspace_member_snapshot::Column::WorkspaceId.eq(channel.workspace_id),
                        )
                        .all(&self.db)
                        .await?;
                    for member in members {
                        self.compose_channel_for_member(
                            member.user_id,
                            channel.workspace_id,
                            channel_id,
                        )
                        .await?;
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    async fn compose_channel_for_member(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        channel_id: Uuid,
    ) -> Result<(), sea_orm::DbErr> {
        let member = workspace_member_snapshot::Entity::find_by_id((workspace_id, user_id))
            .one(&self.db)
            .await?;
        let channel = workspace_channel_snapshot::Entity::find_by_id(channel_id)
            .one(&self.db)
            .await?;

        let should_delete = member
            .as_ref()
            .is_none_or(|member| member.status != "active")
            || channel.is_none();
        if should_delete {
            workspace_channel_unread_projection::Entity::delete_by_id((user_id, channel_id))
                .exec(&self.db)
                .await?;
            workspace_channel_projection::Entity::delete_by_id((user_id, workspace_id, channel_id))
                .exec(&self.db)
                .await?;
            return Ok(());
        }

        let Some(channel) = channel else {
            return Ok(());
        };
        let conversation = conversation_snapshot::Entity::find()
            .filter(conversation_snapshot::Column::WorkspaceChannelId.eq(channel_id))
            .one(&self.db)
            .await?;
        let conversation_id = conversation.as_ref().map(|row| row.conversation_id);
        let unread = match conversation_id {
            Some(conversation_id) => {
                self.compose_workspace_channel_unread(
                    user_id,
                    workspace_id,
                    channel_id,
                    conversation_id,
                )
                .await?
            }
            None => None,
        };
        let updated_at = Utc::now().into();

        match workspace_channel_projection::Entity::find_by_id((user_id, workspace_id, channel_id))
            .one(&self.db)
            .await?
        {
            Some(existing) => {
                let mut active = existing.into_active_model();
                active.conversation_id = Set(conversation_id);
                active.channel_name = Set(channel.name);
                active.channel_kind = Set(channel.channel_kind);
                active.position = Set(channel.position);
                active.last_message_seq = Set(unread.as_ref().and_then(|row| row.last_message_seq));
                active.last_read_conversation_message_seq = Set(unread
                    .as_ref()
                    .and_then(|row| row.last_read_conversation_message_seq));
                active.unread_count = Set(unread.map_or(0, |row| row.unread_count));
                active.updated_at = Set(updated_at);
                active.update(&self.db).await?;
            }
            None => {
                workspace_channel_projection::ActiveModel {
                    user_id: Set(user_id),
                    workspace_id: Set(workspace_id),
                    channel_id: Set(channel_id),
                    conversation_id: Set(conversation_id),
                    channel_name: Set(channel.name),
                    channel_kind: Set(channel.channel_kind),
                    position: Set(channel.position),
                    last_message_seq: Set(unread.as_ref().and_then(|row| row.last_message_seq)),
                    last_read_conversation_message_seq: Set(unread
                        .as_ref()
                        .and_then(|row| row.last_read_conversation_message_seq)),
                    unread_count: Set(unread.map_or(0, |row| row.unread_count)),
                    updated_at: Set(updated_at),
                }
                .insert(&self.db)
                .await?;
            }
        }

        Ok(())
    }

    async fn compose_workspace_unread(
        &self,
        user_id: Option<Uuid>,
        workspace_id: Option<Uuid>,
        conversation_id: Option<Uuid>,
    ) -> Result<(), sea_orm::DbErr> {
        if let Some(conversation_id) = conversation_id {
            let Some(conversation) = conversation_snapshot::Entity::find_by_id(conversation_id)
                .one(&self.db)
                .await?
            else {
                return Ok(());
            };
            let Some(channel_id) = conversation.workspace_channel_id else {
                return Ok(());
            };
            let Some(channel) = workspace_channel_snapshot::Entity::find_by_id(channel_id)
                .one(&self.db)
                .await?
            else {
                return Ok(());
            };
            self.compose_workspace_channel(
                user_id,
                Some(channel.workspace_id),
                Some(channel_id),
                Some(conversation_id),
            )
            .await?;
            return Ok(());
        }

        if let (Some(user_id), Some(workspace_id)) = (user_id, workspace_id) {
            self.compose_workspace_unread_for_member(user_id, workspace_id)
                .await?;
        }

        Ok(())
    }

    async fn compose_workspace_channel_unread(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        channel_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<Option<workspace_channel_unread_projection::Model>, sea_orm::DbErr> {
        let message = conversation_message_state::Entity::find_by_id(conversation_id)
            .one(&self.db)
            .await?;
        let read = conversation_read_state::Entity::find_by_id((conversation_id, user_id))
            .one(&self.db)
            .await?;
        let unread_count = unread_count(
            user_id,
            message.as_ref(),
            read.as_ref()
                .map(|row| row.last_read_conversation_message_seq),
        );
        let updated_at = Utc::now().into();

        let model =
            match workspace_channel_unread_projection::Entity::find_by_id((user_id, channel_id))
                .one(&self.db)
                .await?
            {
                Some(existing) => {
                    let mut active = existing.into_active_model();
                    active.workspace_id = Set(workspace_id);
                    active.conversation_id = Set(conversation_id);
                    active.last_message_seq =
                        Set(message.as_ref().and_then(|row| row.last_message_seq));
                    active.last_read_conversation_message_seq = Set(read
                        .as_ref()
                        .map(|row| row.last_read_conversation_message_seq));
                    active.unread_count = Set(unread_count);
                    active.updated_at = Set(updated_at);
                    active.update(&self.db).await?
                }
                None => {
                    workspace_channel_unread_projection::ActiveModel {
                        user_id: Set(user_id),
                        workspace_id: Set(workspace_id),
                        channel_id: Set(channel_id),
                        conversation_id: Set(conversation_id),
                        last_message_seq: Set(message
                            .as_ref()
                            .and_then(|row| row.last_message_seq)),
                        last_read_conversation_message_seq: Set(read
                            .as_ref()
                            .map(|row| row.last_read_conversation_message_seq)),
                        unread_count: Set(unread_count),
                        updated_at: Set(updated_at),
                    }
                    .insert(&self.db)
                    .await?
                }
            };

        self.compose_workspace_unread_for_member(user_id, workspace_id)
            .await?;

        Ok(Some(model))
    }

    async fn compose_workspace_unread_for_member(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<(), sea_orm::DbErr> {
        let unread_count = workspace_channel_unread_projection::Entity::find()
            .filter(workspace_channel_unread_projection::Column::UserId.eq(user_id))
            .filter(workspace_channel_unread_projection::Column::WorkspaceId.eq(workspace_id))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|row| row.unread_count)
            .sum::<i32>();
        let updated_at = Utc::now().into();

        match workspace_unread_projection::Entity::find_by_id((user_id, workspace_id))
            .one(&self.db)
            .await?
        {
            Some(existing) => {
                let mut active = existing.into_active_model();
                active.unread_count = Set(unread_count);
                active.updated_at = Set(updated_at);
                active.update(&self.db).await?;
            }
            None => {
                workspace_unread_projection::ActiveModel {
                    user_id: Set(user_id),
                    workspace_id: Set(workspace_id),
                    unread_count: Set(unread_count),
                    updated_at: Set(updated_at),
                }
                .insert(&self.db)
                .await?;
            }
        }

        self.compose_workspace_for_member(user_id, workspace_id)
            .await?;

        Ok(())
    }

    async fn compose_dm(
        &self,
        user_id: Option<Uuid>,
        dm_pair_id: Option<Uuid>,
        conversation_id: Option<Uuid>,
    ) -> Result<(), sea_orm::DbErr> {
        if let (Some(user_id), None, None) = (user_id, dm_pair_id, conversation_id) {
            let pairs = dm_pair_snapshot::Entity::find()
                .filter(
                    Condition::any()
                        .add(dm_pair_snapshot::Column::LowUserId.eq(user_id))
                        .add(dm_pair_snapshot::Column::HighUserId.eq(user_id)),
                )
                .all(&self.db)
                .await?;

            for pair in pairs {
                self.compose_dm_for_user(pair.low_user_id, &pair).await?;
                self.compose_dm_for_user(pair.high_user_id, &pair).await?;
            }

            return Ok(());
        }

        let dm_pair_id = match dm_pair_id {
            Some(dm_pair_id) => Some(dm_pair_id),
            None => match conversation_id {
                Some(conversation_id) => conversation_snapshot::Entity::find_by_id(conversation_id)
                    .one(&self.db)
                    .await?
                    .and_then(|conversation| conversation.dm_pair_id),
                None => None,
            },
        };

        let Some(dm_pair_id) = dm_pair_id else {
            return Ok(());
        };
        let Some(pair) = dm_pair_snapshot::Entity::find_by_id(dm_pair_id)
            .one(&self.db)
            .await?
        else {
            return Ok(());
        };

        match user_id {
            Some(user_id) => self.compose_dm_for_user(user_id, &pair).await?,
            None => {
                self.compose_dm_for_user(pair.low_user_id, &pair).await?;
                self.compose_dm_for_user(pair.high_user_id, &pair).await?;
            }
        }

        Ok(())
    }

    async fn compose_dm_for_user(
        &self,
        user_id: Uuid,
        pair: &dm_pair_snapshot::Model,
    ) -> Result<(), sea_orm::DbErr> {
        if user_id != pair.low_user_id && user_id != pair.high_user_id {
            return Ok(());
        }

        let peer_user_id = if user_id == pair.low_user_id {
            pair.high_user_id
        } else {
            pair.low_user_id
        };
        let peer = user_snapshot::Entity::find_by_id(peer_user_id)
            .one(&self.db)
            .await?;
        let conversation = conversation_snapshot::Entity::find()
            .filter(conversation_snapshot::Column::DmPairId.eq(pair.dm_pair_id))
            .one(&self.db)
            .await?;
        let conversation_id = conversation.as_ref().map(|row| row.conversation_id);
        let unread = match conversation_id {
            Some(conversation_id) => {
                self.compose_dm_unread(user_id, pair.dm_pair_id, conversation_id)
                    .await?
            }
            None => None,
        };
        let message = match conversation_id {
            Some(conversation_id) => {
                conversation_message_state::Entity::find_by_id(conversation_id)
                    .one(&self.db)
                    .await?
            }
            None => None,
        };
        let updated_at = Utc::now().into();
        let peer_username = peer
            .as_ref()
            .map_or_else(String::new, |row| row.username.clone());
        let peer_display_name = peer
            .as_ref()
            .map_or_else(String::new, |row| row.display_name.clone());
        let peer_avatar_url = peer.and_then(|row| row.avatar_url);

        match dm_projection::Entity::find_by_id((user_id, pair.dm_pair_id))
            .one(&self.db)
            .await?
        {
            Some(existing) => {
                let mut active = existing.into_active_model();
                active.conversation_id = Set(conversation_id);
                active.peer_user_id = Set(peer_user_id);
                active.peer_username = Set(peer_username);
                active.peer_display_name = Set(peer_display_name);
                active.peer_avatar_url = Set(peer_avatar_url);
                active.last_message_seq =
                    Set(message.as_ref().and_then(|row| row.last_message_seq));
                active.last_read_conversation_message_seq = Set(unread
                    .as_ref()
                    .and_then(|row| row.last_read_conversation_message_seq));
                active.last_message_preview = Set(message
                    .as_ref()
                    .and_then(|row| row.last_message_preview.clone()));
                active.last_activity_at =
                    Set(message.as_ref().and_then(|row| row.last_activity_at));
                active.unread_count = Set(unread.map_or(0, |row| row.unread_count));
                active.updated_at = Set(updated_at);
                active.update(&self.db).await?;
            }
            None => {
                dm_projection::ActiveModel {
                    user_id: Set(user_id),
                    conversation_id: Set(conversation_id),
                    dm_pair_id: Set(pair.dm_pair_id),
                    peer_user_id: Set(peer_user_id),
                    peer_username: Set(peer_username),
                    peer_display_name: Set(peer_display_name),
                    peer_avatar_url: Set(peer_avatar_url),
                    last_message_seq: Set(message.as_ref().and_then(|row| row.last_message_seq)),
                    last_read_conversation_message_seq: Set(unread
                        .as_ref()
                        .and_then(|row| row.last_read_conversation_message_seq)),
                    last_message_preview: Set(message
                        .as_ref()
                        .and_then(|row| row.last_message_preview.clone())),
                    last_activity_at: Set(message.as_ref().and_then(|row| row.last_activity_at)),
                    unread_count: Set(unread.map_or(0, |row| row.unread_count)),
                    updated_at: Set(updated_at),
                }
                .insert(&self.db)
                .await?;
            }
        }

        Ok(())
    }

    async fn compose_dm_unread(
        &self,
        user_id: Uuid,
        dm_pair_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<Option<dm_unread_projection::Model>, sea_orm::DbErr> {
        let message = conversation_message_state::Entity::find_by_id(conversation_id)
            .one(&self.db)
            .await?;
        let read = conversation_read_state::Entity::find_by_id((conversation_id, user_id))
            .one(&self.db)
            .await?;
        let unread_count = unread_count(
            user_id,
            message.as_ref(),
            read.as_ref()
                .map(|row| row.last_read_conversation_message_seq),
        );
        let updated_at = Utc::now().into();

        let model = match dm_unread_projection::Entity::find_by_id((user_id, dm_pair_id))
            .one(&self.db)
            .await?
        {
            Some(existing) => {
                let mut active = existing.into_active_model();
                active.conversation_id = Set(conversation_id);
                active.last_message_seq =
                    Set(message.as_ref().and_then(|row| row.last_message_seq));
                active.last_read_conversation_message_seq = Set(read
                    .as_ref()
                    .map(|row| row.last_read_conversation_message_seq));
                active.unread_count = Set(unread_count);
                active.updated_at = Set(updated_at);
                active.update(&self.db).await?
            }
            None => {
                dm_unread_projection::ActiveModel {
                    user_id: Set(user_id),
                    dm_pair_id: Set(dm_pair_id),
                    conversation_id: Set(conversation_id),
                    last_message_seq: Set(message.as_ref().and_then(|row| row.last_message_seq)),
                    last_read_conversation_message_seq: Set(read
                        .as_ref()
                        .map(|row| row.last_read_conversation_message_seq)),
                    unread_count: Set(unread_count),
                    updated_at: Set(updated_at),
                }
                .insert(&self.db)
                .await?
            }
        };

        Ok(Some(model))
    }
}

fn unread_count(
    user_id: Uuid,
    message: Option<&conversation_message_state::Model>,
    last_read_seq: Option<i64>,
) -> i32 {
    if message.and_then(|message| message.last_message_author_user_id) == Some(user_id) {
        return 0;
    }

    message
        .and_then(|message| message.last_message_seq)
        .map(|last_message_seq| {
            last_message_seq
                .saturating_sub(last_read_seq.unwrap_or(0))
                .max(0)
                .min(i32::MAX as i64) as i32
        })
        .unwrap_or(0)
}

#[allow(dead_code)]
fn newest(left: DateTime<FixedOffset>, right: DateTime<FixedOffset>) -> DateTime<FixedOffset> {
    left.max(right)
}
