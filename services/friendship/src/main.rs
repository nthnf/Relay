use friendship::{amqp, config::Config, db, grpc::FriendshipServer};
use std::error::Error;
use std::sync::Arc;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let config = Config::from_env()?;
    let db = db::connect(&config.db_url).await?;
    let grpc = async {
        Server::builder()
            .add_service(FriendshipServer::new(db.clone()).into_server())
            .serve_with_shutdown(config.bind_addr, async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })
    };

    let amqp = amqp::run(
        Arc::new(amqp::AmqpHandler::new(db.clone())),
        config.amqp_addr.clone(),
    );

    tokio::try_join!(grpc, amqp)?;

    Ok(())
}
