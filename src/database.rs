use std::path::Path;

use nix::sys::statfs::{
    BTRFS_SUPER_MAGIC, EXT4_SUPER_MAGIC, FsType, OVERLAYFS_SUPER_MAGIC, TMPFS_MAGIC,
    XFS_SUPER_MAGIC, statfs,
};
use sqlx::ConnectOptions;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};

pub async fn setup_database(
    db_uri: &str,
    slow_query_threshold: std::time::Duration,
) -> sqlx::SqlitePool {
    let options = db_uri
        .parse::<SqliteConnectOptions>()
        .expect("Invalid DATABASE_URL");

    verify_sqlite_filesystem(options.get_filename())
        .expect("DATABASE_URL must point to a supported local filesystem");

    let options = options
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .pragma("cache_size", "-20000")
        .pragma("temp_store", "MEMORY")
        .pragma("busy_timeout", "5000")
        .pragma("auto_vacuum", "INCREMENTAL")
        .log_slow_statements(log::LevelFilter::Warn, slow_query_threshold);

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

fn verify_sqlite_filesystem(database_path: &Path) -> Result<(), String> {
    if database_path == Path::new(":memory:") || database_path.as_os_str().is_empty() {
        return Ok(());
    }

    let probe_path = if database_path.exists() {
        database_path
    } else {
        database_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
    };
    let filesystem = statfs(probe_path).map_err(|error| {
        format!(
            "cannot inspect filesystem for '{}': {error}",
            database_path.display()
        )
    })?;
    let filesystem_type = filesystem.filesystem_type();
    if supported_local_filesystem(filesystem_type) {
        return Ok(());
    }

    Err(format!(
        "SQLite database '{}' is on unsupported filesystem type {filesystem_type:?}; \
         only ext4, XFS, Btrfs, tmpfs and OverlayFS are accepted",
        database_path.display()
    ))
}

fn supported_local_filesystem(filesystem_type: FsType) -> bool {
    matches!(
        filesystem_type,
        EXT4_SUPER_MAGIC
            | XFS_SUPER_MAGIC
            | BTRFS_SUPER_MAGIC
            | TMPFS_MAGIC
            | OVERLAYFS_SUPER_MAGIC
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use nix::sys::statfs::{FUSE_SUPER_MAGIC, NFS_SUPER_MAGIC, OVERLAYFS_SUPER_MAGIC};

    use super::{supported_local_filesystem, verify_sqlite_filesystem};

    #[test]
    fn accepts_memory_and_known_local_filesystems() {
        assert!(verify_sqlite_filesystem(Path::new(":memory:")).is_ok());
        assert!(verify_sqlite_filesystem(Path::new("new-local-database.db")).is_ok());
        assert!(supported_local_filesystem(OVERLAYFS_SUPER_MAGIC));
    }

    #[test]
    fn rejects_network_filesystems_and_unknown_types() {
        assert!(!supported_local_filesystem(NFS_SUPER_MAGIC));
        assert!(!supported_local_filesystem(FUSE_SUPER_MAGIC));
        assert!(!supported_local_filesystem(nix::sys::statfs::FsType(0)));
    }
}
