use crate::errors::AppError;
use crate::persistence::{Database, PreferencesRepository, ProfileRepository};
use crate::terminal::{
    build_terminal_launch_plan_with_directory, inspect_terminal_environment, spawn_terminal_plan,
    system_path_dirs, TerminalEnvironmentInfo, TerminalLaunchResult, TerminalPreference,
};

/// Inspects installed terminal emulators, OpenSSH binary availability, and saved user terminal preferences.
#[tauri::command]
pub async fn get_terminal_environment() -> Result<TerminalEnvironmentInfo, AppError> {
    tokio::task::spawn_blocking(|| {
        let db = Database::open_default()?;
        let repo = PreferencesRepository::new(&db);
        let preference = repo.get_terminal_preference()?;
        let search_dirs = system_path_dirs();
        Ok(inspect_terminal_environment(preference, &search_dirs))
    })
    .await
    .map_err(|e| {
        AppError::Internal(format!(
            "Terminal environment inspection task failed: {}",
            e
        ))
    })?
}

/// Validates and saves the user's preferred external terminal configuration.
#[tauri::command]
pub async fn save_terminal_preference(
    preference: TerminalPreference,
) -> Result<TerminalPreference, AppError> {
    tokio::task::spawn_blocking(move || {
        preference.validate()?;
        let db = Database::open_default()?;
        let repo = PreferencesRepository::new(&db);
        repo.set_terminal_preference(&preference)?;
        Ok(preference)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Save terminal preference task failed: {}", e)))?
}

/// Launches an external interactive SSH terminal session for the selected connection profile,
/// optionally changing directory to `remote_directory` upon connection.
///
/// Security invariants:
/// - Never reads or passes passwords/passphrases on the command line or environment.
/// - Passes host, port, username, and optional identity file as distinct `Command` arguments.
/// - Does not claim the terminal session is authenticated merely because the process spawned.
#[tauri::command]
pub async fn launch_ssh_terminal(
    profile_id: String,
    remote_directory: Option<String>,
) -> Result<TerminalLaunchResult, AppError> {
    let trimmed_id = profile_id.trim().to_string();
    if trimmed_id.is_empty() {
        return Err(AppError::Validation(
            "A connection profile must be selected to open an SSH terminal.".to_string(),
        ));
    }

    let trimmed_dir = remote_directory
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty());

    tokio::task::spawn_blocking(move || {
        let db = Database::open_default()?;
        let profile_repo = ProfileRepository::new(&db);
        let profile = profile_repo.get_profile(&trimmed_id)?.ok_or_else(|| {
            AppError::Validation(format!(
                "Connection profile '{}' was not found. Save the connection profile first.",
                trimmed_id
            ))
        })?;

        let pref_repo = PreferencesRepository::new(&db);
        let preference = pref_repo.get_terminal_preference()?;

        let search_dirs = system_path_dirs();
        let plan = build_terminal_launch_plan_with_directory(
            &profile,
            &preference,
            &search_dirs,
            trimmed_dir.as_deref(),
        )?;
        spawn_terminal_plan(&profile, &plan)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Terminal launch task failed: {}", e)))?
}
