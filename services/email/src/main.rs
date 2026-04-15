use email::{amqp::handler::Handler, config::Config, db, smtp::SmtpClient};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let db = db::connect(&config.db_url).await?;
    let smtp = SmtpClient::new(
        config.smtp_url.clone(),
        config.sender_email.clone(),
        config.sender_name.clone(),
    );
    let handler = Handler::new(
        db,
        config.public_web_base_url.clone(),
        config.smtp_provider_name.clone(),
        smtp,
    );

    Arc::new(handler).run(config.amqp_addr.clone()).await
}
