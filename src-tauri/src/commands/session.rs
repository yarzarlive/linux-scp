use crate::errors::AppError;
use crate::persistence::{Database, KnownHostEntry, KnownHostsRepository, ProfileRepository};
use crate::protocol::{ConnectOutcome, SessionManager, SessionStateSnapshot};
use tauri::State;

/// Connects to an SFTP server using a saved profile ID.
#[tauri::command]
pub async fn connect_sftp_session(
    state: State<'_, SessionManager>,
    profile_id: String,
    secret: Option<String>,
) -> Result<ConnectOutcome, AppError> {
    let profile = tokio::task::spawn_blocking(move || {
        let db = Database::open_default()?;
        let repo = ProfileRepository::new(&db);
        repo.get_profile(&profile_id)?
            .ok_or_else(|| AppError::Validation(format!("Profile '{}' not found", profile_id)))
    })
    .await
    .map_err(|e| AppError::Internal(format!("Profile lookup task failed: {}", e)))??;

    state.connect(profile, secret, false).await
}

/// Disconnects the active SFTP session (or cancels an in-progress connection).
#[tauri::command]
pub async fn disconnect_sftp_session(
    state: State<'_, SessionManager>,
) -> Result<SessionStateSnapshot, AppError> {
    state.disconnect().await
}

/// Reconnects to the most recently used connection profile.
#[tauri::command]
pub async fn reconnect_sftp_session(
    state: State<'_, SessionManager>,
    secret: Option<String>,
) -> Result<ConnectOutcome, AppError> {
    state.reconnect(secret).await
}

/// Retrieves the current session lifecycle state snapshot.
#[tauri::command]
pub async fn get_session_state(
    state: State<'_, SessionManager>,
) -> Result<SessionStateSnapshot, AppError> {
    Ok(state.snapshot().await)
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
