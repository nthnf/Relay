use serde::Serialize;

#[derive(Serialize)]
pub struct FriendshipPairPayload {
    pub user_id: String,
    pub friend_user_id: String,
}

#[derive(Serialize)]
pub struct FriendRequestCreatedPayload {
    pub friend_request_id: String,
    pub requester_user_id: String,
    pub addressee_user_id: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct FriendRequestAcceptedPayload {
    pub friend_request_id: String,
    pub requester_user_id: String,
    pub addressee_user_id: String,
    pub accepted_at: String,
    pub friendship_pairs: Vec<FriendshipPairPayload>,
}

#[derive(Serialize)]
pub struct FriendRequestRejectedPayload {
    pub friend_request_id: String,
    pub requester_user_id: String,
    pub addressee_user_id: String,
    pub rejected_at: String,
}

#[derive(Serialize)]
pub struct FriendRequestCanceledByBlockPayload {
    pub friend_request_id: String,
    pub requester_user_id: String,
    pub addressee_user_id: String,
    pub blocked_by_user_id: String,
    pub canceled_at: String,
    pub status: String,
}

#[derive(Serialize)]
pub struct FriendshipRemovedPayload {
    pub friendship_pairs: Vec<FriendshipPairPayload>,
    pub removed_at: String,
    pub reason: String,
}

#[derive(Serialize)]
pub struct UserBlockedPayload {
    pub blocker_user_id: String,
    pub blocked_user_id: String,
    pub blocked_at: String,
}

#[derive(Serialize)]
pub struct UserUnblockedPayload {
    pub blocker_user_id: String,
    pub blocked_user_id: String,
    pub unblocked_at: String,
}
