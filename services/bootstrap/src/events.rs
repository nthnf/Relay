use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct UserRegisteredPayload {
    pub user_id: String,
    pub email: String,
    pub email_verified: bool,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub registered_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct UserProfileUpdatedPayload {
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct FriendRequestCreatedPayload {
    pub friend_request_id: String,
    pub requester_user_id: String,
    pub addressee_user_id: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct FriendRequestAcceptedPayload {
    pub friend_request_id: String,
    pub requester_user_id: String,
    pub addressee_user_id: String,
    pub accepted_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct FriendRequestRejectedPayload {
    pub friend_request_id: String,
    pub requester_user_id: String,
    pub addressee_user_id: String,
    pub rejected_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct FriendRequestCanceledByBlockPayload {
    pub friend_request_id: String,
    pub requester_user_id: String,
    pub addressee_user_id: String,
    pub blocked_by_user_id: String,
    pub canceled_at: String,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct WorkspaceCreatedPayload {
    pub workspace_id: String,
    pub name: String,
    pub owner_user_id: String,
    pub created_at: String,
    pub initial_member_user_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct WorkspaceMemberAddedPayload {
    pub workspace_id: String,
    pub user_id: String,
    pub joined_at: String,
    pub added_by_user_id: String,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct WorkspaceMemberRemovedPayload {
    pub workspace_id: String,
    pub user_id: String,
    pub removed_at: String,
    pub removed_by_user_id: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct WorkspaceChannelCreatedPayload {
    pub channel_id: String,
    pub workspace_id: String,
    pub name: String,
    pub channel_kind: String,
    pub position: i32,
    pub created_by_user_id: String,
    pub created_at: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationTargetType {
    Dm,
    WorkspaceChannel,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ConversationCreatedPayload {
    pub conversation_id: String,
    pub target_type: ConversationTargetType,
    pub dm_pair_id: Option<String>,
    pub workspace_channel_id: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct DmPairCreatedPayload {
    pub dm_pair_id: String,
    pub low_user_id: String,
    pub high_user_id: String,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct MessageCreatedPayload {
    pub delivery_id: String,
    pub message_id: String,
    pub conversation_id: String,
    pub target_type: ConversationTargetType,
    pub workspace_id: Option<String>,
    pub workspace_channel_id: Option<String>,
    pub author_user_id: String,
    pub conversation_message_seq: i64,
    pub body: String,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct MessageEditedPayload {
    pub delivery_id: String,
    pub message_id: String,
    pub conversation_id: String,
    pub target_type: ConversationTargetType,
    pub workspace_id: Option<String>,
    pub workspace_channel_id: Option<String>,
    pub editor_user_id: String,
    pub body: String,
    pub edited_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct MessageDeletedPayload {
    pub delivery_id: String,
    pub message_id: String,
    pub conversation_id: String,
    pub target_type: ConversationTargetType,
    pub workspace_id: Option<String>,
    pub workspace_channel_id: Option<String>,
    pub deleted_by_user_id: String,
    pub deleted_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ConversationReadCursorUpdatedPayload {
    pub conversation_id: String,
    pub target_type: ConversationTargetType,
    pub workspace_channel_id: Option<String>,
    pub user_id: String,
    pub last_read_conversation_message_seq: i64,
    pub read_at: String,
}
