use crate::errors::AppError;
use crate::persistence::database::Database;
use rusqlite::{params, OptionalExtension};

/// Repository for persisting per-connection remote browsing state (such as the last visited remote directory).
pub struct RemoteStateRepository<'a> {
    db: &'a Database,
}

impl<'a> RemoteStateRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Retrieves the last visited remote directory for a saved connection profile.
    pub fn get_last_remote_directory(&self, profile_id: &str) -> Result<Option<String>, AppError> {
        let conn = self.db.connection();
        conn.query_row(
            "SELECT last_remote_directory FROM connection_remote_state WHERE profile_id = ?1",
            [profile_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| AppError::Database(format!("Failed to query last remote directory: {}", e)))
    }

    /// Persists the last visited remote directory for a saved connection profile.
    pub fn set_last_remote_directory(&self, profile_id: &str, path: &str) -> Result<(), AppError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let conn = self.db.connection();
        conn.execute(
            "INSERT OR REPLACE INTO connection_remote_state (profile_id, last_remote_directory, updated_at_ms)
             VALUES (?1, ?2, ?3)",
            params![profile_id, path, now],
        )
        .map_err(|e| AppError::Database(format!("Failed to save last remote directory: {}", e)))?;
        Ok(())
    }

    /// Removes persisted remote browsing state for a profile.
    pub fn delete_last_remote_directory(&self, profile_id: &str) -> Result<(), AppError> {
        let conn = self.db.connection();
        conn.execute(
            "DELETE FROM connection_remote_state WHERE profile_id = ?1",
            [profile_id],
        )
        .map_err(|e| {
            AppError::Database(format!("Failed to delete last remote directory: {}", e))
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_last_remote_directory_persistence() {
        let db = Database::open_in_memory().expect("Should init db");
        let repo = RemoteStateRepository::new(&db);

        assert_eq!(repo.get_last_remote_directory("prof-1").unwrap(), None);

        repo.set_last_remote_directory("prof-1", "/var/www/html")
            .unwrap();
        assert_eq!(
            repo.get_last_remote_directory("prof-1").unwrap(),
            Some("/var/www/html".to_string())
        );

        // Overwrite updates to latest directory
        repo.set_last_remote_directory("prof-1", "/home/ubuntu/日本語 folder")
            .unwrap();
        assert_eq!(
            repo.get_last_remote_directory("prof-1").unwrap(),
            Some("/home/ubuntu/日本語 folder".to_string())
        );

        // Independent per profile
        assert_eq!(repo.get_last_remote_directory("prof-2").unwrap(), None);

        repo.delete_last_remote_directory("prof-1").unwrap();
        assert_eq!(repo.get_last_remote_directory("prof-1").unwrap(), None);
    }
}
