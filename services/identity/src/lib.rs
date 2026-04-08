pub mod auth;
pub mod db;
pub mod entity;
pub mod grpc;

use std::{env, error::Error, net::SocketAddr};

use tonic::transport::Server;

pub async fn run() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();

    let db = db::connection::connect().await?;
    let auth = auth::AuthKeys::from_shared_secret(env::var("TOKEN_SECRET")?.as_bytes());
    let addr: SocketAddr = env::var("BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50051".to_string())
        .parse()?;

    let service = grpc::identity::IdentityServer::new(db, auth);

    Server::builder()
        .add_service(service.into_server())
        .serve(addr)
        .await?;

    Ok(())
}
