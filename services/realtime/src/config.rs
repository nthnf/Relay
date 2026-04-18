use std::{error::Error, net::SocketAddr};

pub struct Config {
    pub redis_url: String,
    pub ws_bind_addr: SocketAddr,
    pub bind_addr: SocketAddr,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn Error>> {
        dotenvy::dotenv().ok();

        Ok(Self {
            redis_url: std::env::var("REDIS_URL")?,
            ws_bind_addr: std::env::var("WS_BIND_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
                .parse()?,
            bind_addr: std::env::var("GRPC_BIND_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:50051".to_string())
                .parse()?,
        })
    }
}
