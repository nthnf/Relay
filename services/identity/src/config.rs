use std::{error::Error, net::SocketAddr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub db_url: String,
    pub token_secret: String,
    pub bind_addr: SocketAddr,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn Error>> {
        dotenvy::dotenv().ok();

        Ok(Self {
            db_url: std::env::var("DATABASE_URL")?,
            token_secret: std::env::var("TOKEN_SECRET")?,
            bind_addr: std::env::var("BIND_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:50051".to_string())
                .parse()?,
        })
    }
}
