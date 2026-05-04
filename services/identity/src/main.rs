use identity::{auth::AuthKeys, config::Config, db, grpc::IdentityServer};
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let db = db::connect(&config.db_url).await?;
    let auth = AuthKeys::from_shared_secret(config.token_secret.as_bytes());
    let service = IdentityServer::new(db, auth);
    let auth_service = service.clone();
    let (_, health_service) = tonic_health::server::health_reporter();

    Server::builder()
        .add_service(health_service)
        .add_service(service.into_server())
        .add_service(auth_service.into_auth_server())
        .serve(config.bind_addr)
        .await?;

    Ok(())
}
