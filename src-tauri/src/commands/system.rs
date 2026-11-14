use crate::errors::AppError;
use crate::persistence::{Database, PreferencesRepository};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemInfo {
    pub app_name: String,
    pub app_version: String,
    pub os_name: String,
    pub os_family: String,
    pub arch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PingResponse {
    pub status: String,
    pub timestamp_ms: u64,
}

/// Retrieves authentic application and host platform metadata.
#[tauri::command]
pub async fn get_system_info() -> Result<SystemInfo, AppError> {
    tracing::debug!("Handling get_system_info command");
    Ok(SystemInfo {
        app_name: env!("CARGO_PKG_NAME").to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        os_name: std::env::consts::OS.to_string(),
        os_family: std::env::consts::FAMILY.to_string(),
        arch: std::env::consts::ARCH.to_string(),
    })
}

/// Verifies bidirectional IPC responsiveness with a timestamped response.
#[tauri::command]
pub async fn ping_backend(payload: String) -> Result<PingResponse, AppError> {
    tracing::debug!("Handling ping_backend with payload: {}", payload);
    if payload.trim().is_empty() {
        return Err(AppError::Validation(
            "Payload must not be empty".to_string(),
        ));
    }

    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    Ok(PingResponse {
        status: format!("ACK: {}", payload),
        timestamp_ms,
    })
}

/// Retrieves the saved theme preference ("dark" or "light").
#[tauri::command]
pub async fn get_theme_preference() -> Result<String, AppError> {
    tokio::task::spawn_blocking(|| {
        let db = Database::open_default()?;
        let repo = PreferencesRepository::new(&db);
        repo.get_theme_preference()
    })
    .await
    .map_err(|e| AppError::Internal(format!("Failed to retrieve theme: {}", e)))?
}

/// Persists the theme preference ("dark" or "light").
#[tauri::command]
pub async fn set_theme_preference(theme: String) -> Result<(), AppError> {
    tokio::task::spawn_blocking(move || {
        let db = Database::open_default()?;
        let repo = PreferencesRepository::new(&db);
        repo.set_theme_preference(&theme)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Failed to save theme: {}", e)))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_system_info() {
        let info = get_system_info().await.expect("Should return SystemInfo");
        assert_eq!(info.app_name, "linuxscp");
        assert_eq!(info.os_name, "linux");
    }

    #[tokio::test]
    async fn test_ping_backend_success() {
        let res = ping_backend("healthcheck".to_string())
            .await
            .expect("Should ping");
        assert_eq!(res.status, "ACK: healthcheck");
        assert!(res.timestamp_ms > 0);
    }

    #[tokio::test]
    async fn test_ping_backend_validation_error() {
        let res = ping_backend("   ".to_string()).await;
        assert_eq!(
            res,
            Err(AppError::Validation(
                "Payload must not be empty".to_string()
            ))
        );
    }
}
