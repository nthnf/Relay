use std::error::Error;

use bootstrap::{amqp, compositor::Compositor, config::Config, db, grpc::BootstrapServer};
use relay_amqp::AmqpSubscriber;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let config = Config::from_env()?;
    let db = db::connect(&config.db_url).await?;

    let grpc = async {
        let (_, health_service) = tonic_health::server::health_reporter();

        Server::builder()
            .add_service(health_service)
            .add_service(BootstrapServer::new(db.clone()).into_server())
            .serve_with_shutdown(config.bind_addr, async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await
            .map_err(|e| -> Box<dyn Error + Send + Sync> { Box::new(e) })
    };

    let identity_amqp = AmqpSubscriber::topic(
        "bootstrap",
        "bootstrap.identity.events",
        "bootstrap-identity-service",
        "relay.events",
        "identity.*",
    )
    .handle(amqp::AmqpHandler::new(db.clone()))
    .run(&config.amqp_addr);

    let friendship_amqp = AmqpSubscriber::topic(
        "bootstrap",
        "bootstrap.friendship.events",
        "bootstrap-friendship-service",
        "relay.events",
        "friendship.*",
    )
    .handle(amqp::AmqpHandler::new(db.clone()))
    .run(&config.amqp_addr);

    let workspace_amqp = AmqpSubscriber::topic(
        "bootstrap",
        "bootstrap.workspace.events",
        "bootstrap-workspace-service",
        "relay.events",
        "workspace.*",
    )
    .handle(amqp::AmqpHandler::new(db.clone()))
    .run(&config.amqp_addr);

    let chat_amqp = AmqpSubscriber::topic(
        "bootstrap",
        "bootstrap.chat.events",
        "bootstrap-chat-service",
        "relay.events",
        "chat.*",
    )
    .handle(amqp::AmqpHandler::new(db.clone()))
    .run(&config.amqp_addr);

    let compositor = Compositor::new(db.clone()).run();

    tokio::try_join!(
        grpc,
        identity_amqp,
        friendship_amqp,
        workspace_amqp,
        chat_amqp,
        compositor
    )?;

    Ok(())
}
