use std::{error::Error, net::SocketAddr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub db_url: String,
    pub amqp_addr: String,
    pub workspace_service_url: String,
    pub realtime_service_url: String,
    pub bind_addr: SocketAddr,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn Error + Send + Sync>> {
        dotenvy::dotenv().ok();

        Ok(Self {
            db_url: std::env::var("DATABASE_URL")?,
            amqp_addr: std::env::var("AMQP_ADDR")?,
            workspace_service_url: std::env::var("WORKSPACE_SERVICE_URL")?,
            realtime_service_url: std::env::var("REALTIME_SERVICE_URL")?,
            bind_addr: std::env::var("BIND_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:50051".to_string())
                .parse()?,
        })
    }
}
