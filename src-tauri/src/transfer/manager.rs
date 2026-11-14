use crate::errors::AppError;
use crate::models::{
    ConflictDecision, ConflictPolicy, ProfileTransferSettings, TransferConflictPrompt,
    TransferDirection, TransferJob, TransferProgressEvent, TransferState,
};
use crate::protocol::sftp::{classify_sftp_error, join_posix_path, normalize_posix_path};
use crate::protocol::{ActiveTransferContext, SessionManager, TransferClient};
use russh_sftp::client::error::Error as SftpClientError;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::{FileAttributes, StatusCode};
use std::collections::{HashMap, VecDeque};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot, watch, RwLock, Semaphore};
use uuid::Uuid;

/// Bounded streaming buffer size (64 KB). Never loads entire files into memory.
const CHUNK_SIZE: usize = 64 * 1024;
/// Minimum interval between `transfer://progress` IPC events.
const PROGRESS_THROTTLE: Duration = Duration::from_millis(75);
/// Rolling window for transfer speed calculation.
const SPEED_WINDOW: Duration = Duration::from_millis(1500);
/// Maximum directory recursion depth to prevent runaway symlink/tree loops.
const MAX_RECURSION_DEPTH: usize = 64;

/// Abstraction for emitting typed transfer lifecycle, progress, and conflict events.
pub trait TransferEventSink: Send + Sync {
    fn emit_updated(&self, job: &TransferJob);
    fn emit_progress(&self, progress: &TransferProgressEvent);
    fn emit_conflict(&self, conflict: &TransferConflictPrompt);
}

/// Production Tauri event sink emitting to the frontend window.
pub struct TauriTransferEventSink {
    app_handle: AppHandle,
}

impl TauriTransferEventSink {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }
}

impl TransferEventSink for TauriTransferEventSink {
    fn emit_updated(&self, job: &TransferJob) {
        let _ = self.app_handle.emit("transfer://updated", job);
    }

    fn emit_progress(&self, progress: &TransferProgressEvent) {
        let _ = self.app_handle.emit("transfer://progress", progress);
    }

    fn emit_conflict(&self, conflict: &TransferConflictPrompt) {
        let _ = self.app_handle.emit("transfer://conflict", conflict);
    }
}

/// No-op event sink for unit tests that do not inspect events.
pub struct NoopTransferEventSink;

impl TransferEventSink for NoopTransferEventSink {
    fn emit_updated(&self, _job: &TransferJob) {}
    fn emit_progress(&self, _progress: &TransferProgressEvent) {}
    fn emit_conflict(&self, _conflict: &TransferConflictPrompt) {}
}

/// Test event captured by `ChannelTransferEventSink`.
#[derive(Debug, Clone)]
pub enum CapturedTransferEvent {
    Updated(TransferJob),
    Progress(TransferProgressEvent),
    Conflict(TransferConflictPrompt),
}

/// In-memory channel sink for deterministic integration testing.
pub struct ChannelTransferEventSink {
    tx: mpsc::UnboundedSender<CapturedTransferEvent>,
}

impl ChannelTransferEventSink {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<CapturedTransferEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { tx }, rx)
    }
}

impl TransferEventSink for ChannelTransferEventSink {
    fn emit_updated(&self, job: &TransferJob) {
        let _ = self.tx.send(CapturedTransferEvent::Updated(job.clone()));
    }

    fn emit_progress(&self, progress: &TransferProgressEvent) {
        let _ = self
            .tx
            .send(CapturedTransferEvent::Progress(progress.clone()));
    }

    fn emit_conflict(&self, conflict: &TransferConflictPrompt) {
        let _ = self
            .tx
            .send(CapturedTransferEvent::Conflict(conflict.clone()));
    }
}

/// Rolling-window throughput calculator.
struct SpeedTracker {
    samples: VecDeque<(Instant, u64)>,
    window_bytes: u64,
}

impl SpeedTracker {
    fn new() -> Self {
        Self {
            samples: VecDeque::new(),
            window_bytes: 0,
        }
    }

    fn record(&mut self, bytes: u64) {
        let now = Instant::now();
        self.samples.push_back((now, bytes));
        self.window_bytes += bytes;
        self.prune(now);
    }

    fn prune(&mut self, now: Instant) {
        while let Some((ts, b)) = self.samples.front() {
            if now.duration_since(*ts) > SPEED_WINDOW {
                self.window_bytes = self.window_bytes.saturating_sub(*b);
                self.samples.pop_front();
            } else {
                break;
            }
        }
    }

    fn speed_bps(&mut self) -> u64 {
        let now = Instant::now();
        self.prune(now);
        let Some((first_ts, _)) = self.samples.front() else {
            return 0;
        };
        let elapsed = now.duration_since(*first_ts).as_secs_f64();
        if elapsed < 0.05 {
            0
        } else {
            (self.window_bytes as f64 / elapsed) as u64
        }
    }
}

/// Generates a numbered rename candidate like `report (1).pdf` or `archive (2)`.
pub fn generate_renamed_path(path: &str, attempt: usize) -> String {
    let p = Path::new(path);
    let parent = p.parent().map(|d| d.to_string_lossy().to_string());
    let file_name = p
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());

    let (stem, ext) = match file_name.rfind('.') {
        Some(idx) if idx > 0 => (&file_name[..idx], Some(&file_name[idx..])),
        _ => (file_name.as_str(), None),
    };

    let new_name = match ext {
        Some(e) => format!("{} ({}){}", stem, attempt, e),
        None => format!("{} ({})", stem, attempt),
    };

    match parent {
        Some(dir) if !dir.is_empty() => {
            if dir == "/" {
                format!("/{}", new_name)
            } else {
                format!("{}/{}", dir.trim_end_matches('/'), new_name)
            }
        }
        _ => new_name,
    }
}

type PendingConflictMap = HashMap<String, oneshot::Sender<(ConflictDecision, bool)>>;

/// Manages queued, active, and finished file/directory transfers.
#[derive(Clone)]
pub struct TransferManager {
    session_manager: SessionManager,
    event_sink: Arc<dyn TransferEventSink>,
    jobs: Arc<RwLock<Vec<TransferJob>>>,
    cancellations: Arc<RwLock<HashMap<String, watch::Sender<bool>>>>,
    pending_conflicts: Arc<RwLock<PendingConflictMap>>,
    concurrency_sem: Arc<Semaphore>,
}

impl TransferManager {
    pub fn new(session_manager: SessionManager, event_sink: Arc<dyn TransferEventSink>) -> Self {
        Self {
            session_manager,
            event_sink,
            jobs: Arc::new(RwLock::new(Vec::new())),
            cancellations: Arc::new(RwLock::new(HashMap::new())),
            pending_conflicts: Arc::new(RwLock::new(HashMap::new())),
            concurrency_sem: Arc::new(Semaphore::new(2)),
        }
    }

    /// Lists all transfer jobs in the queue.
    pub async fn list_transfers(&self) -> Vec<TransferJob> {
        self.jobs.read().await.clone()
    }

    /// Enqueues local files/directories for upload to `remote_dir` over the active session.
    pub async fn enqueue_uploads(
        &self,
        local_paths: Vec<String>,
        remote_dir: String,
        conflict_policy: ConflictPolicy,
    ) -> Result<Vec<TransferJob>, AppError> {
        self.enqueue_uploads_for_session(local_paths, remote_dir, conflict_policy, None)
            .await
    }

    /// Enqueues local files/directories for upload to `remote_dir`, optionally specifying a session ID.
    pub async fn enqueue_uploads_for_session(
        &self,
        local_paths: Vec<String>,
        remote_dir: String,
        conflict_policy: ConflictPolicy,
        session_id: Option<String>,
    ) -> Result<Vec<TransferJob>, AppError> {
        if local_paths.is_empty() {
            return Err(AppError::Validation(
                "No local paths selected for upload".to_string(),
            ));
        }

        let ctx = match session_id {
            Some(ref sid) => self.session_manager.transfer_context_for(sid).await?,
            None => self.session_manager.active_transfer_context().await?,
        };
        let dest_dir = normalize_posix_path(&remote_dir);
        let mut created = Vec::new();

        for src in local_paths {
            let src_path = PathBuf::from(&src);
            let meta = tokio::fs::symlink_metadata(&src_path).await.map_err(|e| {
                AppError::Filesystem(format!("Cannot access local path '{}': {}", src, e))
            })?;

            let name = src_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .ok_or_else(|| AppError::Validation(format!("Invalid source path '{}'", src)))?;

            let dest_path = join_posix_path(&dest_dir, &name);
            let is_dir = meta.is_dir();
            let total_bytes = if is_dir { None } else { Some(meta.len()) };

            let mut job = TransferJob::new(
                ctx.session_id.clone(),
                src,
                dest_path,
                TransferDirection::Upload,
                total_bytes,
                conflict_policy,
            );
            job.is_directory = is_dir;
            job.total_files = if is_dir { 0 } else { 1 };
            job.current_file = Some(name);

            self.insert_and_spawn(job.clone()).await;
            created.push(job);
        }

        Ok(created)
    }

    /// Enqueues remote files/directories for download to `local_dir` over the active session.
    pub async fn enqueue_downloads(
        &self,
        remote_paths: Vec<String>,
        local_dir: String,
        conflict_policy: ConflictPolicy,
    ) -> Result<Vec<TransferJob>, AppError> {
        self.enqueue_downloads_for_session(remote_paths, local_dir, conflict_policy, None)
            .await
    }

    /// Enqueues remote files/directories for download to `local_dir`, optionally specifying a session ID.
    pub async fn enqueue_downloads_for_session(
        &self,
        remote_paths: Vec<String>,
        local_dir: String,
        conflict_policy: ConflictPolicy,
        session_id: Option<String>,
    ) -> Result<Vec<TransferJob>, AppError> {
        if remote_paths.is_empty() {
            return Err(AppError::Validation(
                "No remote paths selected for download".to_string(),
            ));
        }

        let ctx = match session_id {
            Some(ref sid) => self.session_manager.transfer_context_for(sid).await?,
            None => self.session_manager.active_transfer_context().await?,
        };
        let local_base = PathBuf::from(&local_dir);
        if !local_base.is_dir() {
            return Err(AppError::Validation(format!(
                "Local destination '{}' is not a directory",
                local_dir
            )));
        }

        let mut created = Vec::new();
        for src in remote_paths {
            let norm_src = normalize_posix_path(&src);
            let name = norm_src
                .rsplit('/')
                .next()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| AppError::Validation(format!("Invalid remote path '{}'", norm_src)))?
                .to_string();

            let dest_path = local_base.join(&name).to_string_lossy().to_string();
            let (is_dir, total_bytes) = if let Some(sftp) = &ctx.sftp {
                let attrs = tokio::time::timeout(
                    ctx.timeout_duration,
                    sftp.symlink_metadata(norm_src.as_str()),
                )
                .await
                .map_err(|_| {
                    AppError::Network(format!("Timed out inspecting remote path '{}'", norm_src))
                })?
                .map_err(|e| classify_sftp_error(e, &norm_src))?;
                let dir = attrs.is_dir();
                let bytes = if dir { None } else { attrs.size };
                (dir, bytes)
            } else {
                (false, None)
            };

            let mut job = TransferJob::new(
                ctx.session_id.clone(),
                norm_src,
                dest_path,
                TransferDirection::Download,
                total_bytes,
                conflict_policy,
            );
            job.is_directory = is_dir;
            job.total_files = if is_dir { 0 } else { 1 };
            job.current_file = Some(name);

            self.insert_and_spawn(job.clone()).await;
            created.push(job);
        }

        Ok(created)
    }

    /// Cancels a queued or active transfer job.
    pub async fn cancel_transfer(&self, job_id: &str) -> Result<(), AppError> {
        if let Some(tx) = self.cancellations.read().await.get(job_id) {
            let _ = tx.send(true);
        }

        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.iter_mut().find(|j| j.id.0 == job_id) {
            if !job.state.is_terminal() {
                job.state = TransferState::Cancelled;
                job.speed_bps = 0;
                job.eta_seconds = None;
                job.updated_at_ms = now_ms();
                self.event_sink.emit_updated(job);
            }
            Ok(())
        } else {
            Err(AppError::Validation(format!(
                "Transfer job '{}' not found",
                job_id
            )))
        }
    }

    /// Retries a failed or cancelled transfer job.
    pub async fn retry_transfer(&self, job_id: &str) -> Result<TransferJob, AppError> {
        let updated_job = {
            let mut jobs = self.jobs.write().await;
            let job = jobs.iter_mut().find(|j| j.id.0 == job_id).ok_or_else(|| {
                AppError::Validation(format!("Transfer job '{}' not found", job_id))
            })?;

            if !matches!(
                job.state,
                TransferState::Failed { .. } | TransferState::Cancelled
            ) {
                return Err(AppError::Validation(
                    "Only failed or cancelled transfers can be retried".to_string(),
                ));
            }

            job.state = TransferState::Queued;
            job.transferred_bytes = 0;
            job.speed_bps = 0;
            job.eta_seconds = None;
            job.completed_files = 0;
            job.updated_at_ms = now_ms();
            self.event_sink.emit_updated(job);
            job.clone()
        };

        self.spawn_worker(updated_job.clone()).await;
        Ok(updated_job)
    }

    /// Delivers a user's explicit conflict decision to a waiting transfer job.
    pub async fn resolve_transfer_conflict(
        &self,
        conflict_id: &str,
        decision: ConflictDecision,
        apply_to_all: bool,
    ) -> Result<(), AppError> {
        let sender = self.pending_conflicts.write().await.remove(conflict_id);
        match sender {
            Some(tx) => {
                let _ = tx.send((decision, apply_to_all));
                Ok(())
            }
            None => Err(AppError::Validation(format!(
                "Conflict prompt '{}' is no longer active",
                conflict_id
            ))),
        }
    }

    /// Removes completed and cancelled jobs from the queue list.
    pub async fn clear_finished_transfers(&self) -> Vec<TransferJob> {
        let mut jobs = self.jobs.write().await;
        jobs.retain(|j| !matches!(j.state, TransferState::Completed | TransferState::Cancelled));
        jobs.clone()
    }

    async fn insert_and_spawn(&self, job: TransferJob) {
        {
            let mut jobs = self.jobs.write().await;
            jobs.push(job.clone());
        }
        self.event_sink.emit_updated(&job);
        self.spawn_worker(job).await;
    }

    async fn spawn_worker(&self, job: TransferJob) {
        let (cancel_tx, cancel_rx) = watch::channel(false);
        self.cancellations
            .write()
            .await
            .insert(job.id.0.clone(), cancel_tx);

        let manager = self.clone();
        tokio::spawn(async move {
            manager.run_job(job, cancel_rx).await;
        });
    }

    async fn run_job(&self, job: TransferJob, mut cancel_rx: watch::Receiver<bool>) {
        let job_id = job.id.0.clone();

        // Wait for a concurrency slot, aborting immediately if cancelled while queued
        let _permit = tokio::select! {
            permit = self.concurrency_sem.acquire() => {
                match permit {
                    Ok(p) => p,
                    Err(_) => return,
                }
            }
            _ = wait_cancelled(&mut cancel_rx) => {
                self.finish_cancelled(&job_id).await;
                self.cancellations.write().await.remove(&job_id);
                return;
            }
        };

        if *cancel_rx.borrow() {
            self.finish_cancelled(&job_id).await;
            self.cancellations.write().await.remove(&job_id);
            return;
        }

        // Transition: Connecting
        self.set_job_state(&job_id, TransferState::Connecting).await;

        let ctx = match self
            .session_manager
            .transfer_context_for(&job.session_id)
            .await
        {
            Ok(c) => c,
            Err(_) => match self.session_manager.active_transfer_context().await {
                Ok(c) => c,
                Err(e) => {
                    self.finish_failed(&job_id, e.to_string()).await;
                    self.cancellations.write().await.remove(&job_id);
                    return;
                }
            },
        };

        let result = match job.direction {
            TransferDirection::Upload => self.execute_upload_job(&job, &ctx, &mut cancel_rx).await,
            TransferDirection::Download => {
                self.execute_download_job(&job, &ctx, &mut cancel_rx).await
            }
        };

        match result {
            Ok(()) => {
                self.finish_completed(&job_id).await;
            }
            Err(TransferFailure::Cancelled) => {
                self.finish_cancelled(&job_id).await;
            }
            Err(TransferFailure::Error(msg)) => {
                self.finish_failed(&job_id, msg).await;
            }
        }

        self.cancellations.write().await.remove(&job_id);
    }

    async fn execute_upload_job(
        &self,
        job: &TransferJob,
        ctx: &ActiveTransferContext,
        cancel_rx: &mut watch::Receiver<bool>,
    ) -> Result<(), TransferFailure> {
        if let TransferClient::Scp { ssh_handle } = &ctx.client {
            return self
                .execute_scp_upload(job, ssh_handle, ctx, cancel_rx)
                .await;
        }

        let sftp = ctx
            .sftp
            .as_ref()
            .ok_or_else(|| TransferFailure::Error("SFTP session unavailable".to_string()))?;

        let job_id = &job.id.0;
        let mut tracker = SpeedTracker::new();
        let mut last_emit = Instant::now() - PROGRESS_THROTTLE;
        let mut policy_override: Option<ConflictPolicy> = None;

        if !job.is_directory {
            let src_path = Path::new(&job.source_path);
            let meta = tokio::fs::symlink_metadata(src_path)
                .await
                .map_err(|e| TransferFailure::Error(format!("Failed to stat local file: {}", e)))?;
            let total_bytes = meta.len();
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs());
            let mode = Some(meta.mode());

            self.set_job_totals(job_id, Some(total_bytes), 1).await;
            self.set_job_state(job_id, TransferState::Transferring)
                .await;

            let mut transferred = 0u64;
            self.upload_single_file(
                job_id,
                src_path,
                &job.destination_path,
                total_bytes,
                mtime,
                mode,
                job.conflict_policy,
                &mut policy_override,
                ctx,
                cancel_rx,
                &mut transferred,
                Some(total_bytes),
                0,
                1,
                &mut tracker,
                &mut last_emit,
            )
            .await?;

            self.update_job_progress(
                job_id,
                transferred,
                Some(total_bytes),
                0,
                None,
                1,
                1,
                job.current_file.clone(),
            )
            .await;
            return Ok(());
        }

        // Recursive directory upload
        let src_root = PathBuf::from(&job.source_path);
        let (rel_dirs, files, total_bytes) = collect_local_tree(&src_root)
            .await
            .map_err(TransferFailure::Error)?;
        let total_files = files.len() as u64;

        self.set_job_totals(job_id, Some(total_bytes), total_files)
            .await;
        self.set_job_state(job_id, TransferState::Transferring)
            .await;

        // Ensure root destination directory and all subdirectories exist
        ensure_remote_dir(sftp, &job.destination_path, ctx.timeout_duration)
            .await
            .map_err(TransferFailure::Error)?;

        for rel_dir in rel_dirs {
            if *cancel_rx.borrow() {
                return Err(TransferFailure::Cancelled);
            }
            let target_dir = join_posix_path(&job.destination_path, &rel_dir);
            ensure_remote_dir(sftp, &target_dir, ctx.timeout_duration)
                .await
                .map_err(TransferFailure::Error)?;
        }

        let mut cumulative_bytes = 0u64;
        let mut completed_files = 0u64;
        let mut failures: Vec<String> = Vec::new();

        for file in files {
            if *cancel_rx.borrow() {
                return Err(TransferFailure::Cancelled);
            }

            let dest_file = join_posix_path(&job.destination_path, &file.rel_path);
            let effective_policy = policy_override.unwrap_or(job.conflict_policy);
            let before_bytes = cumulative_bytes;

            let res = self
                .upload_single_file(
                    job_id,
                    &file.abs_path,
                    &dest_file,
                    file.size,
                    file.mtime_secs,
                    file.mode,
                    effective_policy,
                    &mut policy_override,
                    ctx,
                    cancel_rx,
                    &mut cumulative_bytes,
                    Some(total_bytes),
                    completed_files,
                    total_files,
                    &mut tracker,
                    &mut last_emit,
                )
                .await;

            match res {
                Ok(()) => {
                    completed_files += 1;
                }
                Err(TransferFailure::Cancelled) => return Err(TransferFailure::Cancelled),
                Err(TransferFailure::Error(msg)) => {
                    // Reset cumulative bytes for failed file so progress remains accurate
                    cumulative_bytes = before_bytes;
                    failures.push(format!("{}: {}", file.rel_path, msg));
                }
            }
        }

        self.update_job_progress(
            job_id,
            cumulative_bytes,
            Some(total_bytes),
            0,
            None,
            completed_files,
            total_files,
            None,
        )
        .await;

        if !failures.is_empty() {
            return Err(TransferFailure::Error(format!(
                "{} file(s) failed during directory upload (first: {})",
                failures.len(),
                failures[0]
            )));
        }

        Ok(())
    }

    async fn execute_scp_upload(
        &self,
        job: &TransferJob,
        ssh_handle: &Arc<
            tokio::sync::Mutex<russh::client::Handle<crate::protocol::sftp::SshClientHandler>>,
        >,
        ctx: &ActiveTransferContext,
        cancel_rx: &mut watch::Receiver<bool>,
    ) -> Result<(), TransferFailure> {
        let job_id = &job.id.0;

        let src_path = Path::new(&job.source_path);
        let meta = tokio::fs::symlink_metadata(src_path)
            .await
            .map_err(|e| TransferFailure::Error(format!("Failed to stat local file: {}", e)))?;
        let total_bytes = meta.len();
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs());
        let mode = meta.mode();

        self.set_job_totals(job_id, Some(total_bytes), 1).await;
        self.set_job_state(job_id, TransferState::Transferring)
            .await;

        let preserve_ts =
            ctx.transfer_settings.preserve_timestamps && ctx.capabilities.can_preserve_timestamps;

        let (prog_tx, mut prog_rx) = tokio::sync::mpsc::unbounded_channel::<u64>();
        let mgr = self.clone();
        let job_id_clone = job_id.to_string();
        let display_name_clone = src_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string());
        let monitor_task = tokio::spawn(async move {
            let mut transferred = 0u64;
            let mut tracker = SpeedTracker::new();
            let mut last_emit = Instant::now();
            while let Some(chunk_size) = prog_rx.recv().await {
                transferred += chunk_size;
                tracker.record(chunk_size);
                if last_emit.elapsed() >= PROGRESS_THROTTLE || transferred == total_bytes {
                    last_emit = Instant::now();
                    let speed = tracker.speed_bps();
                    let eta = compute_eta(Some(total_bytes), transferred, speed);
                    mgr.update_job_progress(
                        &job_id_clone,
                        transferred,
                        Some(total_bytes),
                        speed,
                        eta,
                        0,
                        1,
                        display_name_clone.clone(),
                    )
                    .await;
                }
            }
        });

        let handle_guard = ssh_handle.lock().await;
        let res = crate::protocol::scp::upload_single_file_scp(
            &handle_guard,
            src_path,
            &job.destination_path,
            total_bytes,
            mode,
            mtime,
            preserve_ts,
            cancel_rx,
            |chunk_size| {
                let _ = prog_tx.send(chunk_size);
            },
            ctx.timeout_duration,
        )
        .await;

        drop(handle_guard);
        let _ = monitor_task.await;

        match res {
            Ok(()) => {
                self.update_job_progress(
                    job_id,
                    total_bytes,
                    Some(total_bytes),
                    0,
                    None,
                    1,
                    1,
                    src_path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string()),
                )
                .await;
                Ok(())
            }
            Err(AppError::Protocol(msg)) if msg.contains("cancelled") => {
                Err(TransferFailure::Cancelled)
            }
            Err(e) => Err(TransferFailure::Error(e.to_string())),
        }
    }

    async fn execute_scp_download(
        &self,
        job: &TransferJob,
        ssh_handle: &Arc<
            tokio::sync::Mutex<russh::client::Handle<crate::protocol::sftp::SshClientHandler>>,
        >,
        ctx: &ActiveTransferContext,
        cancel_rx: &mut watch::Receiver<bool>,
    ) -> Result<(), TransferFailure> {
        let job_id = &job.id.0;

        let local_path = Path::new(&job.destination_path);
        if let Some(parent) = local_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                TransferFailure::Error(format!("Failed to create destination dir: {}", e))
            })?;
        }

        self.set_job_state(job_id, TransferState::Transferring)
            .await;

        let preserve_ts =
            ctx.transfer_settings.preserve_timestamps && ctx.capabilities.can_preserve_timestamps;

        let (prog_tx, mut prog_rx) = tokio::sync::mpsc::unbounded_channel::<u64>();
        let mgr = self.clone();
        let job_id_clone = job_id.to_string();
        let display_name_clone = local_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string());
        let total_bytes_opt = job.total_bytes;
        let final_transferred = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let final_transferred_clone = final_transferred.clone();
        let monitor_task = tokio::spawn(async move {
            let mut transferred = 0u64;
            let mut tracker = SpeedTracker::new();
            let mut last_emit = Instant::now();
            while let Some(chunk_size) = prog_rx.recv().await {
                transferred += chunk_size;
                final_transferred_clone.store(transferred, std::sync::atomic::Ordering::Relaxed);
                tracker.record(chunk_size);
                if last_emit.elapsed() >= PROGRESS_THROTTLE
                    || (total_bytes_opt.is_some() && Some(transferred) == total_bytes_opt)
                {
                    last_emit = Instant::now();
                    let speed = tracker.speed_bps();
                    let eta = compute_eta(total_bytes_opt, transferred, speed);
                    mgr.update_job_progress(
                        &job_id_clone,
                        transferred,
                        total_bytes_opt,
                        speed,
                        eta,
                        0,
                        1,
                        display_name_clone.clone(),
                    )
                    .await;
                }
            }
        });

        let handle_guard = ssh_handle.lock().await;
        let res = crate::protocol::scp::download_single_file_scp(
            &handle_guard,
            &job.source_path,
            local_path,
            preserve_ts,
            cancel_rx,
            |chunk_size| {
                let _ = prog_tx.send(chunk_size);
            },
            ctx.timeout_duration,
        )
        .await;

        drop(handle_guard);
        let _ = monitor_task.await;
        let transferred = final_transferred.load(std::sync::atomic::Ordering::Relaxed);

        match res {
            Ok(()) => {
                self.update_job_progress(
                    job_id,
                    transferred,
                    Some(transferred),
                    0,
                    None,
                    1,
                    1,
                    local_path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string()),
                )
                .await;
                Ok(())
            }
            Err(AppError::Protocol(msg)) if msg.contains("cancelled") => {
                Err(TransferFailure::Cancelled)
            }
            Err(e) => Err(TransferFailure::Error(e.to_string())),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn upload_single_file(
        &self,
        job_id: &str,
        src_path: &Path,
        dest_path: &str,
        file_size: u64,
        mtime_secs: Option<u64>,
        mode: Option<u32>,
        conflict_policy: ConflictPolicy,
        policy_override: &mut Option<ConflictPolicy>,
        ctx: &ActiveTransferContext,
        cancel_rx: &mut watch::Receiver<bool>,
        cumulative_bytes: &mut u64,
        total_bytes: Option<u64>,
        completed_files: u64,
        total_files: u64,
        tracker: &mut SpeedTracker,
        last_emit: &mut Instant,
    ) -> Result<(), TransferFailure> {
        let sftp = ctx
            .sftp
            .as_ref()
            .ok_or_else(|| TransferFailure::Error("SFTP session unavailable".to_string()))?;

        let display_name = src_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string());

        let dest_meta = check_remote_exists(sftp, dest_path, ctx.timeout_duration).await?;

        let (final_dest, replace_existing) = if let Some(target_attrs) = dest_meta {
            let effective = policy_override.unwrap_or(conflict_policy);
            match self
                .resolve_conflict(
                    job_id,
                    TransferDirection::Upload,
                    &src_path.to_string_lossy(),
                    dest_path,
                    Some(file_size),
                    mtime_secs,
                    target_attrs.size,
                    target_attrs.mtime.map(u64::from),
                    effective,
                    policy_override,
                    cancel_rx,
                )
                .await?
            {
                ResolvedAction::Overwrite => (dest_path.to_string(), true),
                ResolvedAction::Skip => {
                    *cumulative_bytes += file_size;
                    return Ok(());
                }
                ResolvedAction::Rename => {
                    let unique =
                        find_unique_remote_name(sftp, dest_path, ctx.timeout_duration).await?;
                    (unique, false)
                }
            }
        } else {
            (dest_path.to_string(), false)
        };

        let temp_path = format!("{}.linuxscp-tmp-{}", final_dest, Uuid::new_v4());

        let mut local_file = tokio::fs::File::open(src_path)
            .await
            .map_err(|e| TransferFailure::Error(format!("Failed to open local file: {}", e)))?;

        let mut remote_file = tokio::select! {
            res = sftp.create(temp_path.as_str()) => {
                res.map_err(|e| TransferFailure::Error(classify_sftp_error(e, &temp_path).to_string()))?
            }
            _ = wait_cancelled(cancel_rx) => {
                return Err(TransferFailure::Cancelled);
            }
        };

        let mut buf = vec![0u8; CHUNK_SIZE];
        let mut written_this_file = 0u64;

        let stream_res: Result<(), TransferFailure> = async {
            loop {
                let n = tokio::select! {
                    res = local_file.read(&mut buf) => {
                        res.map_err(|e| TransferFailure::Error(format!("Local read error: {}", e)))?
                    }
                    _ = wait_cancelled(cancel_rx) => {
                        return Err(TransferFailure::Cancelled);
                    }
                };

                if n == 0 {
                    break;
                }

                tokio::select! {
                    res = remote_file.write_all(&buf[..n]) => {
                        res.map_err(|e| TransferFailure::Error(format!("Remote write error: {}", e)))?
                    }
                    _ = wait_cancelled(cancel_rx) => {
                        return Err(TransferFailure::Cancelled);
                    }
                }

                written_this_file += n as u64;
                *cumulative_bytes += n as u64;
                tracker.record(n as u64);

                if last_emit.elapsed() >= PROGRESS_THROTTLE {
                    *last_emit = Instant::now();
                    let speed = tracker.speed_bps();
                    let eta = compute_eta(total_bytes, *cumulative_bytes, speed);
                    self.update_job_progress(
                        job_id,
                        *cumulative_bytes,
                        total_bytes,
                        speed,
                        eta,
                        completed_files,
                        total_files,
                        display_name.clone(),
                    )
                    .await;
                }
            }

            remote_file
                .flush()
                .await
                .map_err(|e| TransferFailure::Error(format!("Remote flush error: {}", e)))?;
            remote_file
                .shutdown()
                .await
                .map_err(|e| TransferFailure::Error(format!("Remote close error: {}", e)))?;
            Ok(())
        }
        .await;

        if let Err(err) = stream_res {
            drop(remote_file);
            let _ = sftp.remove_file(temp_path.as_str()).await;
            return Err(err);
        }

        if written_this_file != file_size {
            let _ = sftp.remove_file(temp_path.as_str()).await;
            return Err(TransferFailure::Error(format!(
                "Upload byte mismatch for '{}': expected {} bytes, wrote {}",
                src_path.display(),
                file_size,
                written_this_file
            )));
        }

        // Atomic promotion: remove existing destination if overwriting (SFTPv3 rename fails on existing target)
        if replace_existing {
            let _ = sftp.remove_file(final_dest.as_str()).await;
        }
        if let Err(e) = sftp.rename(temp_path.as_str(), final_dest.as_str()).await {
            let _ = sftp.remove_file(temp_path.as_str()).await;
            return Err(TransferFailure::Error(
                classify_sftp_error(e, &final_dest).to_string(),
            ));
        }

        // Preserve permissions & timestamps only when configured and supported
        apply_remote_metadata(sftp, &final_dest, mode, mtime_secs, ctx).await;

        Ok(())
    }

    async fn execute_download_job(
        &self,
        job: &TransferJob,
        ctx: &ActiveTransferContext,
        cancel_rx: &mut watch::Receiver<bool>,
    ) -> Result<(), TransferFailure> {
        if let TransferClient::Scp { ssh_handle } = &ctx.client {
            return self
                .execute_scp_download(job, ssh_handle, ctx, cancel_rx)
                .await;
        }

        let sftp = ctx
            .sftp
            .as_ref()
            .ok_or_else(|| TransferFailure::Error("SFTP session unavailable".to_string()))?;

        let job_id = &job.id.0;
        let mut tracker = SpeedTracker::new();
        let mut last_emit = Instant::now() - PROGRESS_THROTTLE;
        let mut policy_override: Option<ConflictPolicy> = None;

        if !job.is_directory {
            let attrs = tokio::time::timeout(
                ctx.timeout_duration,
                sftp.metadata(job.source_path.as_str()),
            )
            .await
            .map_err(|_| TransferFailure::Error("Timed out reading remote metadata".to_string()))?
            .map_err(|e| {
                TransferFailure::Error(classify_sftp_error(e, &job.source_path).to_string())
            })?;

            let total_bytes = attrs.size;
            self.set_job_totals(job_id, total_bytes, 1).await;
            self.set_job_state(job_id, TransferState::Transferring)
                .await;

            let mut transferred = 0u64;
            self.download_single_file(
                job_id,
                &job.source_path,
                Path::new(&job.destination_path),
                total_bytes,
                attrs.mtime.map(u64::from),
                attrs.permissions,
                job.conflict_policy,
                &mut policy_override,
                ctx,
                cancel_rx,
                &mut transferred,
                total_bytes,
                0,
                1,
                &mut tracker,
                &mut last_emit,
            )
            .await?;

            self.update_job_progress(
                job_id,
                transferred,
                total_bytes,
                0,
                None,
                1,
                1,
                job.current_file.clone(),
            )
            .await;
            return Ok(());
        }

        // Recursive directory download
        let (rel_dirs, files, total_bytes) =
            collect_remote_tree(sftp, &job.source_path, ctx.timeout_duration)
                .await
                .map_err(TransferFailure::Error)?;
        let total_files = files.len() as u64;

        self.set_job_totals(job_id, Some(total_bytes), total_files)
            .await;
        self.set_job_state(job_id, TransferState::Transferring)
            .await;

        let dest_root = PathBuf::from(&job.destination_path);
        tokio::fs::create_dir_all(&dest_root)
            .await
            .map_err(|e| TransferFailure::Error(format!("Failed to create local dir: {}", e)))?;

        for rel_dir in rel_dirs {
            if *cancel_rx.borrow() {
                return Err(TransferFailure::Cancelled);
            }
            let target_dir = dest_root.join(&rel_dir);
            tokio::fs::create_dir_all(&target_dir).await.map_err(|e| {
                TransferFailure::Error(format!(
                    "Failed to create local dir '{}': {}",
                    target_dir.display(),
                    e
                ))
            })?;
        }

        let mut cumulative_bytes = 0u64;
        let mut completed_files = 0u64;
        let mut failures: Vec<String> = Vec::new();

        for file in files {
            if *cancel_rx.borrow() {
                return Err(TransferFailure::Cancelled);
            }

            let dest_file = dest_root.join(&file.rel_path);
            let effective_policy = policy_override.unwrap_or(job.conflict_policy);
            let before_bytes = cumulative_bytes;

            let res = self
                .download_single_file(
                    job_id,
                    &file.abs_path,
                    &dest_file,
                    file.size,
                    file.mtime_secs,
                    file.mode,
                    effective_policy,
                    &mut policy_override,
                    ctx,
                    cancel_rx,
                    &mut cumulative_bytes,
                    Some(total_bytes),
                    completed_files,
                    total_files,
                    &mut tracker,
                    &mut last_emit,
                )
                .await;

            match res {
                Ok(()) => {
                    completed_files += 1;
                }
                Err(TransferFailure::Cancelled) => return Err(TransferFailure::Cancelled),
                Err(TransferFailure::Error(msg)) => {
                    cumulative_bytes = before_bytes;
                    failures.push(format!("{}: {}", file.rel_path, msg));
                }
            }
        }

        self.update_job_progress(
            job_id,
            cumulative_bytes,
            Some(total_bytes),
            0,
            None,
            completed_files,
            total_files,
            None,
        )
        .await;

        if !failures.is_empty() {
            return Err(TransferFailure::Error(format!(
                "{} file(s) failed during directory download (first: {})",
                failures.len(),
                failures[0]
            )));
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn download_single_file(
        &self,
        job_id: &str,
        src_path: &str,
        dest_path: &Path,
        expected_size: Option<u64>,
        mtime_secs: Option<u64>,
        mode: Option<u32>,
        conflict_policy: ConflictPolicy,
        policy_override: &mut Option<ConflictPolicy>,
        ctx: &ActiveTransferContext,
        cancel_rx: &mut watch::Receiver<bool>,
        cumulative_bytes: &mut u64,
        total_bytes: Option<u64>,
        completed_files: u64,
        total_files: u64,
        tracker: &mut SpeedTracker,
        last_emit: &mut Instant,
    ) -> Result<(), TransferFailure> {
        let display_name = dest_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string());

        let dest_meta = tokio::fs::symlink_metadata(dest_path).await.ok();
        let final_dest = if let Some(target_meta) = dest_meta {
            let target_mtime = target_meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs());
            let effective = policy_override.unwrap_or(conflict_policy);
            match self
                .resolve_conflict(
                    job_id,
                    TransferDirection::Download,
                    src_path,
                    &dest_path.to_string_lossy(),
                    expected_size,
                    mtime_secs,
                    Some(target_meta.len()),
                    target_mtime,
                    effective,
                    policy_override,
                    cancel_rx,
                )
                .await?
            {
                ResolvedAction::Overwrite => dest_path.to_path_buf(),
                ResolvedAction::Skip => {
                    if let Some(sz) = expected_size {
                        *cumulative_bytes += sz;
                    }
                    return Ok(());
                }
                ResolvedAction::Rename => {
                    let unique = find_unique_local_name(dest_path).await;
                    unique
                }
            }
        } else {
            dest_path.to_path_buf()
        };

        if let Some(parent) = final_dest.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                TransferFailure::Error(format!("Failed to create destination directory: {}", e))
            })?;
        }

        let temp_path = PathBuf::from(format!(
            "{}.linuxscp-tmp-{}",
            final_dest.to_string_lossy(),
            Uuid::new_v4()
        ));

        let sftp = ctx
            .sftp
            .as_ref()
            .ok_or_else(|| TransferFailure::Error("SFTP session unavailable".to_string()))?;

        let mut remote_file = tokio::select! {
            res = sftp.open(src_path) => {
                res.map_err(|e| TransferFailure::Error(classify_sftp_error(e, src_path).to_string()))?
            }
            _ = wait_cancelled(cancel_rx) => {
                return Err(TransferFailure::Cancelled);
            }
        };

        let mut local_file = tokio::fs::File::create(&temp_path).await.map_err(|e| {
            TransferFailure::Error(format!("Failed to create temporary download file: {}", e))
        })?;

        let mut buf = vec![0u8; CHUNK_SIZE];
        let mut written_this_file = 0u64;

        let stream_res: Result<(), TransferFailure> = async {
            loop {
                let n = tokio::select! {
                    res = remote_file.read(&mut buf) => {
                        res.map_err(|e| TransferFailure::Error(format!("Remote read error: {}", e)))?
                    }
                    _ = wait_cancelled(cancel_rx) => {
                        return Err(TransferFailure::Cancelled);
                    }
                };

                if n == 0 {
                    break;
                }

                tokio::select! {
                    res = local_file.write_all(&buf[..n]) => {
                        res.map_err(|e| TransferFailure::Error(format!("Local write error: {}", e)))?
                    }
                    _ = wait_cancelled(cancel_rx) => {
                        return Err(TransferFailure::Cancelled);
                    }
                }

                written_this_file += n as u64;
                *cumulative_bytes += n as u64;
                tracker.record(n as u64);

                if last_emit.elapsed() >= PROGRESS_THROTTLE {
                    *last_emit = Instant::now();
                    let speed = tracker.speed_bps();
                    let eta = compute_eta(total_bytes, *cumulative_bytes, speed);
                    self.update_job_progress(
                        job_id,
                        *cumulative_bytes,
                        total_bytes,
                        speed,
                        eta,
                        completed_files,
                        total_files,
                        display_name.clone(),
                    )
                    .await;
                }
            }

            local_file
                .flush()
                .await
                .map_err(|e| TransferFailure::Error(format!("Local flush error: {}", e)))?;
            local_file
                .sync_all()
                .await
                .map_err(|e| TransferFailure::Error(format!("Local sync error: {}", e)))?;
            Ok(())
        }
        .await;

        drop(local_file);
        let _ = remote_file.close().await;

        if let Err(err) = stream_res {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(err);
        }

        if let Some(expected) = expected_size {
            if written_this_file != expected {
                let _ = tokio::fs::remove_file(&temp_path).await;
                return Err(TransferFailure::Error(format!(
                    "Download byte mismatch for '{}': expected {} bytes, got {}",
                    src_path, expected, written_this_file
                )));
            }
        }

        if let Err(e) = tokio::fs::rename(&temp_path, &final_dest).await {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(TransferFailure::Error(format!(
                "Failed to finalize downloaded file '{}': {}",
                final_dest.display(),
                e
            )));
        }

        apply_local_metadata(&final_dest, mode, mtime_secs, &ctx.transfer_settings).await;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn resolve_conflict(
        &self,
        job_id: &str,
        direction: TransferDirection,
        source_path: &str,
        destination_path: &str,
        source_size: Option<u64>,
        source_mtime: Option<u64>,
        target_size: Option<u64>,
        target_mtime: Option<u64>,
        policy: ConflictPolicy,
        policy_override: &mut Option<ConflictPolicy>,
        cancel_rx: &mut watch::Receiver<bool>,
    ) -> Result<ResolvedAction, TransferFailure> {
        match policy {
            ConflictPolicy::Overwrite => Ok(ResolvedAction::Overwrite),
            ConflictPolicy::Skip => Ok(ResolvedAction::Skip),
            ConflictPolicy::Rename => Ok(ResolvedAction::Rename),
            ConflictPolicy::OverwriteIfNewer => {
                if is_source_newer(source_mtime, target_mtime) {
                    Ok(ResolvedAction::Overwrite)
                } else {
                    Ok(ResolvedAction::Skip)
                }
            }
            ConflictPolicy::Ask => {
                let conflict_id = Uuid::new_v4().to_string();
                let (tx, rx) = oneshot::channel();
                self.pending_conflicts
                    .write()
                    .await
                    .insert(conflict_id.clone(), tx);

                self.event_sink.emit_conflict(&TransferConflictPrompt {
                    conflict_id: conflict_id.clone(),
                    job_id: job_id.to_string(),
                    direction,
                    source_path: source_path.to_string(),
                    destination_path: destination_path.to_string(),
                    source_size,
                    source_modified_at_secs: source_mtime,
                    target_size,
                    target_modified_at_secs: target_mtime,
                });

                let decision = tokio::select! {
                    res = rx => {
                        match res {
                            Ok(val) => val,
                            Err(_) => {
                                self.pending_conflicts.write().await.remove(&conflict_id);
                                return Err(TransferFailure::Cancelled);
                            }
                        }
                    }
                    _ = wait_cancelled(cancel_rx) => {
                        self.pending_conflicts.write().await.remove(&conflict_id);
                        return Err(TransferFailure::Cancelled);
                    }
                };

                let (choice, apply_all) = decision;
                let mapped_policy = match choice {
                    ConflictDecision::Overwrite => Some(ConflictPolicy::Overwrite),
                    ConflictDecision::OverwriteIfNewer => Some(ConflictPolicy::OverwriteIfNewer),
                    ConflictDecision::Skip => Some(ConflictPolicy::Skip),
                    ConflictDecision::Rename => Some(ConflictPolicy::Rename),
                    ConflictDecision::Cancel => return Err(TransferFailure::Cancelled),
                };

                if apply_all {
                    *policy_override = mapped_policy;
                }

                match choice {
                    ConflictDecision::Overwrite => Ok(ResolvedAction::Overwrite),
                    ConflictDecision::Skip => Ok(ResolvedAction::Skip),
                    ConflictDecision::Rename => Ok(ResolvedAction::Rename),
                    ConflictDecision::OverwriteIfNewer => {
                        if is_source_newer(source_mtime, target_mtime) {
                            Ok(ResolvedAction::Overwrite)
                        } else {
                            Ok(ResolvedAction::Skip)
                        }
                    }
                    ConflictDecision::Cancel => Err(TransferFailure::Cancelled),
                }
            }
        }
    }

    async fn set_job_state(&self, job_id: &str, state: TransferState) {
        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.iter_mut().find(|j| j.id.0 == job_id) {
            job.state = state;
            job.updated_at_ms = now_ms();
            self.event_sink.emit_updated(job);
        }
    }

    async fn set_job_totals(&self, job_id: &str, total_bytes: Option<u64>, total_files: u64) {
        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.iter_mut().find(|j| j.id.0 == job_id) {
            job.total_bytes = total_bytes;
            job.total_files = total_files;
            job.updated_at_ms = now_ms();
            self.event_sink.emit_updated(job);
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn update_job_progress(
        &self,
        job_id: &str,
        transferred_bytes: u64,
        total_bytes: Option<u64>,
        speed_bps: u64,
        eta_seconds: Option<u64>,
        completed_files: u64,
        total_files: u64,
        current_file: Option<String>,
    ) {
        {
            let mut jobs = self.jobs.write().await;
            if let Some(job) = jobs.iter_mut().find(|j| j.id.0 == job_id) {
                job.transferred_bytes = transferred_bytes;
                if total_bytes.is_some() {
                    job.total_bytes = total_bytes;
                }
                job.speed_bps = speed_bps;
                job.eta_seconds = eta_seconds;
                job.completed_files = completed_files;
                job.total_files = total_files;
                job.current_file = current_file.clone();
                job.updated_at_ms = now_ms();
            }
        }

        self.event_sink.emit_progress(&TransferProgressEvent {
            job_id: job_id.to_string(),
            transferred_bytes,
            total_bytes,
            speed_bps,
            eta_seconds,
            completed_files,
            total_files,
            current_file,
        });
    }

    async fn finish_completed(&self, job_id: &str) {
        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.iter_mut().find(|j| j.id.0 == job_id) {
            job.state = TransferState::Completed;
            job.speed_bps = 0;
            job.eta_seconds = Some(0);
            job.completed_files = job.total_files;
            job.current_file = None;
            job.updated_at_ms = now_ms();
            self.event_sink.emit_updated(job);
        }
    }

    async fn finish_cancelled(&self, job_id: &str) {
        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.iter_mut().find(|j| j.id.0 == job_id) {
            job.state = TransferState::Cancelled;
            job.speed_bps = 0;
            job.eta_seconds = None;
            job.updated_at_ms = now_ms();
            self.event_sink.emit_updated(job);
        }
    }

    async fn finish_failed(&self, job_id: &str, message: String) {
        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.iter_mut().find(|j| j.id.0 == job_id) {
            job.state = TransferState::Failed {
                error_message: message,
            };
            job.speed_bps = 0;
            job.eta_seconds = None;
            job.updated_at_ms = now_ms();
            self.event_sink.emit_updated(job);
        }
    }
}

enum TransferFailure {
    Cancelled,
    Error(String),
}

enum ResolvedAction {
    Overwrite,
    Skip,
    Rename,
}

fn is_source_newer(source_mtime: Option<u64>, target_mtime: Option<u64>) -> bool {
    match (source_mtime, target_mtime) {
        (Some(s), Some(t)) => s > t,
        (Some(_), None) => true,
        _ => false,
    }
}

fn compute_eta(total_bytes: Option<u64>, transferred_bytes: u64, speed_bps: u64) -> Option<u64> {
    let total = total_bytes?;
    if speed_bps == 0 || transferred_bytes >= total {
        None
    } else {
        Some((total - transferred_bytes) / speed_bps)
    }
}

async fn wait_cancelled(rx: &mut watch::Receiver<bool>) {
    if *rx.borrow() {
        return;
    }
    while rx.changed().await.is_ok() {
        if *rx.borrow() {
            return;
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

async fn check_remote_exists(
    sftp: &SftpSession,
    path: &str,
    timeout: Duration,
) -> Result<Option<FileAttributes>, TransferFailure> {
    match tokio::time::timeout(timeout, sftp.symlink_metadata(path)).await {
        Ok(Ok(attrs)) => Ok(Some(attrs)),
        Ok(Err(SftpClientError::Status(s))) if s.status_code == StatusCode::NoSuchFile => Ok(None),
        Ok(Err(e)) => Err(TransferFailure::Error(
            classify_sftp_error(e, path).to_string(),
        )),
        Err(_) => Err(TransferFailure::Error(format!(
            "Timed out checking remote path '{}'",
            path
        ))),
    }
}

async fn find_unique_remote_name(
    sftp: &SftpSession,
    dest_path: &str,
    timeout: Duration,
) -> Result<String, TransferFailure> {
    for attempt in 1..=999 {
        let candidate = generate_renamed_path(dest_path, attempt);
        if check_remote_exists(sftp, &candidate, timeout)
            .await?
            .is_none()
        {
            return Ok(candidate);
        }
    }
    Err(TransferFailure::Error(format!(
        "Could not generate unique remote filename for '{}'",
        dest_path
    )))
}

async fn find_unique_local_name(dest_path: &Path) -> PathBuf {
    let base_str = dest_path.to_string_lossy().to_string();
    for attempt in 1..=999 {
        let candidate = PathBuf::from(generate_renamed_path(&base_str, attempt));
        if tokio::fs::symlink_metadata(&candidate).await.is_err() {
            return candidate;
        }
    }
    PathBuf::from(format!("{}.{}", base_str, Uuid::new_v4()))
}

async fn ensure_remote_dir(sftp: &SftpSession, dir: &str, timeout: Duration) -> Result<(), String> {
    let norm = normalize_posix_path(dir);
    if norm == "/" {
        return Ok(());
    }
    match tokio::time::timeout(timeout, sftp.metadata(norm.as_str())).await {
        Ok(Ok(attrs)) => {
            if attrs.is_dir() {
                return Ok(());
            }
            return Err(format!(
                "Remote path '{}' exists and is not a directory",
                norm
            ));
        }
        Ok(Err(SftpClientError::Status(s))) if s.status_code == StatusCode::NoSuchFile => {}
        Ok(Err(e)) => return Err(classify_sftp_error(e, &norm).to_string()),
        Err(_) => return Err(format!("Timed out checking remote directory '{}'", norm)),
    }

    match tokio::time::timeout(timeout, sftp.create_dir(norm.as_str())).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => {
            // Re-check in case another concurrent task created it first
            if let Ok(attrs) = sftp.metadata(norm.as_str()).await {
                if attrs.is_dir() {
                    return Ok(());
                }
            }
            Err(classify_sftp_error(e, &norm).to_string())
        }
        Err(_) => Err(format!("Timed out creating remote directory '{}'", norm)),
    }
}

async fn apply_remote_metadata(
    sftp: &SftpSession,
    remote_path: &str,
    mode: Option<u32>,
    mtime_secs: Option<u64>,
    ctx: &ActiveTransferContext,
) {
    let mut attrs = FileAttributes::default();
    let mut has_any = false;

    if ctx.transfer_settings.preserve_permissions && ctx.capabilities.can_set_permissions {
        if let Some(m) = mode {
            attrs.permissions = Some(m & 0o7777);
            has_any = true;
        }
    }

    if ctx.transfer_settings.preserve_timestamps && ctx.capabilities.can_preserve_timestamps {
        if let Some(mt) = mtime_secs {
            let clamped = mt.min(u32::MAX as u64) as u32;
            attrs.mtime = Some(clamped);
            attrs.atime = Some(clamped);
            has_any = true;
        }
    }

    if has_any {
        let _ =
            tokio::time::timeout(ctx.timeout_duration, sftp.set_metadata(remote_path, attrs)).await;
    }
}

async fn apply_local_metadata(
    local_path: &Path,
    mode: Option<u32>,
    mtime_secs: Option<u64>,
    settings: &ProfileTransferSettings,
) {
    if settings.preserve_permissions {
        if let Some(m) = mode {
            let perms = std::fs::Permissions::from_mode(m & 0o7777);
            let _ = tokio::fs::set_permissions(local_path, perms).await;
        }
    }

    if settings.preserve_timestamps {
        if let Some(mt) = mtime_secs {
            let _ = set_local_mtime(local_path, mt);
        }
    }
}

#[cfg(unix)]
fn set_local_mtime(path: &Path, mtime_secs: u64) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c_path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Null byte in path"))?;
    let times = [
        libc::timespec {
            tv_sec: 0,
            tv_nsec: libc::UTIME_OMIT,
        },
        libc::timespec {
            tv_sec: mtime_secs as libc::time_t,
            tv_nsec: 0,
        },
    ];
    let rc = unsafe { libc::utimensat(libc::AT_FDCWD, c_path.as_ptr(), times.as_ptr(), 0) };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

struct LocalWalkFile {
    abs_path: PathBuf,
    rel_path: String,
    size: u64,
    mtime_secs: Option<u64>,
    mode: Option<u32>,
}

async fn collect_local_tree(root: &Path) -> Result<(Vec<String>, Vec<LocalWalkFile>, u64), String> {
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    let mut total_bytes = 0u64;
    let mut stack: Vec<(PathBuf, String, usize)> = vec![(root.to_path_buf(), String::new(), 0)];

    while let Some((cur_abs, cur_rel, depth)) = stack.pop() {
        if depth > MAX_RECURSION_DEPTH {
            return Err(format!(
                "Directory recursion exceeded maximum depth of {}",
                MAX_RECURSION_DEPTH
            ));
        }

        let mut rd = tokio::fs::read_dir(&cur_abs).await.map_err(|e| {
            format!(
                "Failed to read local directory '{}': {}",
                cur_abs.display(),
                e
            )
        })?;

        while let Some(entry) = rd
            .next_entry()
            .await
            .map_err(|e| format!("Error reading directory entry: {}", e))?
        {
            let name = entry.file_name().to_string_lossy().to_string();
            let rel = if cur_rel.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", cur_rel, name)
            };
            let abs = entry.path();
            let meta = tokio::fs::symlink_metadata(&abs)
                .await
                .map_err(|e| format!("Failed to stat '{}': {}", abs.display(), e))?;

            if meta.file_type().is_symlink() {
                // Skip symlinks during recursive walk to avoid circular loops
                continue;
            }
            if meta.is_dir() {
                dirs.push(rel.clone());
                stack.push((abs, rel, depth + 1));
            } else if meta.is_file() {
                let sz = meta.len();
                total_bytes += sz;
                let mtime_secs = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs());
                files.push(LocalWalkFile {
                    abs_path: abs,
                    rel_path: rel,
                    size: sz,
                    mtime_secs,
                    mode: Some(meta.mode()),
                });
            }
        }
    }

    Ok((dirs, files, total_bytes))
}

struct RemoteWalkFile {
    abs_path: String,
    rel_path: String,
    size: Option<u64>,
    mtime_secs: Option<u64>,
    mode: Option<u32>,
}

async fn collect_remote_tree(
    sftp: &SftpSession,
    root: &str,
    timeout: Duration,
) -> Result<(Vec<String>, Vec<RemoteWalkFile>, u64), String> {
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    let mut total_bytes = 0u64;
    let mut stack: Vec<(String, String, usize)> = vec![(root.to_string(), String::new(), 0)];

    while let Some((cur_abs, cur_rel, depth)) = stack.pop() {
        if depth > MAX_RECURSION_DEPTH {
            return Err(format!(
                "Remote directory recursion exceeded maximum depth of {}",
                MAX_RECURSION_DEPTH
            ));
        }

        let rd = tokio::time::timeout(timeout, sftp.read_dir(cur_abs.as_str()))
            .await
            .map_err(|_| format!("Timed out reading remote directory '{}'", cur_abs))?
            .map_err(|e| classify_sftp_error(e, &cur_abs).to_string())?;

        for item in rd {
            let name = item.file_name();
            let rel = if cur_rel.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", cur_rel, name)
            };
            let abs = join_posix_path(&cur_abs, &name);
            let attrs = item.metadata();

            if attrs.is_symlink() {
                continue;
            }
            if attrs.is_dir() {
                dirs.push(rel.clone());
                stack.push((abs, rel, depth + 1));
            } else {
                if let Some(sz) = attrs.size {
                    total_bytes += sz;
                }
                files.push(RemoteWalkFile {
                    abs_path: abs,
                    rel_path: rel,
                    size: attrs.size,
                    mtime_secs: attrs.mtime.map(u64::from),
                    mode: attrs.permissions,
                });
            }
        }
    }

    Ok((dirs, files, total_bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AuthMethod, ConnectionProfile, ProtocolType};
    use crate::protocol::sftp::ConnectOutcome;

    #[test]
    fn test_generate_renamed_path() {
        assert_eq!(generate_renamed_path("report.pdf", 1), "report (1).pdf");
        assert_eq!(
            generate_renamed_path("/var/www/index.html", 2),
            "/var/www/index (2).html"
        );
        assert_eq!(generate_renamed_path("/tmp/README", 1), "/tmp/README (1)");
        assert_eq!(generate_renamed_path("/.bashrc", 1), "/.bashrc (1)");
    }

    #[test]
    fn test_compute_eta_and_is_source_newer() {
        assert_eq!(compute_eta(Some(1000), 250, 250), Some(3));
        assert_eq!(compute_eta(Some(1000), 1000, 250), None);
        assert_eq!(compute_eta(Some(1000), 250, 0), None);
        assert_eq!(compute_eta(None, 250, 250), None);

        assert!(is_source_newer(Some(200), Some(100)));
        assert!(!is_source_newer(Some(100), Some(200)));
        assert!(is_source_newer(Some(100), None));
        assert!(!is_source_newer(None, Some(100)));
    }

    async fn wait_for_terminal(
        manager: &TransferManager,
        job_id: &str,
        timeout: Duration,
    ) -> TransferJob {
        let start = Instant::now();
        loop {
            let jobs = manager.list_transfers().await;
            if let Some(job) = jobs.into_iter().find(|j| j.id.0 == job_id) {
                if job.state.is_terminal() {
                    return job;
                }
            }
            if start.elapsed() > timeout {
                panic!("Timed out waiting for transfer job {} to finish", job_id);
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    #[tokio::test]
    async fn test_live_sftp_transfers_small_large_recursion_conflicts_and_cancel() {
        let test_host = "192.168.122.10";
        let test_port = 22u16;

        if tokio::time::timeout(
            Duration::from_secs(2),
            tokio::net::TcpStream::connect((test_host, test_port)),
        )
        .await
        .is_err()
        {
            return;
        }

        let session_manager = SessionManager::new();
        let profile = ConnectionProfile::new(
            "Ubuntu Test VM".to_string(),
            ProtocolType::Sftp,
            test_host.to_string(),
            "ubuntu".to_string(),
            AuthMethod::PublicKey {
                private_key_path: "~/.ssh/id_rsa".to_string(),
                requires_passphrase: false,
            },
        );

        match session_manager.connect(profile, None, false).await {
            Ok(ConnectOutcome::Connected { .. }) => {}
            _ => return,
        }

        let (sink, mut event_rx) = ChannelTransferEventSink::new();
        let transfer_manager = TransferManager::new(session_manager.clone(), Arc::new(sink));

        let temp_base = std::env::temp_dir().join(format!("linuxscp_xfer_test_{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_base).await.unwrap();

        let ctx = session_manager.active_transfer_context().await.unwrap();
        let sftp = ctx.sftp.as_ref().unwrap();
        let remote_test_dir = format!("/tmp/linuxscp_xfer_test_{}", Uuid::new_v4());
        sftp.create_dir(remote_test_dir.as_str()).await.unwrap();

        // 1. Small file upload + download
        let small_local = temp_base.join("hello.txt");
        let small_content = b"Hello LinuxSCP SFTP Transfer!\n";
        tokio::fs::write(&small_local, small_content).await.unwrap();

        let uploaded = transfer_manager
            .enqueue_uploads(
                vec![small_local.to_string_lossy().to_string()],
                remote_test_dir.clone(),
                ConflictPolicy::Overwrite,
            )
            .await
            .unwrap();
        let up_job = wait_for_terminal(
            &transfer_manager,
            &uploaded[0].id.0,
            Duration::from_secs(15),
        )
        .await;
        assert_eq!(up_job.state, TransferState::Completed);
        assert_eq!(up_job.transferred_bytes, small_content.len() as u64);

        let dl_dir = temp_base.join("downloads");
        tokio::fs::create_dir_all(&dl_dir).await.unwrap();
        let downloaded = transfer_manager
            .enqueue_downloads(
                vec![format!("{}/hello.txt", remote_test_dir)],
                dl_dir.to_string_lossy().to_string(),
                ConflictPolicy::Overwrite,
            )
            .await
            .unwrap();
        let dl_job = wait_for_terminal(
            &transfer_manager,
            &downloaded[0].id.0,
            Duration::from_secs(15),
        )
        .await;
        assert_eq!(dl_job.state, TransferState::Completed);
        assert_eq!(
            tokio::fs::read(dl_dir.join("hello.txt")).await.unwrap(),
            small_content
        );

        // 2. Conflict handling: Skip, Rename, and Ask
        tokio::fs::write(&small_local, b"NEW CONTENT")
            .await
            .unwrap();
        let skip_jobs = transfer_manager
            .enqueue_uploads(
                vec![small_local.to_string_lossy().to_string()],
                remote_test_dir.clone(),
                ConflictPolicy::Skip,
            )
            .await
            .unwrap();
        let skip_done = wait_for_terminal(
            &transfer_manager,
            &skip_jobs[0].id.0,
            Duration::from_secs(10),
        )
        .await;
        assert_eq!(skip_done.state, TransferState::Completed);

        let rename_jobs = transfer_manager
            .enqueue_uploads(
                vec![small_local.to_string_lossy().to_string()],
                remote_test_dir.clone(),
                ConflictPolicy::Rename,
            )
            .await
            .unwrap();
        let rename_done = wait_for_terminal(
            &transfer_manager,
            &rename_jobs[0].id.0,
            Duration::from_secs(10),
        )
        .await;
        assert_eq!(rename_done.state, TransferState::Completed);
        assert!(sftp
            .try_exists(format!("{}/hello (1).txt", remote_test_dir))
            .await
            .unwrap());

        // Conflict Ask -> resolves via resolve_transfer_conflict
        let ask_jobs = transfer_manager
            .enqueue_uploads(
                vec![small_local.to_string_lossy().to_string()],
                remote_test_dir.clone(),
                ConflictPolicy::Ask,
            )
            .await
            .unwrap();
        let ask_id = ask_jobs[0].id.0.clone();

        let conflict_prompt = tokio::time::timeout(Duration::from_secs(5), async {
            while let Some(ev) = event_rx.recv().await {
                if let CapturedTransferEvent::Conflict(c) = ev {
                    if c.job_id == ask_id {
                        return c;
                    }
                }
            }
            panic!("Did not receive conflict event");
        })
        .await
        .unwrap();

        transfer_manager
            .resolve_transfer_conflict(
                &conflict_prompt.conflict_id,
                ConflictDecision::Overwrite,
                false,
            )
            .await
            .unwrap();
        let ask_done = wait_for_terminal(&transfer_manager, &ask_id, Duration::from_secs(10)).await;
        assert_eq!(ask_done.state, TransferState::Completed);

        // 3. Large file (2 MB) streaming upload + progress events check
        let large_local = temp_base.join("large_2mb.bin");
        let large_data = vec![0xABu8; 2 * 1024 * 1024];
        tokio::fs::write(&large_local, &large_data).await.unwrap();

        let large_jobs = transfer_manager
            .enqueue_uploads(
                vec![large_local.to_string_lossy().to_string()],
                remote_test_dir.clone(),
                ConflictPolicy::Overwrite,
            )
            .await
            .unwrap();
        let large_id = large_jobs[0].id.0.clone();
        let large_done =
            wait_for_terminal(&transfer_manager, &large_id, Duration::from_secs(30)).await;
        assert_eq!(large_done.state, TransferState::Completed);
        assert_eq!(large_done.transferred_bytes, large_data.len() as u64);

        // Verify progress events were emitted with matching job_id
        let mut saw_large_progress = false;
        while let Ok(ev) = event_rx.try_recv() {
            if let CapturedTransferEvent::Progress(p) = ev {
                if p.job_id == large_id {
                    saw_large_progress = true;
                }
            }
        }
        assert!(saw_large_progress);

        // 4. Recursive directory upload + download
        let tree_src = temp_base.join("tree_src");
        tokio::fs::create_dir_all(tree_src.join("nested"))
            .await
            .unwrap();
        tokio::fs::write(tree_src.join("a.txt"), b"alpha")
            .await
            .unwrap();
        tokio::fs::write(tree_src.join("nested/b.txt"), b"beta")
            .await
            .unwrap();

        let dir_up = transfer_manager
            .enqueue_uploads(
                vec![tree_src.to_string_lossy().to_string()],
                remote_test_dir.clone(),
                ConflictPolicy::Overwrite,
            )
            .await
            .unwrap();
        let dir_up_done =
            wait_for_terminal(&transfer_manager, &dir_up[0].id.0, Duration::from_secs(15)).await;
        assert_eq!(dir_up_done.state, TransferState::Completed);
        assert_eq!(dir_up_done.total_files, 2);
        assert_eq!(dir_up_done.completed_files, 2);

        let tree_dl_dest = temp_base.join("tree_dl");
        tokio::fs::create_dir_all(&tree_dl_dest).await.unwrap();
        let dir_dl = transfer_manager
            .enqueue_downloads(
                vec![format!("{}/tree_src", remote_test_dir)],
                tree_dl_dest.to_string_lossy().to_string(),
                ConflictPolicy::Overwrite,
            )
            .await
            .unwrap();
        let dir_dl_done =
            wait_for_terminal(&transfer_manager, &dir_dl[0].id.0, Duration::from_secs(15)).await;
        assert_eq!(dir_dl_done.state, TransferState::Completed);
        assert_eq!(
            tokio::fs::read_to_string(tree_dl_dest.join("tree_src/nested/b.txt"))
                .await
                .unwrap(),
            "beta"
        );

        // 5. Cancellation cleans up temp files
        let cancel_jobs = transfer_manager
            .enqueue_downloads(
                vec![format!("{}/large_2mb.bin", remote_test_dir)],
                dl_dir.to_string_lossy().to_string(),
                ConflictPolicy::Overwrite,
            )
            .await
            .unwrap();
        let cancel_id = cancel_jobs[0].id.0.clone();
        transfer_manager.cancel_transfer(&cancel_id).await.unwrap();
        let cancel_done =
            wait_for_terminal(&transfer_manager, &cancel_id, Duration::from_secs(10)).await;
        assert_eq!(cancel_done.state, TransferState::Cancelled);

        // Verify no .linuxscp-tmp-* files left in dl_dir
        let mut rd = tokio::fs::read_dir(&dl_dir).await.unwrap();
        while let Some(entry) = rd.next_entry().await.unwrap() {
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(
                !name.contains(".linuxscp-tmp-"),
                "Orphaned temp file remained: {}",
                name
            );
        }

        // Cleanup remote & local test artifacts
        let _ = sftp
            .remove_file(format!("{}/hello.txt", remote_test_dir))
            .await;
        let _ = sftp
            .remove_file(format!("{}/hello (1).txt", remote_test_dir))
            .await;
        let _ = sftp
            .remove_file(format!("{}/large_2mb.bin", remote_test_dir))
            .await;
        let _ = sftp
            .remove_file(format!("{}/tree_src/nested/b.txt", remote_test_dir))
            .await;
        let _ = sftp
            .remove_dir(format!("{}/tree_src/nested", remote_test_dir))
            .await;
        let _ = sftp
            .remove_file(format!("{}/tree_src/a.txt", remote_test_dir))
            .await;
        let _ = sftp
            .remove_dir(format!("{}/tree_src", remote_test_dir))
            .await;
        let _ = sftp.remove_dir(remote_test_dir.as_str()).await;
        let _ = tokio::fs::remove_dir_all(&temp_base).await;
        let _ = session_manager.disconnect().await;
    }
}
