use axum::extract::ws::Message;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use tokio::sync::mpsc::Sender;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct SessionRegistryPayload {
    pub actor_id: Uuid,
    pub connected_at: DateTime<Utc>,
    pub last_activity_at: DateTime<Utc>,
    pub sender: Sender<Message>,
}

// Mapping Session ID -> Session Registry Payload
type SessionRegistry = DashMap<Uuid, SessionRegistryPayload>;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TargetKind {
    WorkspaceChannel,
    DirectMessage,
}

#[derive(Clone, Debug)]
pub struct SubscriptionSessionPayload {
    pub actor_id: Uuid,
    pub subscribed_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct SubscriptionMapPayload {
    pub session_ids: DashMap<Uuid, SubscriptionSessionPayload>,
}

// Mapping (Target Kind, Target ID) -> Subscription Payload
type SubscriptionMap = DashMap<(TargetKind, Uuid), SubscriptionMapPayload>;

pub struct Store {
    pub session_registry: SessionRegistry,
    pub subscription_map: SubscriptionMap,
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

impl Store {
    pub fn new() -> Self {
        Self {
            session_registry: DashMap::new(),
            subscription_map: DashMap::new(),
        }
    }

    pub fn get_session(&self, session_id: &Uuid) -> Option<SessionRegistryPayload> {
        self.session_registry
            .get(session_id)
            .map(|entry| entry.clone())
    }

    pub fn create_session(
        &self,
        session_id: Uuid,
        actor_id: Uuid,
        connected_at: DateTime<Utc>,
        sender: Sender<Message>,
    ) {
        self.session_registry.insert(
            session_id,
            SessionRegistryPayload {
                actor_id,
                connected_at,
                last_activity_at: connected_at,
                sender,
            },
        );
    }

    pub fn remove_session(&self, session_id: &Uuid) -> Option<SessionRegistryPayload> {
        self.session_registry
            .remove(session_id)
            .map(|(_, value)| value)
    }

    pub fn get_subscription(
        &self,
        target_kind: &TargetKind,
        target_id: &Uuid,
    ) -> Option<SubscriptionMapPayload> {
        self.subscription_map
            .get(&(target_kind.clone(), *target_id))
            .map(|entry| SubscriptionMapPayload {
                session_ids: entry.session_ids.clone(),
            })
    }

    pub fn create_session_subscription(
        &self,
        target_kind: TargetKind,
        target_id: Uuid,
        session_id: Uuid,
        actor_id: Uuid,
        subscribed_at: DateTime<Utc>,
    ) {
        let entry = self
            .subscription_map
            .entry((target_kind, target_id))
            .or_insert_with(|| SubscriptionMapPayload {
                session_ids: DashMap::new(),
            });
        entry.session_ids.insert(
            session_id,
            SubscriptionSessionPayload {
                actor_id,
                subscribed_at,
            },
        );
    }

    pub fn remove_session_subscription(
        &self,
        target_kind: &TargetKind,
        target_id: &Uuid,
        session_id: &Uuid,
    ) -> Option<SubscriptionSessionPayload> {
        let key = (target_kind.clone(), *target_id);
        let payload = self.subscription_map.get(&key).and_then(|subscription| {
            subscription
                .session_ids
                .remove(session_id)
                .map(|(_, value)| value)
        });

        if self
            .subscription_map
            .get(&key)
            .is_some_and(|subscription| subscription.session_ids.is_empty())
        {
            self.subscription_map.remove(&key);
        }

        payload
    }

    pub fn remove_session_subscriptions(&self, session_id: &Uuid) {
        let keys: Vec<(TargetKind, Uuid)> = self
            .subscription_map
            .iter()
            .map(|entry| entry.key().clone())
            .collect();

        for key in keys {
            if let Some(subscription) = self.subscription_map.get(&key) {
                subscription.session_ids.remove(session_id);
                let is_empty = subscription.session_ids.is_empty();
                drop(subscription);

                if is_empty {
                    self.subscription_map.remove(&key);
                }
            }
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_registry_round_trip() {
        let store = Store::new();
        let (sender, _receiver) = tokio::sync::mpsc::channel(1);
        let session_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let connected_at = Utc::now();

        store.create_session(session_id, actor_id, connected_at, sender);

        let session = store.get_session(&session_id).expect("session missing");
        assert_eq!(session.actor_id, actor_id);
        assert_eq!(session.connected_at, connected_at);
        assert_eq!(session.last_activity_at, connected_at);

        let removed = store.remove_session(&session_id).expect("session missing");
        assert_eq!(removed.actor_id, actor_id);
    }

    #[test]
    fn subscription_registry_round_trip() {
        let store = Store::new();
        let target_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let subscribed_at = Utc::now();

        store.create_session_subscription(
            TargetKind::WorkspaceChannel,
            target_id,
            session_id,
            actor_id,
            subscribed_at,
        );

        let subscription = store
            .get_subscription(&TargetKind::WorkspaceChannel, &target_id)
            .expect("subscription missing");

        let entry = subscription
            .session_ids
            .get(&session_id)
            .expect("session missing");
        assert_eq!(entry.actor_id, actor_id);
        assert_eq!(entry.subscribed_at, subscribed_at);

        let removed = store
            .remove_session_subscription(&TargetKind::WorkspaceChannel, &target_id, &session_id)
            .expect("subscription missing");
        assert_eq!(removed.actor_id, actor_id);
        assert!(
            store
                .get_subscription(&TargetKind::WorkspaceChannel, &target_id)
                .is_none()
        );
    }

}
