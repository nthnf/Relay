use realtime::redis::{RedisStore, presence_sessions_key, presence_state_key};
use redis::AsyncCommands;
use testcontainers::{
    GenericImage,
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
};
use uuid::Uuid;

async fn start_store() -> Result<
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

#[tokio::test]
async fn mark_online_adds_session_and_updates_presence()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (mut store, _container) = start_store().await?;
    let user_id = Uuid::new_v4();
    let session_1 = Uuid::new_v4();
    let state_key = presence_state_key(user_id);

    let count = store.mark_online(user_id, session_1).await?;

    assert_eq!(count, 1);
    assert_eq!(store.get_session_count(user_id).await?, 1);
    assert_eq!(
        store
            .conn
            .hget::<_, _, String>(&state_key, "presence")
            .await?,
        "online"
    );
    assert_eq!(
        store
            .conn
            .hget::<_, _, usize>(&state_key, "session_count")
            .await?,
        1
    );

    Ok(())
}

#[tokio::test]
async fn mark_online_counts_multiple_sessions()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (mut store, _container) = start_store().await?;
    let user_id = Uuid::new_v4();
    let session_1 = Uuid::new_v4();
    let session_2 = Uuid::new_v4();
    let state_key = presence_state_key(user_id);

    assert_eq!(store.mark_online(user_id, session_1).await?, 1);
    assert_eq!(store.mark_online(user_id, session_2).await?, 2);
    assert_eq!(store.get_session_count(user_id).await?, 2);
    assert_eq!(
        store
            .conn
            .hget::<_, _, String>(&state_key, "presence")
            .await?,
        "online"
    );
    assert_eq!(
        store
            .conn
            .hget::<_, _, usize>(&state_key, "session_count")
            .await?,
        2
    );

    Ok(())
}

#[tokio::test]
async fn mark_offline_flips_only_on_last_session()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (mut store, _container) = start_store().await?;
    let user_id = Uuid::new_v4();
    let session_1 = Uuid::new_v4();
    let session_2 = Uuid::new_v4();
    let state_key = presence_state_key(user_id);
    let sessions_key = presence_sessions_key(user_id);

    store.mark_online(user_id, session_1).await?;
    store.mark_online(user_id, session_2).await?;

    assert_eq!(store.mark_offline(user_id, session_1).await?, 1);
    assert_eq!(store.get_session_count(user_id).await?, 1);
    assert_eq!(
        store
            .conn
            .hget::<_, _, String>(&state_key, "presence")
            .await?,
        "online"
    );
    assert_eq!(
        store
            .conn
            .hget::<_, _, usize>(&state_key, "session_count")
            .await?,
        1
    );

    assert_eq!(store.mark_offline(user_id, session_2).await?, 0);
    assert_eq!(store.get_session_count(user_id).await?, 0);
    assert_eq!(
        store
            .conn
            .hget::<_, _, String>(&state_key, "presence")
            .await?,
        "offline"
    );
    assert_eq!(
        store
            .conn
            .hget::<_, _, usize>(&state_key, "session_count")
            .await?,
        0
    );
    let set_size: usize = store.conn.scard(&sessions_key).await?;
    assert_eq!(set_size, 0);

    Ok(())
}

#[tokio::test]
async fn refresh_presence_ttl_keeps_keys_alive()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (mut store, _container) = start_store().await?;
    let user_id = Uuid::new_v4();
    let session_1 = Uuid::new_v4();
    let state_key = presence_state_key(user_id);
    let sessions_key = presence_sessions_key(user_id);

    store.mark_online(user_id, session_1).await?;
    store.refresh_presence_ttl(user_id).await?;

    let state_ttl: i64 = redis::cmd("TTL")
        .arg(&state_key)
        .query_async(&mut store.conn)
        .await?;
    let sessions_ttl: i64 = redis::cmd("TTL")
        .arg(&sessions_key)
        .query_async(&mut store.conn)
        .await?;

    assert!(state_ttl > 0);
    assert!(sessions_ttl > 0);

    Ok(())
}
