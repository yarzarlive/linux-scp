use crate::errors::AppError;
use crate::filesystem::local::{
    create_directory, create_file, delete_entries, get_home_dir, list_directory, rename_entry,
    LocalDirectoryListing,
};
use crate::persistence::{Database, PreferencesRepository};

/// Lists entries in a local directory, expanding tildes and resolving canonical paths.
#[tauri::command]
pub async fn list_local_directory(
    path: Option<String>,
    show_hidden: Option<bool>,
) -> Result<LocalDirectoryListing, AppError> {
    let target_path = path.unwrap_or_else(get_home_dir);
    let show = show_hidden.unwrap_or(false);

    tokio::task::spawn_blocking(move || list_directory(&target_path, show))
        .await
        .map_err(|e| AppError::Internal(format!("Filesystem task joined with error: {}", e)))?
}

/// Retrieves the current user's home directory.
#[tauri::command]
pub async fn get_home_directory() -> Result<String, AppError> {
    Ok(get_home_dir())
}

/// Creates a new directory inside parent_path.
#[tauri::command]
pub async fn create_local_directory(
    parent_path: String,
    dir_name: String,
) -> Result<String, AppError> {
    tokio::task::spawn_blocking(move || create_directory(&parent_path, &dir_name))
        .await
        .map_err(|e| AppError::Internal(format!("Filesystem task joined with error: {}", e)))?
}

/// Creates a new empty file inside parent_path.
#[tauri::command]
pub async fn create_local_file(parent_path: String, file_name: String) -> Result<String, AppError> {
    tokio::task::spawn_blocking(move || create_file(&parent_path, &file_name))
        .await
        .map_err(|e| AppError::Internal(format!("Filesystem task joined with error: {}", e)))?
}

/// Renames a file or directory.
#[tauri::command]
pub async fn rename_local_entry(source_path: String, new_name: String) -> Result<String, AppError> {
    tokio::task::spawn_blocking(move || rename_entry(&source_path, &new_name))
        .await
        .map_err(|e| AppError::Internal(format!("Filesystem task joined with error: {}", e)))?
}

/// Deletes one or more files, symlinks, or directories.
#[tauri::command]
pub async fn delete_local_entries(paths: Vec<String>) -> Result<Vec<String>, AppError> {
    tokio::task::spawn_blocking(move || delete_entries(paths))
        .await
        .map_err(|e| AppError::Internal(format!("Filesystem task joined with error: {}", e)))?
}

/// Retrieves saved directory paths (bookmarks) for a pane (e.g. "local" or "remote").
#[tauri::command]
pub async fn get_saved_paths(pane_type: String) -> Result<Vec<String>, AppError> {
    tokio::task::spawn_blocking(move || {
        let db = Database::open_default()?;
        let repo = PreferencesRepository::new(&db);
        repo.get_saved_paths(&pane_type)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Failed to retrieve saved paths: {}", e)))?
}

/// Persists saved directory paths (bookmarks) for a pane (e.g. "local" or "remote").
#[tauri::command]
pub async fn save_saved_paths(pane_type: String, paths: Vec<String>) -> Result<(), AppError> {
    tokio::task::spawn_blocking(move || {
        let db = Database::open_default()?;
        let repo = PreferencesRepository::new(&db);
        repo.set_saved_paths(&pane_type, &paths)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Failed to save paths: {}", e)))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_home_directory() {
        let home = get_home_directory().await.expect("Should return home dir");
        assert!(!home.is_empty());
    }

    #[tokio::test]
    async fn test_list_local_directory_default() {
        let res = list_local_directory(Some("/usr".to_string()), Some(false))
            .await
            .expect("Should list /usr");
        assert_eq!(res.current_path, "/usr");
    }

    #[tokio::test]
    async fn test_crud_commands_flow() {
        let temp_dir =
            std::env::temp_dir().join(format!("linuxscp_ipc_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let parent = temp_dir.to_str().unwrap().to_string();

        // 1. Create directory
        let created_dir = create_local_directory(parent.clone(), "test_ipc_dir".to_string())
            .await
            .expect("Should create directory");
        assert!(std::path::Path::new(&created_dir).is_dir());

        // 2. Create file
        let created_file = create_local_file(parent.clone(), "test_ipc_file.txt".to_string())
            .await
            .expect("Should create file");
        assert!(std::path::Path::new(&created_file).is_file());

        // 3. Rename file
        let renamed_file =
            rename_local_entry(created_file.clone(), "renamed_ipc_file.txt".to_string())
                .await
                .expect("Should rename file");
        assert!(std::path::Path::new(&renamed_file).is_file());
        assert!(!std::path::Path::new(&created_file).exists());

        // 4. Delete entries
        let deleted = delete_local_entries(vec![created_dir.clone(), renamed_file.clone()])
            .await
            .expect("Should delete entries");
        assert_eq!(deleted.len(), 2);
        assert!(!std::path::Path::new(&created_dir).exists());
        assert!(!std::path::Path::new(&renamed_file).exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
