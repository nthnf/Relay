use std::error::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub db_url: String,
    pub amqp_addr: String,
    pub public_web_base_url: String,
    pub smtp_url: String,
    pub smtp_provider_name: String,
    pub sender_email: String,
    pub sender_name: String,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn Error + Send + Sync>> {
        dotenvy::dotenv().ok();
        Ok(Self {
            db_url: std::env::var("DATABASE_URL")?,
            amqp_addr: std::env::var("AMQP_ADDR")?,
            public_web_base_url: std::env::var("PUBLIC_WEB_BASE_URL")?,
            smtp_url: std::env::var("SMTP_URL")?,
            smtp_provider_name: std::env::var("SMTP_PROVIDER_NAME")?,
            sender_email: std::env::var("SENDER_EMAIL")?,
            sender_name: std::env::var("SENDER_NAME")?,
        })
    }
}
