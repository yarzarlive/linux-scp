use crate::desktop_integration::{
    app_matches_mime_types, build_app_launch_plan, discover_system_applications,
    format_extension_display, infer_mime_types_for_extension, normalize_file_extension,
    spawn_app_launch_plan, validate_local_file_target, FileAppAssociation, LocalFileLaunchResult,
    LocalFileOpenOutcome, OpenWithContext, NO_EXTENSION_KEY,
};
use crate::errors::AppError;
use crate::persistence::{AppAssociationsRepository, Database, PreferencesRepository};
use crate::terminal::system_path_dirs;

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

