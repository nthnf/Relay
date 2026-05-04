use sea_orm::{Database, DatabaseConnection, DbErr};
use std::time::Duration;
use tokio::time::sleep;
use tracing::warn;

const CONNECT_RETRY_DELAY: Duration = Duration::from_secs(2);
const CONNECT_MAX_RETRIES: usize = 30;

pub async fn connect(database_url: &str) -> Result<DatabaseConnection, DbErr> {
    let mut last_error = None;

    for attempt in 1..=CONNECT_MAX_RETRIES {
        match Database::connect(database_url).await {
            Ok(connection) => return Ok(connection),
            Err(error) => {
                warn!(attempt, error = %error, "database connection failed; retrying");
                last_error = Some(error);
                sleep(CONNECT_RETRY_DELAY).await;
            }
        }
    }

    Err(last_error.expect("database connection should have been attempted"))
}
