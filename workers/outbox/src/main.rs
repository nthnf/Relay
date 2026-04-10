use outbox::{config::Config, worker};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    worker::run(Config::from_env()?).await
}
