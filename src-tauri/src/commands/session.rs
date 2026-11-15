use crate::errors::AppError;
use crate::persistence::{Database, KnownHostEntry, KnownHostsRepository, ProfileRepository};
use crate::protocol::{
    ConnectOutcome, RemoteDirectoryListing, SessionManager, SessionStateSnapshot,
};
use tauri::State;

/// Connects to an SFTP server using a saved profile ID.
#[tauri::command]
pub async fn connect_sftp_session(
    state: State<'_, SessionManager>,
    profile_id: String,
    secret: Option<String>,
    session_id: Option<String>,
) -> Result<ConnectOutcome, AppError> {
    let profile = tokio::task::spawn_blocking(move || {
        let db = Database::open_default()?;
        let repo = ProfileRepository::new(&db);
        repo.get_profile(&profile_id)?
            .ok_or_else(|| AppError::Validation(format!("Profile '{}' not found", profile_id)))
    })
    .await
    .map_err(|e| AppError::Internal(format!("Profile lookup task failed: {}", e)))??;

    state
        .connect_with_session_id(profile, secret, false, session_id)
        .await
}

/// Disconnects a remote session (or active session if session_id is None).
#[tauri::command]
pub async fn disconnect_sftp_session(
    state: State<'_, SessionManager>,
    session_id: Option<String>,
) -> Result<SessionStateSnapshot, AppError> {
    state.disconnect_session(session_id.as_deref()).await
}

/// Reconnects to the most recently used connection profile for a session.
#[tauri::command]
pub async fn reconnect_sftp_session(
    state: State<'_, SessionManager>,
    secret: Option<String>,
    session_id: Option<String>,
) -> Result<ConnectOutcome, AppError> {
    state.reconnect_session(secret, session_id.as_deref()).await
}

/// Retrieves the session lifecycle state snapshot for a session (or active session).
#[tauri::command]
pub async fn get_session_state(
    state: State<'_, SessionManager>,
    session_id: Option<String>,
) -> Result<SessionStateSnapshot, AppError> {
    Ok(state.snapshot_for(session_id.as_deref()).await)
}

/// Sets the currently focused / active connection session ID.
#[tauri::command]
pub async fn set_active_session(
    state: State<'_, SessionManager>,
    session_id: String,
) -> Result<(), AppError> {
    state.set_active_session_id(&session_id).await;
    Ok(())
}

/// Explicitly trusts and stores an unknown SSH host key after user approval.
/// Strictly refuses to overwrite an existing changed host key.
#[tauri::command]
pub async fn trust_host_key(
    host: String,
    port: u16,
    algorithm: String,
    fingerprint_sha256: String,
    public_key_openssh: String,
) -> Result<KnownHostEntry, AppError> {
    tokio::task::spawn_blocking(move || {
        let db = Database::open_default()?;
        let repo = KnownHostsRepository::new(&db);
        repo.trust_unknown_host_key(
            &host,
            port,
            &algorithm,
            &fingerprint_sha256,
            &public_key_openssh,
        )
    })
    .await
    .map_err(|e| AppError::Internal(format!("Trust host key task failed: {}", e)))?
}

/// Lists all trusted SSH host keys in the persistent known_hosts store.
#[tauri::command]
pub async fn list_known_hosts() -> Result<Vec<KnownHostEntry>, AppError> {
    tokio::task::spawn_blocking(|| {
        let db = Database::open_default()?;
        let repo = KnownHostsRepository::new(&db);
        repo.list_known_hosts()
    })
    .await
    .map_err(|e| AppError::Internal(format!("List known hosts task failed: {}", e)))?
}

/// Lists a remote directory using the active or specified SFTP session.
#[tauri::command]
pub async fn list_remote_directory(
    state: State<'_, SessionManager>,
    path: Option<String>,
    show_hidden: Option<bool>,
    session_id: Option<String>,
    refresh: Option<bool>,
) -> Result<RemoteDirectoryListing, AppError> {
    state
        .list_remote_directory(
            path,
            show_hidden.unwrap_or(false),
            session_id.as_deref(),
            refresh.unwrap_or(false),
        )
        .await
}

/// Checks whether a remote path (following symlinks) is a directory.
#[tauri::command]
pub async fn check_remote_path_is_dir(
    state: State<'_, SessionManager>,
    path: String,
    session_id: Option<String>,
) -> Result<bool, AppError> {
    state
        .check_remote_path_is_dir(path, session_id.as_deref())
        .await
}

/// Creates a new directory on the remote SFTP server.
#[tauri::command]
pub async fn create_remote_directory(
    state: State<'_, SessionManager>,
    parent_path: String,
    dir_name: String,
    session_id: Option<String>,
) -> Result<String, AppError> {
    state
        .create_remote_directory(&parent_path, &dir_name, session_id.as_deref())
        .await
}

/// Creates a new empty file on the remote SFTP server.
#[tauri::command]
pub async fn create_remote_file(
    state: State<'_, SessionManager>,
    parent_path: String,
    file_name: String,
    session_id: Option<String>,
) -> Result<String, AppError> {
    state
        .create_remote_file(&parent_path, &file_name, session_id.as_deref())
        .await
}

/// Renames a remote file or directory.
#[tauri::command]
pub async fn rename_remote_entry(
    state: State<'_, SessionManager>,
    source_path: String,
    new_name: String,
    session_id: Option<String>,
) -> Result<String, AppError> {
    state
        .rename_remote_entry(&source_path, &new_name, session_id.as_deref())
        .await
}

/// Deletes a remote file or symlink.
#[tauri::command]
pub async fn delete_remote_file(
    state: State<'_, SessionManager>,
    path: String,
    session_id: Option<String>,
) -> Result<(), AppError> {
    state.delete_remote_file(&path, session_id.as_deref()).await
}

/// Deletes a remote directory (recursive only when explicitly confirmed).
#[tauri::command]
pub async fn delete_remote_directory(
    state: State<'_, SessionManager>,
    path: String,
    recursive: bool,
    session_id: Option<String>,
) -> Result<(), AppError> {
    state
        .delete_remote_directory(&path, recursive, session_id.as_deref())
        .await
}

/// Sets Unix permissions (octal mode, optionally recursive) on one or more remote paths.
#[tauri::command]
pub async fn set_remote_permissions(
    state: State<'_, SessionManager>,
    paths: Vec<String>,
    octal_mode: String,
    recursive: bool,
    session_id: Option<String>,
) -> Result<(), AppError> {
    state
        .set_remote_permissions(paths, &octal_mode, recursive, session_id.as_deref())
        .await
}

/// Sets numeric UID/GID (or resolves names via optional `getent`) on a remote path.
#[tauri::command]
pub async fn set_remote_ownership(
    state: State<'_, SessionManager>,
    path: String,
    uid: Option<u32>,
    gid: Option<u32>,
    user_name: Option<String>,
    group_name: Option<String>,
    session_id: Option<String>,
) -> Result<(), AppError> {
    state
        .set_remote_ownership(
            &path,
            uid,
            gid,
            user_name,
            group_name,
            session_id.as_deref(),
        )
        .await
}

/// Optionally resolves numeric UID/GID to user/group names via remote `getent`.
#[tauri::command]
pub async fn resolve_remote_identity(
    state: State<'_, SessionManager>,
    uid: Option<u32>,
    gid: Option<u32>,
    session_id: Option<String>,
) -> Result<crate::models::FileOwnership, AppError> {
    state
        .resolve_remote_identity(uid, gid, session_id.as_deref())
        .await
}
