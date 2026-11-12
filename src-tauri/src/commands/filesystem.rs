use crate::errors::AppError;
use crate::filesystem::local::{get_home_dir, list_directory, LocalDirectoryListing};

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
}
