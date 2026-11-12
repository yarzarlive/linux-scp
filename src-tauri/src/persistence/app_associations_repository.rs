use crate::desktop_integration::{normalize_extension_input, FileAppAssociation};
use crate::errors::AppError;
use crate::persistence::database::Database;
use rusqlite::params;

pub struct AppAssociationsRepository<'a> {
    db: &'a Database,
}

impl<'a> AppAssociationsRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Retrieves the saved default desktop application association for a file extension.
    /// Automatically normalizes the extension key case-insensitively.
    pub fn get_association(
        &self,
        raw_extension: &str,
    ) -> Result<Option<FileAppAssociation>, AppError> {
        let ext = normalize_extension_input(raw_extension);
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "SELECT extension, desktop_app_id, created_at, updated_at_ms
                 FROM file_app_associations
                 WHERE extension = ?1",
            )
            .map_err(|e| {
                AppError::Database(format!("Failed to prepare association query: {}", e))
            })?;

        let mut rows = stmt
            .query_map([&ext], |row| {
                Ok(FileAppAssociation {
                    extension: row.get(0)?,
                    desktop_app_id: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at_ms: row.get::<_, i64>(3)? as u64,
                })
            })
            .map_err(|e| AppError::Database(format!("Failed to query association: {}", e)))?;

        if let Some(row) = rows.next() {
            let assoc = row
                .map_err(|e| AppError::Database(format!("Error reading association row: {}", e)))?;
            Ok(Some(assoc))
        } else {
            Ok(None)
        }
    }

    /// Saves or updates the LinuxSCP-scoped default desktop application for a file extension.
    /// Never modifies system-wide MIME configuration files (`mimeapps.list`).
    pub fn set_association(
        &self,
        raw_extension: &str,
        desktop_app_id: &str,
    ) -> Result<FileAppAssociation, AppError> {
        let ext = normalize_extension_input(raw_extension);
        let app_id = desktop_app_id.trim();
        if app_id.is_empty() {
            return Err(AppError::Validation(
                "Desktop application ID cannot be empty.".to_string(),
            ));
        }

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let created_at = format!("{}", now_ms);

        let assoc = FileAppAssociation {
            extension: ext,
            desktop_app_id: app_id.to_string(),
            created_at,
            updated_at_ms: now_ms,
        };

        self.db
            .connection()
            .execute(
                "INSERT OR REPLACE INTO file_app_associations (extension, desktop_app_id, created_at, updated_at_ms)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    assoc.extension,
                    assoc.desktop_app_id,
                    assoc.created_at,
                    assoc.updated_at_ms as i64
                ],
            )
            .map_err(|e| AppError::Database(format!("Failed to save file association: {}", e)))?;

        Ok(assoc)
    }

    /// Removes a saved default application association for an extension.
    pub fn delete_association(&self, raw_extension: &str) -> Result<(), AppError> {
        let ext = normalize_extension_input(raw_extension);
        self.db
            .connection()
            .execute(
                "DELETE FROM file_app_associations WHERE extension = ?1",
                [&ext],
            )
            .map_err(|e| AppError::Database(format!("Failed to delete file association: {}", e)))?;
        Ok(())
    }

    /// Lists all saved LinuxSCP file extension associations ordered by extension.
    pub fn list_associations(&self) -> Result<Vec<FileAppAssociation>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "SELECT extension, desktop_app_id, created_at, updated_at_ms
                 FROM file_app_associations
                 ORDER BY extension ASC",
            )
            .map_err(|e| {
                AppError::Database(format!("Failed to prepare list_associations: {}", e))
            })?;

        let rows = stmt
            .query_map([], |row| {
                Ok(FileAppAssociation {
                    extension: row.get(0)?,
                    desktop_app_id: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at_ms: row.get::<_, i64>(3)? as u64,
                })
            })
            .map_err(|e| AppError::Database(format!("Failed to query associations: {}", e)))?;

        let mut list = Vec::new();
        for item in rows {
            list.push(item.map_err(|e| {
                AppError::Database(format!("Error reading association row: {}", e))
            })?);
        }
        Ok(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop_integration::NO_EXTENSION_KEY;

    #[test]
    fn test_file_app_association_crud_and_case_normalization() {
        let db = Database::open_in_memory().expect("Should initialize in-memory DB");
        let repo = AppAssociationsRepository::new(&db);

        // Save with uppercase without leading dot
        let saved = repo
            .set_association("TXT", "org.gnome.TextEditor.desktop")
            .expect("Should save association");
        assert_eq!(saved.extension, ".txt");
        assert_eq!(saved.desktop_app_id, "org.gnome.TextEditor.desktop");

        // Retrieve with different case and leading dot
        let fetched = repo
            .get_association(".Txt")
            .expect("Should query")
            .expect("Should find .txt association");
        assert_eq!(fetched.desktop_app_id, "org.gnome.TextEditor.desktop");

        // Save association for extensionless files
        repo.set_association("", "code.desktop")
            .expect("Should save no-extension association");
        let no_ext = repo
            .get_association(NO_EXTENSION_KEY)
            .unwrap()
            .expect("Should find no-extension association");
        assert_eq!(no_ext.desktop_app_id, "code.desktop");

        // List all
        let all = repo.list_associations().unwrap();
        assert_eq!(all.len(), 2);

        // Delete
        repo.delete_association(".TXT").unwrap();
        assert!(repo.get_association(".txt").unwrap().is_none());
    }
}
