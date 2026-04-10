use std::{env, time::Duration};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub database_url: String,
    pub amqp_addr: String,
    pub exchange: String,
    pub publisher_service: String,
    pub batch_size: u64,
    pub poll_interval: Duration,
    pub claim_ttl: Duration,
    pub retry_delay: Duration,
    pub max_publish_attempts: i32,
    pub worker_id: String,
}

impl Config {
    pub fn from_env() -> Result<Self, env::VarError> {
        let database_url = env::var("DATABASE_URL")?;
        let publisher_service = env::var("OUTBOX_PUBLISHER_SERVICE")?;

        Ok(Self {
            database_url,
            amqp_addr: env::var("AMQP_ADDR")
                .unwrap_or_else(|_| "amqp://relay:relay@127.0.0.1:5672/%2f".to_string()),
            exchange: env::var("OUTBOX_EXCHANGE").unwrap_or_else(|_| "relay.events".to_string()),
            publisher_service: publisher_service.clone(),
            batch_size: parse_u64("OUTBOX_BATCH_SIZE", 100),
            poll_interval: Duration::from_secs(parse_u64("OUTBOX_POLL_INTERVAL_SECS", 2)),
            claim_ttl: Duration::from_secs(parse_u64("OUTBOX_CLAIM_TTL_SECS", 30)),
            retry_delay: Duration::from_secs(parse_u64("OUTBOX_RETRY_DELAY_SECS", 10)),
            max_publish_attempts: parse_i32("OUTBOX_MAX_PUBLISH_ATTEMPTS", 10),
            worker_id: env::var("OUTBOX_WORKER_ID")
                .unwrap_or_else(|_| format!("{}:{}", publisher_service, std::process::id())),
        })
    }
}

fn parse_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn parse_i32(key: &str, default: i32) -> i32 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn worker_id_defaults_to_service_and_pid() {
        unsafe {
            std::env::set_var("DATABASE_URL", "postgres://relay:relay@localhost/identity");
            std::env::set_var("OUTBOX_PUBLISHER_SERVICE", "identity");
            std::env::remove_var("OUTBOX_WORKER_ID");
        }

        let config = Config::from_env().expect("config");

        assert!(config.worker_id.starts_with("identity:"));
        assert_eq!(config.exchange, "relay.events");
    }
}
