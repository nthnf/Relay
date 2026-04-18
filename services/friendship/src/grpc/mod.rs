pub mod lib;

pub mod accept_friend_request;
pub mod block_user;
pub mod create_friend_request;
pub mod handler;
pub mod list_friends;
pub mod list_pending_requests;
pub mod reject_friend_request;
pub mod remove_friend;
pub mod unblock_user;

pub use handler::Handler as FriendshipServer;
