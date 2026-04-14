use std::{error::Error, net::SocketAddr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub db_url: String,
    pub identity_url: String,
    pub bind_addr: SocketAddr,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn Error + Send + Sync>> {
        dotenvy::dotenv().ok();

        Ok(Self {
            db_url: std::env::var("DATABASE_URL")?,
            identity_url: std::env::var("IDENTITY_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:50052".to_string()),
            bind_addr: std::env::var("BIND_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:50051".to_string())
                .parse()?,
        })
    }
}
