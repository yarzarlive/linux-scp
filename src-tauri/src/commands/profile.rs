use crate::errors::AppError;
use crate::models::{ConnectionProfile, ConnectionProfileSummary};
use crate::persistence::{Database, KeyringService, ProfileRepository};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileExportPayload {
    pub version: u32,
    pub app: String,
    pub exported_at_ms: u128,
    pub count: usize,
    pub profiles: Vec<ConnectionProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportProfilesResult {
    pub file_path: Option<String>,
    pub json_content: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportProfilesResult {
    pub imported_count: usize,
    pub total_count: usize,
    pub mode: String,
}

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

/// Exports all saved connection profiles as formatted JSON, optionally writing to destination_path.
#[tauri::command]
pub async fn export_connection_profiles(
    destination_path: Option<String>,
) -> Result<ExportProfilesResult, AppError> {
    tokio::task::spawn_blocking(move || {
        let db = Database::open_default()?;
        let repo = ProfileRepository::new(&db);
        let profiles = repo.list_profiles()?;

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);

        let payload = ProfileExportPayload {
            version: 1,
            app: "LinuxSCP".to_string(),
            exported_at_ms: now_ms,
            count: profiles.len(),
            profiles: profiles.clone(),
        };

        let json_content = serde_json::to_string_pretty(&payload).map_err(|e| {
            AppError::Internal(format!("Failed to serialize profiles export: {}", e))
        })?;

        let mut saved_file_path = None;
        if let Some(dest) = destination_path {
            let trimmed = dest.trim();
            if !trimmed.is_empty() {
                let expanded = shellexpand::tilde(trimmed).to_string();
                let path = std::path::Path::new(&expanded);
                if let Some(parent) = path.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent).map_err(|e| {
                            AppError::Filesystem(format!(
                                "Failed to create directory '{}': {}",
                                parent.display(),
                                e
                            ))
                        })?;
                    }
                }
                std::fs::write(path, &json_content).map_err(|e| {
                    AppError::Filesystem(format!(
                        "Failed to write export file '{}': {}",
                        expanded, e
                    ))
                })?;
                saved_file_path = Some(expanded);
            }
        }

        Ok(ExportProfilesResult {
            file_path: saved_file_path,
            json_content,
            count: profiles.len(),
        })
    })
    .await
    .map_err(|e| AppError::Internal(format!("Export profiles task failed: {}", e)))?
}

/// Imports connection profiles from JSON content using 'merge' or 'replace' mode.
#[tauri::command]
pub async fn import_connection_profiles(
    json_content: String,
    mode: String,
) -> Result<ImportProfilesResult, AppError> {
    tokio::task::spawn_blocking(move || {
        let mode_normalized = mode.trim().to_lowercase();
        if mode_normalized != "merge" && mode_normalized != "replace" {
            return Err(AppError::Validation(format!(
                "Invalid import mode '{}'. Expected 'merge' or 'replace'.",
                mode
            )));
        }

        let trimmed_content = json_content.trim();
        if trimmed_content.is_empty() {
            return Err(AppError::Validation(
                "Cannot import from empty configuration content".to_string(),
            ));
        }

        let profiles: Vec<ConnectionProfile> = if let Ok(payload) =
            serde_json::from_str::<ProfileExportPayload>(trimmed_content)
        {
            payload.profiles
        } else if let Ok(list) = serde_json::from_str::<Vec<ConnectionProfile>>(trimmed_content) {
            list
        } else {
            return Err(AppError::Validation(
                "Invalid JSON: content must be a valid LinuxSCP profile export or profile array"
                    .to_string(),
            ));
        };

        if profiles.is_empty() {
            return Err(AppError::Validation(
                "Configuration contains no site profiles to import".to_string(),
            ));
        }

        // Validate each profile before touching database
        for profile in &profiles {
            profile.validate()?;
        }

        let db = Database::open_default()?;
        let repo = ProfileRepository::new(&db);

        if mode_normalized == "replace" {
            repo.clear_all_profiles()?;
            for mut profile in profiles.clone() {
                if profile.id.trim().is_empty() {
                    profile.id = uuid::Uuid::new_v4().to_string();
                }
                repo.save_profile(&profile)?;
            }
        } else {
            // "merge"
            let existing = repo.list_profiles()?;
            for mut profile in profiles.clone() {
                if profile.id.trim().is_empty() {
                    if let Some(matched) = existing
                        .iter()
                        .find(|e| e.name.eq_ignore_ascii_case(&profile.name))
                    {
                        profile.id = matched.id.clone();
                    } else {
                        profile.id = uuid::Uuid::new_v4().to_string();
                    }
                } else if !existing.iter().any(|e| e.id == profile.id) {
                    if let Some(matched) = existing
                        .iter()
                        .find(|e| e.name.eq_ignore_ascii_case(&profile.name))
                    {
                        profile.id = matched.id.clone();
                    }
                }
                repo.save_profile(&profile)?;
            }
        }

        let all_profiles = repo.list_profiles()?;
        Ok(ImportProfilesResult {
            imported_count: profiles.len(),
            total_count: all_profiles.len(),
            mode: mode_normalized,
        })
    })
    .await
    .map_err(|e| AppError::Internal(format!("Import profiles task failed: {}", e)))?
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

    #[tokio::test]
    async fn test_export_and_import_profiles_roundtrip() {
        let p1 = ConnectionProfile::new(
            "Export Test Alpha".to_string(),
            ProtocolType::Sftp,
            "192.168.10.1".to_string(),
            "deploy".to_string(),
            AuthMethod::Password,
        );
        let _ = save_connection_profile(p1.clone(), None).await.unwrap();

        // Test export without destination path
        let exported = export_connection_profiles(None).await.unwrap();
        assert!(exported.count >= 1);
        assert!(exported.json_content.contains("Export Test Alpha"));

        // Test import merge
        let import_merge_res =
            import_connection_profiles(exported.json_content.clone(), "merge".to_string())
                .await
                .unwrap();
        assert_eq!(import_merge_res.mode, "merge");
        assert!(import_merge_res.imported_count >= 1);

        // Test import replace with single profile
        let single_payload = ProfileExportPayload {
            version: 1,
            app: "LinuxSCP".to_string(),
            exported_at_ms: 123456789,
            count: 1,
            profiles: vec![p1.clone()],
        };
        let single_json = serde_json::to_string(&single_payload).unwrap();
        let import_replace_res = import_connection_profiles(single_json, "replace".to_string())
            .await
            .unwrap();
        assert_eq!(import_replace_res.mode, "replace");
        assert_eq!(import_replace_res.imported_count, 1);
        assert_eq!(import_replace_res.total_count, 1);

        // Test invalid mode rejection
        let invalid_mode =
            import_connection_profiles(exported.json_content, "invalid_mode".to_string()).await;
        assert!(invalid_mode.is_err());
        assert!(matches!(invalid_mode.unwrap_err(), AppError::Validation(_)));
    }
}
