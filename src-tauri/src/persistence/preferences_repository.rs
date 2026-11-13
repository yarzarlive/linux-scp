use crate::errors::AppError;
use crate::persistence::database::Database;
use crate::remote_edit::RemoteEditUploadMode;
use crate::terminal::TerminalPreference;
use rusqlite::params;

pub const KEY_TERMINAL_PREFERENCE: &str = "terminal_preference";
pub const KEY_REMOTE_EDIT_UPLOAD_MODE: &str = "remote_edit_upload_mode";

pub struct PreferencesRepository<'a> {
    db: &'a Database,
}

impl<'a> PreferencesRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Loads the saved `TerminalPreference` from SQLite, returning `TerminalPreference::default()` if none is stored.
    pub fn get_terminal_preference(&self) -> Result<TerminalPreference, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare("SELECT value_json FROM app_preferences WHERE key = ?1")
            .map_err(|e| {
                AppError::Database(format!("Failed to prepare preference query: {}", e))
            })?;

        let mut rows = stmt
            .query_map([KEY_TERMINAL_PREFERENCE], |row| row.get::<_, String>(0))
            .map_err(|e| AppError::Database(format!("Failed to query preference: {}", e)))?;

        if let Some(row) = rows.next() {
            let json = row
                .map_err(|e| AppError::Database(format!("Error reading preference row: {}", e)))?;
            let pref: TerminalPreference = serde_json::from_str(&json).map_err(|e| {
                AppError::Database(format!("Invalid terminal preference JSON: {}", e))
            })?;
            Ok(pref)
        } else {
            Ok(TerminalPreference::default())
        }
    }

    /// Validates and persists the `TerminalPreference` into SQLite.
    pub fn set_terminal_preference(&self, preference: &TerminalPreference) -> Result<(), AppError> {
        preference.validate()?;

        let json = serde_json::to_string(preference).map_err(|e| {
            AppError::Database(format!("Failed to serialize terminal preference: {}", e))
        })?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        self.db
            .connection()
            .execute(
                "INSERT OR REPLACE INTO app_preferences (key, value_json, updated_at_ms)
                 VALUES (?1, ?2, ?3)",
                params![KEY_TERMINAL_PREFERENCE, json, now],
            )
            .map_err(|e| {
                AppError::Database(format!("Failed to save terminal preference: {}", e))
            })?;

        Ok(())
    }

    /// Loads the saved `RemoteEditUploadMode` from SQLite, defaulting to `RemoteEditUploadMode::Auto`.
    pub fn get_remote_edit_upload_mode(&self) -> Result<RemoteEditUploadMode, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare("SELECT value_json FROM app_preferences WHERE key = ?1")
            .map_err(|e| {
                AppError::Database(format!("Failed to prepare preference query: {}", e))
            })?;

        let mut rows = stmt
            .query_map([KEY_REMOTE_EDIT_UPLOAD_MODE], |row| row.get::<_, String>(0))
            .map_err(|e| AppError::Database(format!("Failed to query preference: {}", e)))?;

        if let Some(row) = rows.next() {
            let json = row
                .map_err(|e| AppError::Database(format!("Error reading preference row: {}", e)))?;
            let mode: RemoteEditUploadMode = serde_json::from_str(&json).map_err(|e| {
                AppError::Database(format!("Invalid remote edit upload mode JSON: {}", e))
            })?;
            Ok(mode)
        } else {
            Ok(RemoteEditUploadMode::default())
        }
    }

    /// Persists the `RemoteEditUploadMode` into SQLite.
    pub fn set_remote_edit_upload_mode(&self, mode: RemoteEditUploadMode) -> Result<(), AppError> {
        let json = serde_json::to_string(&mode).map_err(|e| {
            AppError::Database(format!(
                "Failed to serialize remote edit upload mode: {}",
                e
            ))
        })?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        self.db
            .connection()
            .execute(
                "INSERT OR REPLACE INTO app_preferences (key, value_json, updated_at_ms)
                 VALUES (?1, ?2, ?3)",
                params![KEY_REMOTE_EDIT_UPLOAD_MODE, json, now],
            )
            .map_err(|e| {
                AppError::Database(format!("Failed to save remote edit upload mode: {}", e))
            })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::TerminalPreferenceMode;

    #[test]
    fn test_terminal_preference_persistence_roundtrip() {
        let db = Database::open_in_memory().expect("Should init in-memory DB");
        let repo = PreferencesRepository::new(&db);

        // Default when unset
        let initial = repo.get_terminal_preference().unwrap();
        assert_eq!(initial.mode, TerminalPreferenceMode::Auto);

        // Save preset preference
        let preset_pref = TerminalPreference {
            mode: TerminalPreferenceMode::Preset,
            preset_binary: Some("gnome-terminal".to_string()),
            custom_executable: None,
            custom_args_prefix: vec![],
        };
        repo.set_terminal_preference(&preset_pref)
            .expect("Should save preset preference");

        let loaded = repo.get_terminal_preference().unwrap();
        assert_eq!(loaded, preset_pref);

        // Save custom preference
        let custom_pref = TerminalPreference {
            mode: TerminalPreferenceMode::Custom,
            preset_binary: None,
            custom_executable: Some("/usr/bin/xterm".to_string()),
            custom_args_prefix: vec!["-e".to_string()],
        };
        repo.set_terminal_preference(&custom_pref)
            .expect("Should save custom preference");

        let loaded_custom = repo.get_terminal_preference().unwrap();
        assert_eq!(loaded_custom, custom_pref);
    }
}
