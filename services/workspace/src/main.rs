use std::error::Error;

use relay_amqp::AmqpSubscriber;
use tonic::transport::Server;
use workspace::{amqp, config::Config, db, grpc::WorkspaceServer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let config = Config::from_env()?;
    let db = db::connect(&config.db_url).await?;

    let grpc = async {
        let (_, health_service) = tonic_health::server::health_reporter();

        Server::builder()
            .add_service(health_service)
            .add_service(WorkspaceServer::new(db.clone()).into_server())
            .serve_with_shutdown(config.bind_addr, async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })
    };

    let amqp = AmqpSubscriber::topic(
        "workspace",
        "workspace.events",
        "workspace-service",
        "relay.events",
        "identity.*",
    )
    .handle(amqp::AmqpHandler::new(db.clone()))
    .run(&config.amqp_addr);

    tokio::try_join!(grpc, amqp)?;

    Ok(())
}
