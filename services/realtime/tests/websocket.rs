use futures_util::SinkExt;
use realtime::{
    redis::RedisStore,
    store::{Store, TargetKind},
    websocket,
};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use testcontainers::{
    GenericImage,
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
};
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::handshake::client::generate_key;
use tokio_tungstenite::tungstenite::http::Request;
use uuid::Uuid;

async fn start_redis() -> Result<
    (RedisStore, testcontainers::ContainerAsync<GenericImage>),
    Box<dyn std::error::Error + Send + Sync>,
> {
    let container = GenericImage::new("redis", "7.2.4")
        .with_exposed_port(6379.tcp())
        .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
        .start()
        .await?;

    let host = container.get_host().await?;
    let host_port = container.get_host_port_ipv4(6379.tcp()).await?;
    let url = format!("redis://{host}:{host_port}");
    let store = RedisStore::new(&url).await?;

    Ok((store, container))
}

async fn wait_for_subscription(store: &Arc<Store>, target_id: Uuid) {
    let deadline = tokio::time::sleep(Duration::from_secs(3));
    tokio::pin!(deadline);

    loop {
        if let Some(subscription) = store.get_subscription(&TargetKind::WorkspaceChannel, &target_id)
            && !subscription.session_ids.is_empty()
        {
            return;
        }

        tokio::select! {
            _ = &mut deadline => panic!("subscription not created in time"),
            _ = tokio::time::sleep(Duration::from_millis(25)) => {}
        }
    }
}

async fn wait_for_unsubscription(store: &Arc<Store>, target_id: Uuid) {
    let deadline = tokio::time::sleep(Duration::from_secs(3));
    tokio::pin!(deadline);

    loop {
        if store
            .get_subscription(&TargetKind::WorkspaceChannel, &target_id)
            .is_none()
        {
            return;
        }

        tokio::select! {
            _ = &mut deadline => panic!("subscription not removed in time"),
            _ = tokio::time::sleep(Duration::from_millis(25)) => {}
        }
    }
}

#[tokio::test]
async fn websocket_upgrade_and_subscription_flow() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (redis, _container) = start_redis().await?;
    let store = Arc::new(Store::new());
    let redis = Arc::new(redis);
    let app = websocket::app(store.clone(), redis);

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr: SocketAddr = listener.local_addr()?;

    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .expect("serve websocket app");
    });

    let target_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let request = Request::builder()
        .uri(format!("ws://{addr}/ws"))
        .header("Host", addr.to_string())
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", generate_key())
        .header("x-user-id", actor_id.to_string())
        .body(())?;

    let (mut socket, _response) = connect_async(request).await?;

    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            format!(
                r#"{{"type":"subscribe","target_kind":"workspace_channel","target_id":"{}"}}"#,
                target_id
            )
            .into(),
        ))
        .await?;

    wait_for_subscription(&store, target_id).await;

    socket.close(None).await?;
    wait_for_unsubscription(&store, target_id).await;

    server.abort();

    Ok(())
}

#[tokio::test]
async fn websocket_unsubscribe_flow() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (redis, _container) = start_redis().await?;
    let store = Arc::new(Store::new());
    let redis = Arc::new(redis);
    let app = websocket::app(store.clone(), redis);

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr: SocketAddr = listener.local_addr()?;

    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .expect("serve websocket app");
    });

    let target_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let request = Request::builder()
        .uri(format!("ws://{addr}/ws"))
        .header("Host", addr.to_string())
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", generate_key())
        .header("x-user-id", actor_id.to_string())
        .body(())?;

    let (mut socket, _response) = connect_async(request).await?;

    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            format!(
                r#"{{"type":"subscribe","target_kind":"workspace_channel","target_id":"{}"}}"#,
                target_id
            )
            .into(),
        ))
        .await?;

    wait_for_subscription(&store, target_id).await;

    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            format!(
                r#"{{"type":"unsubscribe","target_kind":"workspace_channel","target_id":"{}"}}"#,
                target_id
            )
            .into(),
        ))
        .await?;

    wait_for_unsubscription(&store, target_id).await;

    socket.close(None).await?;
    server.abort();

    Ok(())
}
