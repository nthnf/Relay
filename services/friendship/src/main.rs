use friendship::{
    config::Config,
    db,
    grpc::{FriendshipServer, client::IdentityClient},
};
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::from_env()?;
    let db = db::connect(&config.db_url).await?;
    let identity = IdentityClient::connect(&config.identity_url).await?;
    let service = FriendshipServer::new(db, identity);

    Server::builder()
        .add_service(service.into_server())
        .serve(config.bind_addr)
        .await?;

    Ok(())
}
