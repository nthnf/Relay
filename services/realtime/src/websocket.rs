use axum::{
    Router,
    extract::{
        State,
        connect_info::ConnectInfo,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode, Uri},
    response::IntoResponse,
    routing::any,
};
use axum_extra::TypedHeader;
use chrono::Utc;
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::Deserialize;
use std::{error::Error, net::SocketAddr, sync::Arc};
use tokio::{net::TcpListener, sync::mpsc};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

use crate::redis::RedisStore;
use crate::store::{Store, TargetKind};

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Store>,
    pub redis: Arc<RedisStore>,
}

pub async fn run(
    bind_addr: SocketAddr,
    store: Arc<Store>,
    redis: Arc<RedisStore>,
) -> Result<(), Box<dyn Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = app(store, redis);

    let listener = TcpListener::bind(bind_addr).await?;

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(|error| -> Box<dyn Error> { Box::new(error) })?;

    Ok(())
}

pub fn app(store: Arc<Store>, redis: Arc<RedisStore>) -> Router {
    Router::new()
        .route("/ws", any(ws_handler))
        .with_state(AppState { store, redis })
}

async fn ws_handler(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    uri: Uri,
) -> Result<impl IntoResponse, StatusCode> {
    let user_agent = user_agent
        .map(|TypedHeader(user_agent)| user_agent.to_string())
        .unwrap_or_else(|| String::from("Unknown browser"));

    let actor_id = headers
        .get("x-user-id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::parse_str(value).ok())
        .or_else(|| actor_id_from_query(&uri))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    tracing::debug!(%user_agent, %addr, %actor_id, "ws connected");

    Ok(ws.on_upgrade(move |socket| handle_socket(socket, state.store, state.redis, actor_id)))
}

fn actor_id_from_query(uri: &Uri) -> Option<Uuid> {
    uri.query()?.split('&').find_map(|part| {
        let (key, value) = part.split_once('=')?;
        (key == "user_id")
            .then(|| Uuid::parse_str(value).ok())
            .flatten()
    })
}

async fn handle_socket(
    socket: WebSocket,
    store: Arc<Store>,
    redis: Arc<RedisStore>,
    actor_id: Uuid,
) {
    let session_id = Uuid::new_v4();
    let connected_at = Utc::now();
    let (mut client_sink, mut client_stream) = socket.split();
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<Message>(32);

    store.create_session(session_id, actor_id, connected_at, outbound_tx.clone());

    if let Err(error) = redis.mark_online(actor_id, session_id).await {
        tracing::warn!(%actor_id, %session_id, %error, "presence online update failed");
        store.remove_session(&session_id);
        return;
    }

    let writer = tokio::spawn(async move {
        while let Some(message) = outbound_rx.recv().await {
            if client_sink.send(message).await.is_err() {
                break;
            }
        }
    });

    let mut presence_refresh = tokio::time::interval(std::time::Duration::from_secs(15));

    loop {
        tokio::select! {
            maybe_message = client_stream.next() => {
                match maybe_message {
                    Some(Ok(Message::Text(text))) => {
                        if let Err(error) = handle_client_message(&store, session_id, actor_id, &text) {
                            tracing::warn!(%actor_id, %session_id, %error, "ws control message rejected");
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(error)) => {
                        tracing::warn!(%actor_id, %session_id, %error, "ws receive error");
                        break;
                    }
                }
            }
            _ = presence_refresh.tick() => {
                if let Err(error) = redis.refresh_presence_ttl(actor_id).await {
                    tracing::warn!(%actor_id, %session_id, %error, "presence ttl refresh failed on heartbeat");
                }
            }
        }
    }

    store.remove_session_subscriptions(&session_id);
    store.remove_session(&session_id);

    if let Err(error) = redis.mark_offline(actor_id, session_id).await {
        tracing::warn!(%actor_id, %session_id, %error, "presence offline update failed");
    }

    drop(outbound_tx);
    let _ = writer.await;
}

fn handle_client_message(
    store: &Arc<Store>,
    session_id: Uuid,
    actor_id: Uuid,
    text: &str,
) -> Result<(), String> {
    let message: ClientMessage = serde_json::from_str(text).map_err(|error| error.to_string())?;

    match message {
        ClientMessage::Subscribe {
            target_kind,
            target_id,
        } => {
            store.create_session_subscription(
                target_kind.into(),
                parse_uuid(&target_id)?,
                session_id,
                actor_id,
                Utc::now(),
            );
            Ok(())
        }
        ClientMessage::Unsubscribe {
            target_kind,
            target_id,
        } => {
            let target_kind = target_kind.into();
            let target_id = parse_uuid(&target_id)?;
            store.remove_session_subscription(&target_kind, &target_id, &session_id);
            Ok(())
        }
    }
}

fn parse_uuid(value: &str) -> Result<Uuid, String> {
    Uuid::parse_str(value).map_err(|error| error.to_string())
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    Subscribe {
        target_kind: ClientTargetKind,
        target_id: String,
    },
    Unsubscribe {
        target_kind: ClientTargetKind,
        target_id: String,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ClientTargetKind {
    WorkspaceChannel,
    DirectMessage,
}

impl From<ClientTargetKind> for TargetKind {
    fn from(value: ClientTargetKind) -> Self {
        match value {
            ClientTargetKind::WorkspaceChannel => TargetKind::WorkspaceChannel,
            ClientTargetKind::DirectMessage => TargetKind::DirectMessage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_client_message_subscribe_adds_subscription() {
        let store = Arc::new(Store::new());
        let session_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();
        let json = format!(
            r#"{{"type":"subscribe","target_kind":"workspace_channel","target_id":"{}"}}"#,
            target_id
        );

        handle_client_message(&store, session_id, actor_id, &json).expect("subscribe failed");

        let subscription = store
            .get_subscription(&TargetKind::WorkspaceChannel, &target_id)
            .expect("subscription missing");
        let entry = subscription
            .session_ids
            .get(&session_id)
            .expect("session missing");

        assert_eq!(entry.actor_id, actor_id);
    }

    #[test]
    fn handle_client_message_unsubscribe_removes_subscription() {
        let store = Arc::new(Store::new());
        let session_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();

        let subscribe_json = format!(
            r#"{{"type":"subscribe","target_kind":"direct_message","target_id":"{}"}}"#,
            target_id
        );
        handle_client_message(&store, session_id, actor_id, &subscribe_json)
            .expect("subscribe failed");

        let unsubscribe_json = format!(
            r#"{{"type":"unsubscribe","target_kind":"direct_message","target_id":"{}"}}"#,
            target_id
        );
        handle_client_message(&store, session_id, actor_id, &unsubscribe_json)
            .expect("unsubscribe failed");

        assert!(
            store
                .get_subscription(&TargetKind::DirectMessage, &target_id)
                .is_none()
        );
    }

    #[test]
    fn handle_client_message_rejects_invalid_json() {
        let store = Arc::new(Store::new());
        let session_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();

        let error = handle_client_message(&store, session_id, actor_id, "not-json")
            .expect_err("invalid json should fail");

        assert!(!error.is_empty());
    }
}
