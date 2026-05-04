use chrono::Utc;
use redis::{AsyncTypedCommands, Client, RedisResult, aio::ConnectionManager, pipe};
use std::time::Duration;
use tokio::time::sleep;
use tracing::warn;
use uuid::Uuid;

const PRESENCE_TTL_SECS: i64 = 45;
const CONNECT_RETRY_DELAY: Duration = Duration::from_secs(2);
const CONNECT_MAX_RETRIES: usize = 30;

pub struct RedisStore {
    pub conn: ConnectionManager,
}

pub struct PresenceSummary {
    pub online: bool,
    pub last_seen_at: Option<chrono::DateTime<Utc>>,
}

impl RedisStore {
    pub async fn new(url: &str) -> RedisResult<Self> {
        let client = Client::open(url)?;
        let conn = connection_manager_with_retry(&client).await?;
        Ok(Self { conn })
    }

    pub async fn mark_online(&self, user_id: Uuid, session_id: Uuid) -> RedisResult<usize> {
        let now = Utc::now();
        let session_key = presence_sessions_key(user_id);
        let state_key = presence_state_key(user_id);
        let mut conn = self.conn.clone();

        pipe()
            .atomic()
            .sadd(&session_key, session_id)
            .hset(&state_key, "presence", "online")
            .hset(&state_key, "last_seen_at", now.to_rfc3339())
            .expire(&state_key, PRESENCE_TTL_SECS)
            .expire(&session_key, PRESENCE_TTL_SECS)
            .exec_async(&mut conn)
            .await?;

        let count = conn.scard(&session_key).await?;

        conn.hset(&state_key, "session_count", count).await?;

        Ok(count)
    }

    pub async fn mark_offline(&self, user_id: Uuid, session_id: Uuid) -> RedisResult<usize> {
        let now = Utc::now();
        let sessions_key = presence_sessions_key(user_id);
        let state_key = presence_state_key(user_id);
        let mut conn = self.conn.clone();

        pipe()
            .atomic()
            .srem(&sessions_key, session_id)
            .exec_async(&mut conn)
            .await?;

        let count = conn.scard(&sessions_key).await?;
        let presence = if count == 0 { "offline" } else { "online" };

        pipe()
            .atomic()
            .hset(&state_key, "presence", presence)
            .hset(&state_key, "last_seen_at", now.to_rfc3339())
            .hset(&state_key, "session_count", count)
            .expire(&sessions_key, PRESENCE_TTL_SECS)
            .expire(&state_key, PRESENCE_TTL_SECS)
            .exec_async(&mut conn)
            .await?;

        Ok(count)
    }

    pub async fn get_session_count(&self, user_id: Uuid) -> RedisResult<usize> {
        let key = presence_sessions_key(user_id);
        let mut conn = self.conn.clone();
        conn.scard(key).await
    }

    pub async fn get_presence(&self, user_id: Uuid) -> RedisResult<PresenceSummary> {
        let state_key = presence_state_key(user_id);
        let mut conn = self.conn.clone();

        let presence: Option<String> = conn.hget(&state_key, "presence").await?;
        let last_seen_at: Option<String> = conn.hget(&state_key, "last_seen_at").await?;
        let last_seen_at = last_seen_at.and_then(|value| {
            chrono::DateTime::parse_from_rfc3339(&value)
                .ok()
                .map(|value| value.with_timezone(&Utc))
        });

        Ok(PresenceSummary {
            online: presence.as_deref() == Some("online"),
            last_seen_at,
        })
    }

    pub async fn refresh_presence_ttl(&self, user_id: Uuid) -> RedisResult<()> {
        let sessions_key = presence_sessions_key(user_id);
        let state_key = presence_state_key(user_id);
        let mut conn = self.conn.clone();
        pipe()
            .atomic()
            .expire(&sessions_key, PRESENCE_TTL_SECS)
            .expire(&state_key, PRESENCE_TTL_SECS)
            .exec_async(&mut conn)
            .await
    }
}

async fn connection_manager_with_retry(client: &Client) -> RedisResult<ConnectionManager> {
    let mut last_error = None;

    for attempt in 1..=CONNECT_MAX_RETRIES {
        match client.get_connection_manager().await {
            Ok(connection) => return Ok(connection),
            Err(error) => {
                warn!(attempt, error = %error, "redis connection failed; retrying");
                last_error = Some(error);
                sleep(CONNECT_RETRY_DELAY).await;
            }
        }
    }

    Err(last_error.expect("redis connection should have been attempted"))
}

pub fn presence_state_key(user_id: Uuid) -> String {
    format!("presence_state:{user_id}")
}

pub fn presence_sessions_key(user_id: Uuid) -> String {
    format!("presence_sessions:{user_id}")
}
