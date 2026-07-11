use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};

pub async fn setup_database(db_uri: &str) -> sqlx::SqlitePool {
    let options = db_uri
        .parse::<SqliteConnectOptions>()
        .expect("Invalid DATABASE_URL")
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .pragma("cache_size", "-20000")
        .pragma("temp_store", "MEMORY")
        .pragma("busy_timeout", "5000")
        .pragma("auto_vacuum", "INCREMENTAL");

    let pool = SqlitePoolOptions::new()
        // WAL allows concurrent readers; 1 writer serializes anyway.
        // Matching VU count avoids hidden acquire queuing.
        .max_connections(20)
        .min_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect_with(options)
        .await
        .expect("Unable to open database");

    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Failed to run database migrations");

    pool
}
