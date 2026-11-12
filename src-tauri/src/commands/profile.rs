use crate::errors::AppError;
use crate::models::{ConnectionProfile, ConnectionProfileSummary};
use crate::persistence::{Database, KeyringService, ProfileRepository};

/// Lists saved connection profiles with secret presence indicators.
#[tauri::command]
pub async fn list_connection_profiles() -> Result<Vec<ConnectionProfileSummary>, AppError> {
    tokio::task::spawn_blocking(|| {
        let db = Database::open_default()?;
        let repo = ProfileRepository::new(&db);
        let profiles = repo.list_profiles()?;

        let summaries = profiles
            .into_iter()
            .map(|p| {
                let has_saved_secret = KeyringService::has_secret(&p.id).unwrap_or(false);
                ConnectionProfileSummary {
                    id: p.id,
                    name: p.name,
                    protocol: p.protocol,
                    host: p.host,
                    port: p.port,
                    username: p.username,
                    has_saved_secret,
                    updated_at_ms: p.updated_at_ms,
                }
            })
            .collect();

        Ok(summaries)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Profile list task failed: {}", e)))?
}

/// Retrieves a full connection profile configuration by id.
#[tauri::command]
pub async fn get_connection_profile(id: String) -> Result<ConnectionProfile, AppError> {
    tokio::task::spawn_blocking(move || {
        let db = Database::open_default()?;
        let repo = ProfileRepository::new(&db);
        repo.get_profile(&id)?
            .ok_or_else(|| AppError::Database(format!("Profile '{}' not found", id)))
    })
    .await
    .map_err(|e| AppError::Internal(format!("Profile get task failed: {}", e)))?
}

/// Saves or updates a connection profile and optionally updates its secret in Keyring.
#[tauri::command]
pub async fn save_connection_profile(
    profile: ConnectionProfile,
    secret: Option<String>,
) -> Result<ConnectionProfile, AppError> {
    tokio::task::spawn_blocking(move || {
        profile.validate()?;

        // If a secret is provided, save it to Linux Secret Service first
        if let Some(ref sec) = secret {
            if !sec.is_empty() {
                KeyringService::set_secret(&profile.id, sec)?;
            }
        }

        let db = Database::open_default()?;
        let repo = ProfileRepository::new(&db);
        repo.save_profile(&profile)?;

        Ok(profile)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Profile save task failed: {}", e)))?
}

/// Duplicates an existing connection profile under a new name and copies its secret if present.
#[tauri::command]
pub async fn duplicate_connection_profile(
    id: String,
    new_name: String,
) -> Result<ConnectionProfile, AppError> {
    tokio::task::spawn_blocking(move || {
        let db = Database::open_default()?;
        let repo = ProfileRepository::new(&db);

        let duplicated = repo.duplicate_profile(&id, &new_name)?;

        // If the original profile had a saved secret, copy it to the duplicated profile ID
        if let Ok(Some(secret)) = KeyringService::get_secret(&id) {
            let _ = KeyringService::set_secret(&duplicated.id, &secret);
        }

        Ok(duplicated)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Profile duplicate task failed: {}", e)))?
}

/// Deletes a connection profile and cleans up its Secret Service keyring entry.
#[tauri::command]
pub async fn delete_connection_profile(id: String) -> Result<(), AppError> {
    tokio::task::spawn_blocking(move || {
        let db = Database::open_default()?;
        let repo = ProfileRepository::new(&db);

        repo.delete_profile(&id)?;
        let _ = KeyringService::delete_secret(&id);

        Ok(())
    })
    .await
    .map_err(|e| AppError::Internal(format!("Profile delete task failed: {}", e)))?
}

/// Checks if Linux Secret Service / Keyring integration is accessible on the host.
#[tauri::command]
pub async fn check_keyring_status() -> Result<bool, AppError> {
    tokio::task::spawn_blocking(|| {
        let probe_id = format!("probe-{}", uuid::Uuid::new_v4());
        match KeyringService::has_secret(&probe_id) {
            Ok(_) => Ok(true),
            Err(AppError::Security(msg)) => Err(AppError::Security(msg)),
            Err(e) => Err(AppError::Internal(format!("Keyring probe error: {}", e))),
        }
    })
    .await
    .map_err(|e| AppError::Internal(format!("Keyring probe task failed: {}", e)))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AuthMethod, ProtocolType};

    #[tokio::test]
    async fn test_profile_validation_rejects_invalid_on_save() {
        let invalid = ConnectionProfile::new(
            "".to_string(), // Empty name should fail validation
            ProtocolType::Sftp,
            "10.0.0.1".to_string(),
            "root".to_string(),
            AuthMethod::Password,
        );

        let res = save_connection_profile(invalid, None).await;
        assert!(res.is_err());
        assert!(matches!(res.unwrap_err(), AppError::Validation(_)));
    }
}
