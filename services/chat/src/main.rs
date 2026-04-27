use chat::{
    amqp, config::Config, db,
    grpc::{ChatServer, clients::Clients},
};
use relay_amqp::AmqpSubscriber;
use std::sync::Arc;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::from_env()?;
    let db = db::connect(&config.db_url).await?;
    let clients = Clients::connect(&config).await?;
    let amqp_handler = Arc::new(amqp::AmqpHandler::new(db.clone()));

    let grpc = async {
        Server::builder()
            .add_service(ChatServer::new(db.clone(), clients).into_server())
            .serve_with_shutdown(config.bind_addr, async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })
    };

    let identity_amqp = AmqpSubscriber::topic(
        "chat",
        "chat.identity.events",
        "chat-identity-service",
        "relay.events",
        "identity.*",
    )
    .handle(amqp_handler.as_ref().clone())
    .run(&config.amqp_addr);

    let workspace_amqp = AmqpSubscriber::topic(
        "chat",
        "chat.workspace.events",
        "chat-workspace-service",
        "relay.events",
        "workspace.*",
    )
    .handle(amqp_handler.as_ref().clone())
    .run(&config.amqp_addr);

    tokio::try_join!(grpc, identity_amqp, workspace_amqp)?;

    Ok(())
}
