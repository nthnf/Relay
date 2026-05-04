use realtime::{
    config::Config, grpc::handler::Handler, redis::RedisStore, store::Store, websocket,
};
use std::error::Error;
use std::sync::Arc;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::from_env()?;
    let store = Arc::new(Store::new());
    let redis = Arc::new(RedisStore::new(&config.redis_url).await?);
    let handler = Handler::with_redis(store.clone(), redis.clone());

    tokio::try_join!(websocket::run(config.ws_bind_addr, store, redis), async {
        let (_, health_service) = tonic_health::server::health_reporter();

        Server::builder()
            .add_service(health_service)
            .add_service(handler.into_server())
            .serve(config.bind_addr)
            .await
            .map_err(|error| -> Box<dyn Error> { Box::new(error) })
    })?;

    Ok(())
}
