use crate::errors::AppError;
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};

/// Default database file name in the application data directory.
pub const DB_FILENAME: &str = "linuxscp.db";

pub struct Database {
    conn: Connection,
}

impl Database {
    /// Opens an in-memory database for testing.
    pub fn open_in_memory() -> Result<Self, AppError> {
        let conn = Connection::open_in_memory().map_err(|e| {
            AppError::Database(format!("Failed to open in-memory SQLite database: {}", e))
        })?;
        let mut db = Self { conn };
        db.run_migrations()?;
        Ok(db)
    }

    /// Opens or creates the SQLite database at the specified path.
    pub fn open_at(path: &Path) -> Result<Self, AppError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                AppError::Filesystem(format!("Failed to create database directory: {}", e))
            })?;
        }

        let conn = Connection::open(path).map_err(|e| {
            AppError::Database(format!(
                "Failed to open SQLite database at '{}': {}",
                path.display(),
                e
            ))
        })?;

        let mut db = Self { conn };
        db.run_migrations()?;
        Ok(db)
    }

    /// Resolves standard Linux user data directory path for the database (~/.local/share/linuxscp/linuxscp.db).
    pub fn default_db_path() -> PathBuf {
        let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        base.join("linuxscp").join(DB_FILENAME)
    }

    /// Opens the default persistent SQLite database.
    pub fn open_default() -> Result<Self, AppError> {
        Self::open_at(&Self::default_db_path())
    }

    /// Borrows the internal connection.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Borrows the internal connection mutably.
    pub fn connection_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Executes database schema migrations in order within a transaction.
    pub fn run_migrations(&mut self) -> Result<(), AppError> {
        // Create migration tracker table
        self.conn
            .execute(
                "CREATE TABLE IF NOT EXISTS schema_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at_ms INTEGER NOT NULL
                )",
                [],
            )
            .map_err(|e| AppError::Database(format!("Failed to init schema_migrations: {}", e)))?;

        let current_version: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if current_version < 1 {
            self.apply_migration_1()?;
        }

        if current_version < 2 {
            self.apply_migration_2()?;
        }

        if current_version < 3 {
            self.apply_migration_3()?;
        }

        if current_version < 4 {
            self.apply_migration_4()?;
        }

        if current_version < 5 {
            self.apply_migration_5()?;
        }

        Ok(())
    }

    fn apply_migration_1(&mut self) -> Result<(), AppError> {
        let tx = self.conn.transaction().map_err(|e| {
            AppError::Database(format!("Failed to start migration transaction: {}", e))
        })?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS connection_profiles (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                protocol TEXT NOT NULL,
                host TEXT NOT NULL,
                port INTEGER NOT NULL,
                username TEXT NOT NULL,
                auth_method_json TEXT NOT NULL,
                initial_local_directory TEXT,
                initial_remote_directory TEXT,
                sudo_sftp_json TEXT NOT NULL,
                transfer_settings_json TEXT NOT NULL,
                connection_settings_json TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL,
                updated_at_ms INTEGER NOT NULL
            )",
            [],
        )
        .map_err(|e| AppError::Database(format!("Migration 1 table creation failed: {}", e)))?;

        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_connection_profiles_name ON connection_profiles(name)",
            [],
        )
        .map_err(|e| AppError::Database(format!("Migration 1 index creation failed: {}", e)))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        tx.execute(
            "INSERT INTO schema_migrations (version, applied_at_ms) VALUES (1, ?1)",
            [now],
        )
        .map_err(|e| AppError::Database(format!("Failed to record migration 1: {}", e)))?;

        tx.commit()
            .map_err(|e| AppError::Database(format!("Failed to commit migration 1: {}", e)))?;

        Ok(())
    }

    fn apply_migration_2(&mut self) -> Result<(), AppError> {
        let tx = self.conn.transaction().map_err(|e| {
            AppError::Database(format!("Failed to start migration 2 transaction: {}", e))
        })?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS known_hosts (
                host TEXT NOT NULL,
                port INTEGER NOT NULL,
                algorithm TEXT NOT NULL,
                fingerprint_sha256 TEXT NOT NULL,
                public_key_openssh TEXT NOT NULL,
                trusted_at_ms INTEGER NOT NULL,
                PRIMARY KEY (host, port)
            )",
            [],
        )
        .map_err(|e| AppError::Database(format!("Migration 2 table creation failed: {}", e)))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        tx.execute(
            "INSERT INTO schema_migrations (version, applied_at_ms) VALUES (2, ?1)",
            [now],
        )
        .map_err(|e| AppError::Database(format!("Failed to record migration 2: {}", e)))?;

        tx.commit()
            .map_err(|e| AppError::Database(format!("Failed to commit migration 2: {}", e)))?;

        Ok(())
    }

    fn apply_migration_3(&mut self) -> Result<(), AppError> {
        let tx = self.conn.transaction().map_err(|e| {
            AppError::Database(format!("Failed to start migration 3 transaction: {}", e))
        })?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS connection_remote_state (
                profile_id TEXT PRIMARY KEY NOT NULL,
                last_remote_directory TEXT NOT NULL,
                updated_at_ms INTEGER NOT NULL
            )",
            [],
        )
        .map_err(|e| AppError::Database(format!("Migration 3 table creation failed: {}", e)))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        tx.execute(
            "INSERT INTO schema_migrations (version, applied_at_ms) VALUES (3, ?1)",
            [now],
        )
        .map_err(|e| AppError::Database(format!("Failed to record migration 3: {}", e)))?;

        tx.commit()
            .map_err(|e| AppError::Database(format!("Failed to commit migration 3: {}", e)))?;

        Ok(())
    }

    fn apply_migration_4(&mut self) -> Result<(), AppError> {
        let tx = self.conn.transaction().map_err(|e| {
            AppError::Database(format!("Failed to start migration 4 transaction: {}", e))
        })?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS app_preferences (
                key TEXT PRIMARY KEY NOT NULL,
                value_json TEXT NOT NULL,
                updated_at_ms INTEGER NOT NULL
            )",
            [],
        )
        .map_err(|e| AppError::Database(format!("Migration 4 table creation failed: {}", e)))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        tx.execute(
            "INSERT INTO schema_migrations (version, applied_at_ms) VALUES (4, ?1)",
            [now],
        )
        .map_err(|e| AppError::Database(format!("Failed to record migration 4: {}", e)))?;

        tx.commit()
            .map_err(|e| AppError::Database(format!("Failed to commit migration 4: {}", e)))?;

        Ok(())
    }

    fn apply_migration_5(&mut self) -> Result<(), AppError> {
        let tx = self.conn.transaction().map_err(|e| {
            AppError::Database(format!("Failed to start migration 5 transaction: {}", e))
        })?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS file_app_associations (
                extension TEXT PRIMARY KEY NOT NULL,
                desktop_app_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at_ms INTEGER NOT NULL
            )",
            [],
        )
        .map_err(|e| AppError::Database(format!("Migration 5 table creation failed: {}", e)))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        tx.execute(
            "INSERT INTO schema_migrations (version, applied_at_ms) VALUES (5, ?1)",
            [now],
        )
        .map_err(|e| AppError::Database(format!("Failed to record migration 5: {}", e)))?;

        tx.commit()
            .map_err(|e| AppError::Database(format!("Failed to commit migration 5: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrations_apply_cleanly_in_memory() {
        let db = Database::open_in_memory().expect("In-memory database should initialize");
        let version: i64 = db
            .connection()
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| {
                r.get(0)
            })
            .expect("Should query version");
        assert_eq!(version, 5);

        // Verify table existence
        let count: i64 = db
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='connection_profiles'",
                [],
                |r| r.get(0),
            )
            .expect("Should query tables");
        assert_eq!(count, 1);

        let pref_count: i64 = db
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='app_preferences'",
                [],
                |r| r.get(0),
            )
            .expect("Should query app_preferences table");
        assert_eq!(pref_count, 1);

        let assoc_count: i64 = db
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='file_app_associations'",
                [],
                |r| r.get(0),
            )
            .expect("Should query file_app_associations table");
        assert_eq!(assoc_count, 1);
    }

    #[test]
    fn test_migrations_are_idempotent() {
        let mut db = Database::open_in_memory().expect("In-memory database should initialize");
        // Running migrations again should succeed without error
        assert!(db.run_migrations().is_ok());
    }
}
