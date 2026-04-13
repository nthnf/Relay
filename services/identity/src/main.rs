use identity::{auth::AuthKeys, config::Config, db, grpc::IdentityServer};
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let db = db::connect(&config.db_url).await?;
    let auth = AuthKeys::from_shared_secret(config.token_secret.as_bytes());
    let service = IdentityServer::new(db, auth);

    Server::builder()
        .add_service(service.into_server())
        .serve(config.bind_addr)
        .await?;

    Ok(())
}
