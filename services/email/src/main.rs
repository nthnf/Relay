use email::{amqp, config::Config, db, smtp::SmtpClient};
use relay_amqp::AmqpSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::from_env()?;
    let db = db::connect(&config.db_url).await?;
    let smtp = SmtpClient::new(
        config.smtp_url.clone(),
        config.sender_email.clone(),
        config.sender_name.clone(),
    );
    let handler = amqp::Handler::new(
        db,
        config.public_web_base_url.clone(),
        config.smtp_provider_name.clone(),
        smtp,
    );

    AmqpSubscriber::queue("email", "email.events", "email-service")
        .handle(handler)
        .run(&config.amqp_addr)
        .await
}
