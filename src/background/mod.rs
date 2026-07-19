use sqlx::pool::Pool;
use sqlx::sqlite::Sqlite;
use std::time::Duration;
use tokio::task::JoinHandle;

/// Spawns generic background workers. The returned handle lets
/// `main` abort the tasks on shutdown.
pub fn start(_pool: Pool<Sqlite>) -> JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!("background worker started");
        let mut interval = tokio::time::interval(Duration::from_secs(3600));
        loop {
            interval.tick().await;
            // Place periodic cleanup/telemetry tasks here.
            tracing::debug!("background worker tick");
        }
    })
}
