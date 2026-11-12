use crate::errors::AppError;
use crate::models::{
    AuthMethod, ConnectionProfile, ProfileConnectionSettings, ProfileTransferSettings,
    ProtocolType, SudoSftpConfig,
};
use crate::persistence::database::Database;
use rusqlite::params;
use uuid::Uuid;

pub struct ProfileRepository<'a> {
    db: &'a Database,
}

impl<'a> ProfileRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Lists all saved connection profiles ordered alphabetically by name.
    pub fn list_profiles(&self) -> Result<Vec<ConnectionProfile>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, protocol, host, port, username, auth_method_json,
                        initial_local_directory, initial_remote_directory,
                        sudo_sftp_json, transfer_settings_json, connection_settings_json,
                        created_at_ms, updated_at_ms
                 FROM connection_profiles
                 ORDER BY name COLLATE NOCASE ASC",
            )
            .map_err(|e| AppError::Database(format!("Failed to prepare query: {}", e)))?;

        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let protocol_str: String = row.get(2)?;
                let host: String = row.get(3)?;
                let port: u16 = row.get(4)?;
                let username: String = row.get(5)?;
                let auth_json: String = row.get(6)?;
                let initial_local: Option<String> = row.get(7)?;
                let initial_remote: Option<String> = row.get(8)?;
                let sudo_json: String = row.get(9)?;
                let transfer_json: String = row.get(10)?;
                let conn_json: String = row.get(11)?;
                let created_at_ms: i64 = row.get(12)?;
                let updated_at_ms: i64 = row.get(13)?;

                Ok((
                    id,
                    name,
                    protocol_str,
                    host,
                    port,
                    username,
                    auth_json,
                    initial_local,
                    initial_remote,
                    sudo_json,
                    transfer_json,
                    conn_json,
                    created_at_ms as u64,
                    updated_at_ms as u64,
                ))
            })
            .map_err(|e| AppError::Database(format!("Failed to query profiles: {}", e)))?;

        let mut profiles = Vec::new();
        for item in rows {
            let (
                id,
                name,
                protocol_str,
                host,
                port,
                username,
                auth_json,
                initial_local_directory,
                initial_remote_directory,
                sudo_json,
                transfer_json,
                conn_json,
                created_at_ms,
                updated_at_ms,
            ) =
                item.map_err(|e| AppError::Database(format!("Error reading profile row: {}", e)))?;

            let protocol: ProtocolType = serde_json::from_str(&format!("\"{}\"", protocol_str))
                .unwrap_or(ProtocolType::Sftp);
            let auth_method: AuthMethod = serde_json::from_str(&auth_json)
                .map_err(|e| AppError::Database(format!("Invalid auth_method JSON: {}", e)))?;
            let sudo_sftp: SudoSftpConfig = serde_json::from_str(&sudo_json)
                .map_err(|e| AppError::Database(format!("Invalid sudo_sftp JSON: {}", e)))?;
            let transfer_settings: ProfileTransferSettings = serde_json::from_str(&transfer_json)
                .map_err(|e| {
                AppError::Database(format!("Invalid transfer_settings JSON: {}", e))
            })?;
            let connection_settings: ProfileConnectionSettings = serde_json::from_str(&conn_json)
                .map_err(|e| {
                AppError::Database(format!("Invalid connection_settings JSON: {}", e))
            })?;

            profiles.push(ConnectionProfile {
                id,
                name,
                protocol,
                host,
                port,
                username,
                auth_method,
                initial_local_directory,
                initial_remote_directory,
                sudo_sftp,
                transfer_settings,
                connection_settings,
                created_at_ms,
                updated_at_ms,
            });
        }

        Ok(profiles)
    }

    /// Retrieves a single connection profile by id.
    pub fn get_profile(&self, id: &str) -> Result<Option<ConnectionProfile>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, protocol, host, port, username, auth_method_json,
                        initial_local_directory, initial_remote_directory,
                        sudo_sftp_json, transfer_settings_json, connection_settings_json,
                        created_at_ms, updated_at_ms
                 FROM connection_profiles
                 WHERE id = ?1",
            )
            .map_err(|e| AppError::Database(format!("Failed to prepare query: {}", e)))?;

        let mut rows = stmt
            .query_map([id], |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let protocol_str: String = row.get(2)?;
                let host: String = row.get(3)?;
                let port: u16 = row.get(4)?;
                let username: String = row.get(5)?;
                let auth_json: String = row.get(6)?;
                let initial_local: Option<String> = row.get(7)?;
                let initial_remote: Option<String> = row.get(8)?;
                let sudo_json: String = row.get(9)?;
                let transfer_json: String = row.get(10)?;
                let conn_json: String = row.get(11)?;
                let created_at_ms: i64 = row.get(12)?;
                let updated_at_ms: i64 = row.get(13)?;

                Ok((
                    id,
                    name,
                    protocol_str,
                    host,
                    port,
                    username,
                    auth_json,
                    initial_local,
                    initial_remote,
                    sudo_json,
                    transfer_json,
                    conn_json,
                    created_at_ms as u64,
                    updated_at_ms as u64,
                ))
            })
            .map_err(|e| AppError::Database(format!("Failed to query profile: {}", e)))?;

        if let Some(item) = rows.next() {
            let (
                id,
                name,
                protocol_str,
                host,
                port,
                username,
                auth_json,
                initial_local_directory,
                initial_remote_directory,
                sudo_json,
                transfer_json,
                conn_json,
                created_at_ms,
                updated_at_ms,
            ) =
                item.map_err(|e| AppError::Database(format!("Error reading profile row: {}", e)))?;

            let protocol: ProtocolType = serde_json::from_str(&format!("\"{}\"", protocol_str))
                .unwrap_or(ProtocolType::Sftp);
            let auth_method: AuthMethod = serde_json::from_str(&auth_json)
                .map_err(|e| AppError::Database(format!("Invalid auth_method JSON: {}", e)))?;
            let sudo_sftp: SudoSftpConfig = serde_json::from_str(&sudo_json)
                .map_err(|e| AppError::Database(format!("Invalid sudo_sftp JSON: {}", e)))?;
            let transfer_settings: ProfileTransferSettings = serde_json::from_str(&transfer_json)
                .map_err(|e| {
                AppError::Database(format!("Invalid transfer_settings JSON: {}", e))
            })?;
            let connection_settings: ProfileConnectionSettings = serde_json::from_str(&conn_json)
                .map_err(|e| {
                AppError::Database(format!("Invalid connection_settings JSON: {}", e))
            })?;

            Ok(Some(ConnectionProfile {
                id,
                name,
                protocol,
                host,
                port,
                username,
                auth_method,
                initial_local_directory,
                initial_remote_directory,
                sudo_sftp,
                transfer_settings,
                connection_settings,
                created_at_ms,
                updated_at_ms,
            }))
        } else {
            Ok(None)
        }
    }

    /// Saves or updates a connection profile in the SQLite database.
    pub fn save_profile(&self, profile: &ConnectionProfile) -> Result<(), AppError> {
        profile.validate()?;

        let auth_json = serde_json::to_string(&profile.auth_method)
            .map_err(|e| AppError::Database(format!("Failed to serialize auth_method: {}", e)))?;
        let sudo_json = serde_json::to_string(&profile.sudo_sftp)
            .map_err(|e| AppError::Database(format!("Failed to serialize sudo_sftp: {}", e)))?;
        let transfer_json = serde_json::to_string(&profile.transfer_settings).map_err(|e| {
            AppError::Database(format!("Failed to serialize transfer_settings: {}", e))
        })?;
        let conn_json = serde_json::to_string(&profile.connection_settings).map_err(|e| {
            AppError::Database(format!("Failed to serialize connection_settings: {}", e))
        })?;

        let protocol_str = profile.protocol.as_str();

        let conn = self.db.connection();
        conn.execute(
            "INSERT OR REPLACE INTO connection_profiles (
                id, name, protocol, host, port, username, auth_method_json,
                initial_local_directory, initial_remote_directory,
                sudo_sftp_json, transfer_settings_json, connection_settings_json,
                created_at_ms, updated_at_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                profile.id,
                profile.name,
                protocol_str,
                profile.host,
                profile.port,
                profile.username,
                auth_json,
                profile.initial_local_directory,
                profile.initial_remote_directory,
                sudo_json,
                transfer_json,
                conn_json,
                profile.created_at_ms as i64,
                profile.updated_at_ms as i64,
            ],
        )
        .map_err(|e| AppError::Database(format!("Failed to insert profile: {}", e)))?;

        Ok(())
    }

    /// Deletes a connection profile by id.
    pub fn delete_profile(&self, id: &str) -> Result<(), AppError> {
        let conn = self.db.connection();
        conn.execute("DELETE FROM connection_profiles WHERE id = ?1", [id])
            .map_err(|e| AppError::Database(format!("Failed to delete profile: {}", e)))?;
        let _ = conn.execute(
            "DELETE FROM connection_remote_state WHERE profile_id = ?1",
            [id],
        );
        Ok(())
    }

    /// Duplicates an existing profile under a new ID and name.
    pub fn duplicate_profile(
        &self,
        source_id: &str,
        new_name: &str,
    ) -> Result<ConnectionProfile, AppError> {
        let existing = self
            .get_profile(source_id)?
            .ok_or_else(|| AppError::Database("Profile to duplicate not found".to_string()))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let mut duplicated = existing;
        duplicated.id = Uuid::new_v4().to_string();
        duplicated.name = new_name.to_string();
        duplicated.created_at_ms = now;
        duplicated.updated_at_ms = now;

        self.save_profile(&duplicated)?;
        Ok(duplicated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_crud_lifecycle() {
        let db = Database::open_in_memory().expect("Should init db");
        let repo = ProfileRepository::new(&db);

        // 1. Initial list empty
        assert_eq!(repo.list_profiles().unwrap().len(), 0);

        // 2. Create profile
        let mut profile = ConnectionProfile::new(
            "My Dev Server".to_string(),
            ProtocolType::Sftp,
            "10.0.0.1".to_string(),
            "developer".to_string(),
            AuthMethod::Password,
        );
        profile.initial_remote_directory = Some("/var/www".to_string());

        repo.save_profile(&profile).expect("Save should succeed");

        // 3. Retrieve
        let retrieved = repo
            .get_profile(&profile.id)
            .unwrap()
            .expect("Should find profile");
        assert_eq!(retrieved.name, "My Dev Server");
        assert_eq!(retrieved.host, "10.0.0.1");
        assert_eq!(
            retrieved.initial_remote_directory,
            Some("/var/www".to_string())
        );

        // 4. Duplicate
        let duplicated = repo
            .duplicate_profile(&profile.id, "My Dev Server (Copy)")
            .expect("Duplicate should succeed");
        assert_ne!(duplicated.id, profile.id);
        assert_eq!(duplicated.name, "My Dev Server (Copy)");

        // 5. List
        let list = repo.list_profiles().unwrap();
        assert_eq!(list.len(), 2);

        // 6. Delete
        repo.delete_profile(&profile.id)
            .expect("Delete should succeed");
        assert_eq!(repo.list_profiles().unwrap().len(), 1);
        assert!(repo.get_profile(&profile.id).unwrap().is_none());
    }
}
