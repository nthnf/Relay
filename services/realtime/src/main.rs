use realtime::{config::Config, grpc::handler::Handler, redis::RedisStore, store::Store, websocket};
use std::error::Error;
use std::sync::Arc;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::from_env()?;
    let store = Arc::new(Store::new());
    let redis = Arc::new(RedisStore::new(&config.redis_url).await?);
    let handler = Handler::new(store.clone());

    tokio::try_join!(
        websocket::run(config.ws_bind_addr, store, redis),
        async {
            Server::builder()
                .add_service(handler.into_server())
                .serve(config.bind_addr)
                .await
                .map_err(|error| -> Box<dyn Error> { Box::new(error) })
        }
    )?;

    Ok(())
}
