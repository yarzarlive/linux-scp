use crate::desktop_integration::{
    app_matches_mime_types, build_app_launch_plan, discover_system_applications,
    format_extension_display, infer_mime_types_for_extension, normalize_file_extension,
    spawn_app_launch_plan, validate_local_file_target, DesktopAppInfo, FileAppAssociation,
    LocalFileLaunchResult, LocalFileOpenOutcome, OpenWithContext, RemoteFileLaunchResult,
    RemoteFileOpenOutcome, NO_EXTENSION_KEY,
};
use crate::errors::AppError;
use crate::persistence::{AppAssociationsRepository, Database, PreferencesRepository};
use crate::remote_edit::{
    RemoteEditManager, RemoteEditSession, RemoteEditUploadMode, RemoteEditUploadOutcome,
};
use crate::terminal::system_path_dirs;
use tauri::State;

/// Discovers installed Linux XDG desktop applications and resolves any remembered association
/// and MIME-type recommendations for the given file path or filename.
#[tauri::command]
pub async fn discover_desktop_applications(
    file_path: Option<String>,
) -> Result<OpenWithContext, AppError> {
    tokio::task::spawn_blocking(move || {
        let extension_key = file_path
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(normalize_file_extension)
            .unwrap_or_else(|| NO_EXTENSION_KEY.to_string());

        let extension_display = format_extension_display(&extension_key);
        let applications = discover_system_applications();

        let db = Database::open_default()?;
        let assoc_repo = AppAssociationsRepository::new(&db);
        let remembered_app_id = assoc_repo
            .get_association(&extension_key)?
            .map(|a| a.desktop_app_id);

        let remembered_app_installed = remembered_app_id
            .as_deref()
            .map(|id| applications.iter().any(|app| app.id == id))
            .unwrap_or(false);

        let candidate_mimes = infer_mime_types_for_extension(&extension_key);
        let recommended_app_ids: Vec<String> = applications
            .iter()
            .filter(|app| app_matches_mime_types(app, &candidate_mimes))
            .map(|app| app.id.clone())
            .collect();

        Ok(OpenWithContext {
            extension_key,
            extension_display,
            remembered_app_id,
            remembered_app_installed,
            recommended_app_ids,
            applications,
        })
    })
    .await
    .map_err(|e| AppError::Internal(format!("Application discovery task failed: {}", e)))?
}

/// Attempts to open a local file using its remembered default desktop application.
/// If no default is remembered or the remembered application is no longer installed,
/// returns `LocalFileOpenOutcome::NeedsAppPicker` so the UI can display the `Open With` dialog.
#[tauri::command]
pub async fn open_local_file_default(file_path: String) -> Result<LocalFileOpenOutcome, AppError> {
    tokio::task::spawn_blocking(move || {
        let validated_path = validate_local_file_target(&file_path)?;
        let canonical_str = validated_path.to_string_lossy().into_owned();
        let extension_key = normalize_file_extension(&canonical_str);
        let extension_display = format_extension_display(&extension_key);

        let db = Database::open_default()?;
        let assoc_repo = AppAssociationsRepository::new(&db);
        let Some(assoc) = assoc_repo.get_association(&extension_key)? else {
            return Ok(LocalFileOpenOutcome::NeedsAppPicker {
                extension_key,
                extension_display,
                missing_remembered_app_id: None,
            });
        };

        let apps = discover_system_applications();
        let Some(app) = apps.into_iter().find(|a| a.id == assoc.desktop_app_id) else {
            return Ok(LocalFileOpenOutcome::NeedsAppPicker {
                extension_key,
                extension_display,
                missing_remembered_app_id: Some(assoc.desktop_app_id),
            });
        };

        let pref_repo = PreferencesRepository::new(&db);
        let term_pref = pref_repo.get_terminal_preference().ok();
        let path_dirs = system_path_dirs();

        let plan = build_app_launch_plan(
            &app,
            std::slice::from_ref(&canonical_str),
            &path_dirs,
            term_pref.as_ref(),
        )?;
        let pid = spawn_app_launch_plan(&app, &plan)?;

        Ok(LocalFileOpenOutcome::Launched {
            app,
            extension_key,
            pid,
        })
    })
    .await
    .map_err(|e| AppError::Internal(format!("Open local file task failed: {}", e)))?
}

/// Opens a local file with the explicitly chosen desktop application and optionally stores
/// that application as the LinuxSCP-scoped default for the file's extension.
#[tauri::command]
pub async fn open_local_file_with_app(
    file_path: String,
    desktop_app_id: String,
    remember_default: bool,
) -> Result<LocalFileLaunchResult, AppError> {
    tokio::task::spawn_blocking(move || {
        let validated_path = validate_local_file_target(&file_path)?;
        let canonical_str = validated_path.to_string_lossy().into_owned();
        let extension_key = normalize_file_extension(&canonical_str);

        let trimmed_app_id = desktop_app_id.trim();
        if trimmed_app_id.is_empty() {
            return Err(AppError::Validation(
                "Please select an application to open the file.".to_string(),
            ));
        }

        let apps = discover_system_applications();
        let app = apps
            .into_iter()
            .find(|a| a.id == trimmed_app_id)
            .ok_or_else(|| {
                AppError::Validation(format!(
                    "Selected desktop application '{}' is not installed or could not be found.",
                    trimmed_app_id
                ))
            })?;

        let db = Database::open_default()?;
        let pref_repo = PreferencesRepository::new(&db);
        let term_pref = pref_repo.get_terminal_preference().ok();
        let path_dirs = system_path_dirs();

        let plan = build_app_launch_plan(
            &app,
            std::slice::from_ref(&canonical_str),
            &path_dirs,
            term_pref.as_ref(),
        )?;
        let pid = spawn_app_launch_plan(&app, &plan)?;

        if remember_default {
            let assoc_repo = AppAssociationsRepository::new(&db);
            assoc_repo.set_association(&extension_key, &app.id)?;
        }

        Ok(LocalFileLaunchResult {
            app,
            extension_key,
            saved_as_default: remember_default,
            pid,
        })
    })
    .await
    .map_err(|e| AppError::Internal(format!("Open local file with app task failed: {}", e)))?
}

/// Lists all LinuxSCP-specific file extension to desktop application associations.
#[tauri::command]
pub async fn list_file_app_associations() -> Result<Vec<FileAppAssociation>, AppError> {
    tokio::task::spawn_blocking(|| {
        let db = Database::open_default()?;
        let repo = AppAssociationsRepository::new(&db);
        repo.list_associations()
    })
    .await
    .map_err(|e| AppError::Internal(format!("List associations task failed: {}", e)))?
}

/// Deletes a remembered default application association for a file extension.
#[tauri::command]
pub async fn delete_file_app_association(extension: String) -> Result<(), AppError> {
    tokio::task::spawn_blocking(move || {
        let db = Database::open_default()?;
        let repo = AppAssociationsRepository::new(&db);
        repo.delete_association(&extension)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Delete association task failed: {}", e)))?
}

/// Attempts to open a remote file using its remembered default desktop application.
/// If a default is installed, downloads the remote file into a managed temporary directory,
/// tracks it for modifications, and spawns the application. If no default is remembered
/// (or the remembered app is uninstalled), returns `RemoteFileOpenOutcome::NeedsAppPicker`
/// without downloading prematurely.
#[tauri::command]
pub async fn open_remote_file_default(
    state: State<'_, RemoteEditManager>,
    remote_path: String,
) -> Result<RemoteFileOpenOutcome, AppError> {
    let trimmed = remote_path.trim().to_string();
    if trimmed.is_empty() || trimmed.contains('\0') {
        return Err(AppError::Validation(
            "Remote file path cannot be empty.".to_string(),
        ));
    }

    let lookup_path = trimmed.clone();
    let lookup_res: Result<Result<(DesktopAppInfo, String), RemoteFileOpenOutcome>, AppError> =
        tokio::task::spawn_blocking(move || {
            let extension_key = normalize_file_extension(&lookup_path);
            let extension_display = format_extension_display(&extension_key);

            let db = Database::open_default()?;
            let assoc_repo = AppAssociationsRepository::new(&db);
            let Some(assoc) = assoc_repo.get_association(&extension_key)? else {
                return Ok(Err(RemoteFileOpenOutcome::NeedsAppPicker {
                    extension_key,
                    extension_display,
                    missing_remembered_app_id: None,
                }));
            };

            let apps = discover_system_applications();
            let Some(app) = apps.into_iter().find(|a| a.id == assoc.desktop_app_id) else {
                return Ok(Err(RemoteFileOpenOutcome::NeedsAppPicker {
                    extension_key,
                    extension_display,
                    missing_remembered_app_id: Some(assoc.desktop_app_id),
                }));
            };

            Ok(Ok((app, extension_key)))
        })
        .await
        .map_err(|e| {
            AppError::Internal(format!("Resolve remote default app task failed: {}", e))
        })?;

    let (app, extension_key) = match lookup_res? {
        Ok(found) => found,
        Err(needs_picker) => return Ok(needs_picker),
    };

    let session = state.open_session(&trimmed).await?;
    let local_path = session.local_path.clone();
    let app_for_spawn = app.clone();

    let pid = tokio::task::spawn_blocking(move || {
        let db = Database::open_default()?;
        let pref_repo = PreferencesRepository::new(&db);
        let term_pref = pref_repo.get_terminal_preference().ok();
        let path_dirs = system_path_dirs();

        let plan = build_app_launch_plan(
            &app_for_spawn,
            std::slice::from_ref(&local_path),
            &path_dirs,
            term_pref.as_ref(),
        )?;
        spawn_app_launch_plan(&app_for_spawn, &plan)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Spawn remote edit app task failed: {}", e)))??;

    let updated_session = state.record_launch(&session.id, &app.id, &app.name).await?;

    Ok(RemoteFileOpenOutcome::Launched {
        app,
        extension_key,
        pid,
        session: updated_session,
    })
}

/// Downloads a remote file to a managed temporary working copy, launches the explicitly
/// selected desktop application, and optionally stores the default association for the extension.
#[tauri::command]
pub async fn open_remote_file_with_app(
    state: State<'_, RemoteEditManager>,
    remote_path: String,
    desktop_app_id: String,
    remember_default: bool,
) -> Result<RemoteFileLaunchResult, AppError> {
    let trimmed_path = remote_path.trim().to_string();
    if trimmed_path.is_empty() || trimmed_path.contains('\0') {
        return Err(AppError::Validation(
            "Remote file path cannot be empty.".to_string(),
        ));
    }
    let trimmed_app_id = desktop_app_id.trim().to_string();
    if trimmed_app_id.is_empty() {
        return Err(AppError::Validation(
            "Please select an application to open the file.".to_string(),
        ));
    }

    let lookup_path = trimmed_path.clone();
    let lookup_app_id = trimmed_app_id.clone();
    let (app, extension_key) =
        tokio::task::spawn_blocking(move || -> Result<(DesktopAppInfo, String), AppError> {
            let extension_key = normalize_file_extension(&lookup_path);
            let apps = discover_system_applications();
            let app = apps
                .into_iter()
                .find(|a| a.id == lookup_app_id)
                .ok_or_else(|| {
                    AppError::Validation(format!(
                        "Selected desktop application '{}' is not installed or could not be found.",
                        lookup_app_id
                    ))
                })?;
            Ok((app, extension_key))
        })
        .await
        .map_err(|e| AppError::Internal(format!("Lookup remote edit app task failed: {}", e)))??;

    let session = state.open_session(&trimmed_path).await?;
    let local_path = session.local_path.clone();
    let app_for_spawn = app.clone();
    let ext_for_save = extension_key.clone();

    let pid = tokio::task::spawn_blocking(move || -> Result<Option<u32>, AppError> {
        let db = Database::open_default()?;
        let pref_repo = PreferencesRepository::new(&db);
        let term_pref = pref_repo.get_terminal_preference().ok();
        let path_dirs = system_path_dirs();

        let plan = build_app_launch_plan(
            &app_for_spawn,
            std::slice::from_ref(&local_path),
            &path_dirs,
            term_pref.as_ref(),
        )?;
        let pid = spawn_app_launch_plan(&app_for_spawn, &plan)?;

        if remember_default {
            let assoc_repo = AppAssociationsRepository::new(&db);
            assoc_repo.set_association(&ext_for_save, &app_for_spawn.id)?;
        }

        Ok(pid)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Spawn remote edit app task failed: {}", e)))??;

    let updated_session = state.record_launch(&session.id, &app.id, &app.name).await?;

    Ok(RemoteFileLaunchResult {
        app,
        extension_key,
        saved_as_default: remember_default,
        pid,
        session: updated_session,
    })
}

/// Lists all active remote editing sessions after performing an immediate local change check.
#[tauri::command]
pub async fn list_remote_edit_sessions(
    state: State<'_, RemoteEditManager>,
) -> Result<Vec<RemoteEditSession>, AppError> {
    let _ = state.scan_local_changes(false).await;
    Ok(state.list().await)
}

/// Performs an immediate scan of tracked local temporary working copies (e.g. on window focus)
/// and returns the updated list of remote editing sessions.
#[tauri::command]
pub async fn scan_remote_edit_changes(
    state: State<'_, RemoteEditManager>,
) -> Result<Vec<RemoteEditSession>, AppError> {
    let _ = state.scan_local_changes(false).await;
    Ok(state.list().await)
}

/// Uploads a modified local temporary working copy back to the remote server after verifying
/// remote endpoint identity and conflict status.
#[tauri::command]
pub async fn upload_remote_edit_session(
    state: State<'_, RemoteEditManager>,
    session_id: String,
    overwrite_remote_changes: Option<bool>,
) -> Result<RemoteEditUploadOutcome, AppError> {
    state
        .upload(&session_id, overwrite_remote_changes.unwrap_or(false))
        .await
}

/// Marks a modified remote editing session as "keep local changes" so the upload prompt
/// is dismissed until the local temporary working copy is modified again.
#[tauri::command]
pub async fn keep_local_remote_edit_changes(
    state: State<'_, RemoteEditManager>,
    session_id: String,
) -> Result<RemoteEditSession, AppError> {
    state.keep_local(&session_id).await
}

/// Saves a copy of the local temporary working copy into `destination_dir` under a non-conflicting name.
#[tauri::command]
pub async fn save_remote_edit_local_copy(
    state: State<'_, RemoteEditManager>,
    session_id: String,
    destination_dir: String,
) -> Result<String, AppError> {
    state.save_copy(&session_id, &destination_dir).await
}

/// Discards a tracked remote editing session and removes its temporary working copy directory.
#[tauri::command]
pub async fn discard_remote_edit_session(
    state: State<'_, RemoteEditManager>,
    session_id: String,
    discard_local_changes: Option<bool>,
) -> Result<(), AppError> {
    state
        .discard(&session_id, discard_local_changes.unwrap_or(false))
        .await
}

/// Replaces the local temporary working copy of an active remote editing session with the
/// latest version downloaded from the remote server, and re-launches the associated desktop
/// application if requested.
#[tauri::command]
pub async fn reload_remote_edit_from_remote(
    state: State<'_, RemoteEditManager>,
    session_id: String,
    relaunch_app: Option<bool>,
) -> Result<RemoteEditSession, AppError> {
    let updated = state.reload_from_remote(&session_id).await?;
    if relaunch_app.unwrap_or(true) {
        if let Some(ref app_id) = updated.app_id {
            let app_id_clone = app_id.clone();
            let local_path = updated.local_path.clone();
            let _ = tokio::task::spawn_blocking(move || -> Result<(), AppError> {
                let apps = discover_system_applications();
                if let Some(app) = apps.into_iter().find(|a| a.id == app_id_clone) {
                    let db = Database::open_default()?;
                    let pref_repo = PreferencesRepository::new(&db);
                    let term_pref = pref_repo.get_terminal_preference().ok();
                    let path_dirs = system_path_dirs();
                    let plan = build_app_launch_plan(
                        &app,
                        std::slice::from_ref(&local_path),
                        &path_dirs,
                        term_pref.as_ref(),
                    )?;
                    let _ = spawn_app_launch_plan(&app, &plan)?;
                }
                Ok(())
            })
            .await;
        }
    }
    Ok(updated)
}

/// Returns the current configured `RemoteEditUploadMode` (`auto` or `prompt`).
#[tauri::command]
pub async fn get_remote_edit_upload_mode(
    state: State<'_, RemoteEditManager>,
) -> Result<RemoteEditUploadMode, AppError> {
    Ok(state.upload_mode().await)
}

/// Updates and persists the `RemoteEditUploadMode` (`auto` or `prompt`).
#[tauri::command]
pub async fn set_remote_edit_upload_mode(
    state: State<'_, RemoteEditManager>,
    mode: RemoteEditUploadMode,
) -> Result<RemoteEditUploadMode, AppError> {
    tokio::task::spawn_blocking(move || {
        let db = Database::open_default()?;
        let pref_repo = PreferencesRepository::new(&db);
        pref_repo.set_remote_edit_upload_mode(mode)
    })
    .await
    .map_err(|e| {
        AppError::Internal(format!("Save remote edit upload mode task failed: {}", e))
    })??;

    state.set_upload_mode(mode).await;
    Ok(mode)
}
