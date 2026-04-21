use chat::{amqp, config::Config, db};
use relay_amqp::AmqpSubscriber;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::from_env()?;
    let db = db::connect(&config.db_url).await?;
    let handler = Arc::new(amqp::AmqpHandler::new(db));

    let identity_amqp = AmqpSubscriber::topic(
        "chat",
        "chat.identity.events",
        "chat-identity-service",
        "relay.events",
        "identity.*",
    )
    .handle(handler.as_ref().clone())
    .run(&config.amqp_addr);

    let workspace_amqp = AmqpSubscriber::topic(
        "chat",
        "chat.workspace.events",
        "chat-workspace-service",
        "relay.events",
        "workspace.*",
    )
    .handle(handler.as_ref().clone())
    .run(&config.amqp_addr);

    tokio::try_join!(identity_amqp, workspace_amqp)?;

    Ok(())
}
