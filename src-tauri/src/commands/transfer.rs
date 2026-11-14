use crate::errors::AppError;
use crate::models::{ConflictDecision, ConflictPolicy, TransferJob};
use crate::transfer::TransferManager;
use tauri::State;

/// Enqueues one or more local files or directories for SFTP upload.
#[tauri::command]
pub async fn enqueue_uploads(
    state: State<'_, TransferManager>,
    local_paths: Vec<String>,
    remote_dir: String,
    conflict_policy: Option<ConflictPolicy>,
    session_id: Option<String>,
) -> Result<Vec<TransferJob>, AppError> {
    state
        .enqueue_uploads_for_session(
            local_paths,
            remote_dir,
            conflict_policy.unwrap_or_default(),
            session_id,
        )
        .await
}

/// Enqueues one or more remote files or directories for SFTP download.
#[tauri::command]
pub async fn enqueue_downloads(
    state: State<'_, TransferManager>,
    remote_paths: Vec<String>,
    local_dir: String,
    conflict_policy: Option<ConflictPolicy>,
    session_id: Option<String>,
) -> Result<Vec<TransferJob>, AppError> {
    state
        .enqueue_downloads_for_session(
            remote_paths,
            local_dir,
            conflict_policy.unwrap_or_default(),
            session_id,
        )
        .await
}

/// Lists all transfer jobs in the current queue.
#[tauri::command]
pub async fn list_transfers(
    state: State<'_, TransferManager>,
) -> Result<Vec<TransferJob>, AppError> {
    Ok(state.list_transfers().await)
}

/// Cancels a queued or active transfer job.
#[tauri::command]
pub async fn cancel_transfer(
    state: State<'_, TransferManager>,
    job_id: String,
) -> Result<(), AppError> {
    state.cancel_transfer(&job_id).await
}

/// Retries a failed or cancelled transfer job.
#[tauri::command]
pub async fn retry_transfer(
    state: State<'_, TransferManager>,
    job_id: String,
) -> Result<TransferJob, AppError> {
    state.retry_transfer(&job_id).await
}

/// Delivers an explicit user conflict resolution decision to a waiting transfer.
#[tauri::command]
pub async fn resolve_transfer_conflict(
    state: State<'_, TransferManager>,
    conflict_id: String,
    decision: ConflictDecision,
    apply_to_all: bool,
) -> Result<(), AppError> {
    state
        .resolve_transfer_conflict(&conflict_id, decision, apply_to_all)
        .await
}

/// Clears completed and cancelled jobs from the queue.
#[tauri::command]
pub async fn clear_finished_transfers(
    state: State<'_, TransferManager>,
) -> Result<Vec<TransferJob>, AppError> {
    Ok(state.clear_finished_transfers().await)
}
