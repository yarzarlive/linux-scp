//! Remote editing session manager.
//!
//! Owns the lifecycle of managed local working copies of remote files: download,
//! local change detection, conflict-checked upload, and documented cleanup. All remote
//! I/O goes through [`RemoteEditBackend`] so the safety logic is protocol-independent
//! and unit-testable without a server.

use super::types::{
    compare_fingerprints, FingerprintComparison, RemoteEditSession, RemoteEditState,
    RemoteEditUploadMode, RemoteEditUploadOutcome, RemoteEndpointIdentity, RemoteFileFingerprint,
    RemoteFileStat, RemoteObjectKind, UploadFailureKind,
};
use super::workspace::{
    copy_private, ensure_private_dir, local_stat, read_manifest, remove_session_dir, sha256_file,
    validate_local_file_name, validate_session_id, write_manifest, LocalStat, SessionPaths,
    MANIFEST_FILE,
};
use crate::errors::AppError;
use crate::transfer::generate_renamed_path;
use async_trait::async_trait;
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Files up to this size get a content-hash re-verification before upload, which
/// catches remote edits that kept the same size within the same mtime second.
pub const DEFAULT_VERIFY_HASH_MAX_BYTES: u64 = 16 * 1024 * 1024;

/// Error classes the manager needs to distinguish when talking to a remote backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    /// Not connected, or the connection was lost / timed out.
    Disconnected(String),
    /// The remote path does not exist.
    NotFound(String),
    /// Any other failure (permissions, protocol error, local I/O).
    Other(AppError),
}

impl BackendError {
    pub fn message(&self) -> String {
        match self {
            Self::Disconnected(m) | Self::NotFound(m) => m.clone(),
            Self::Other(e) => e.to_string(),
        }
    }
}

impl From<BackendError> for AppError {
    fn from(err: BackendError) -> Self {
        match err {
            BackendError::Disconnected(m) => AppError::Network(m),
            BackendError::NotFound(m) => AppError::Filesystem(m),
            BackendError::Other(e) => e,
        }
    }
}

/// Protocol operations required for remote editing.
#[async_trait]
pub trait RemoteEditBackend: Send + Sync {
    /// Identity of the currently active remote session, plus its saved profile id.
    async fn current_endpoint(
        &self,
    ) -> Result<(RemoteEndpointIdentity, Option<String>), BackendError>;
    /// Resolves symlinks and relative components into the canonical remote path.
    async fn canonicalize(&self, path: &str) -> Result<String, BackendError>;
    /// Stats a remote path, following symlinks.
    async fn stat(&self, path: &str) -> Result<RemoteFileStat, BackendError>;
    /// Streams a remote file into a new local file at `dest`; returns bytes written.
    async fn download_to(&self, path: &str, dest: &Path) -> Result<u64, BackendError>;
    /// Uploads `src` to a temporary remote name, applies `mode` if given, then replaces
    /// the remote file. Must never leave a partially written file at `path`.
    async fn upload_replace(
        &self,
        src: &Path,
        path: &str,
        mode: Option<u32>,
    ) -> Result<(), BackendError>;
}

/// Receives session lifecycle notifications (Tauri events in production).
pub trait RemoteEditEventSink: Send + Sync {
    fn emit_updated(&self, session: &RemoteEditSession);
    fn emit_removed(&self, session_id: &str);
}

/// Event sink that drops all notifications.
pub struct NoopRemoteEditEventSink;

impl RemoteEditEventSink for NoopRemoteEditEventSink {
    fn emit_updated(&self, _session: &RemoteEditSession) {}
    fn emit_removed(&self, _session_id: &str) {}
}

#[derive(Debug, Clone)]
pub struct RemoteEditConfig {
    pub verify_hash_max_bytes: u64,
    /// Consecutive polls the working copy must be missing before it is reported missing
    /// (editors that save via rename briefly remove the file).
    pub missing_polls_threshold: u32,
}

impl Default for RemoteEditConfig {
    fn default() -> Self {
        Self {
            verify_hash_max_bytes: DEFAULT_VERIFY_HASH_MAX_BYTES,
            missing_polls_threshold: 2,
        }
    }
}

struct Entry {
    session: RemoteEditSession,
    /// Last observed local stat; `None` forces re-observation.
    observed: Option<LocalStat>,
    /// Stat changed and the content still needs hashing once it is stable.
    pending_hash: bool,
    missing_polls: u32,
}

/// Thread-safe manager of all remote editing sessions.
#[derive(Clone)]
pub struct RemoteEditManager {
    backend: Arc<dyn RemoteEditBackend>,
    sink: Arc<dyn RemoteEditEventSink>,
    root: PathBuf,
    config: RemoteEditConfig,
    upload_mode: Arc<Mutex<RemoteEditUploadMode>>,
    entries: Arc<Mutex<HashMap<String, Entry>>>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

async fn hash_file_blocking(path: PathBuf) -> std::io::Result<String> {
    tokio::task::spawn_blocking(move || sha256_file(&path))
        .await
        .map_err(|e| std::io::Error::other(format!("hash task failed: {}", e)))?
}

fn failure_kind(err: &BackendError) -> UploadFailureKind {
    match err {
        BackendError::Disconnected(_) => UploadFailureKind::Disconnected,
        _ => UploadFailureKind::RemoteError,
    }
}

impl RemoteEditManager {
    pub fn new(
        backend: Arc<dyn RemoteEditBackend>,
        sink: Arc<dyn RemoteEditEventSink>,
        root: PathBuf,
        config: RemoteEditConfig,
    ) -> Self {
        Self::new_with_mode(backend, sink, root, config, RemoteEditUploadMode::Prompt)
    }

    pub fn new_with_mode(
        backend: Arc<dyn RemoteEditBackend>,
        sink: Arc<dyn RemoteEditEventSink>,
        root: PathBuf,
        config: RemoteEditConfig,
        initial_mode: RemoteEditUploadMode,
    ) -> Self {
        Self {
            backend,
            sink,
            root,
            config,
            upload_mode: Arc::new(Mutex::new(initial_mode)),
            entries: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn upload_mode(&self) -> RemoteEditUploadMode {
        *self.upload_mode.lock().await
    }

    pub async fn set_upload_mode(&self, mode: RemoteEditUploadMode) {
        *self.upload_mode.lock().await = mode;
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn paths(&self, id: &str) -> SessionPaths {
        SessionPaths::new(&self.root, id)
    }

    fn persist(&self, session: &RemoteEditSession) {
        if let Err(e) = write_manifest(&self.paths(&session.id), session) {
            tracing::warn!(session_id = %session.id, error = %e, "Failed to persist remote edit manifest");
        }
    }

    fn publish(&self, session: &RemoteEditSession) {
        self.persist(session);
        self.sink.emit_updated(session);
    }

    /// Lists all sessions, oldest first.
    pub async fn list(&self) -> Vec<RemoteEditSession> {
        let guard = self.entries.lock().await;
        let mut list: Vec<RemoteEditSession> = guard.values().map(|e| e.session.clone()).collect();
        list.sort_by(|a, b| a.created_at_ms.cmp(&b.created_at_ms).then(a.id.cmp(&b.id)));
        list
    }

    pub async fn get(&self, id: &str) -> Result<RemoteEditSession, AppError> {
        let guard = self.entries.lock().await;
        guard
            .get(id)
            .map(|e| e.session.clone())
            .ok_or_else(|| not_found(id))
    }

    /// Downloads `remote_path` into a new managed working copy, or returns the existing
    /// session for the same endpoint and file. Existing sessions with local changes are
    /// never re-downloaded; clean ones are refreshed if the remote file changed.
    pub async fn open_session(&self, remote_path: &str) -> Result<RemoteEditSession, AppError> {
        let trimmed = remote_path.trim();
        if trimmed.is_empty() || trimmed.contains('\0') {
            return Err(AppError::Validation(
                "A remote file path is required.".to_string(),
            ));
        }

        let (endpoint, profile_id) = self.backend.current_endpoint().await?;
        let canonical = self.backend.canonicalize(trimmed).await?;
        let file_name = validate_local_file_name(&canonical)?;

        let existing_id = {
            let guard = self.entries.lock().await;
            guard
                .values()
                .find(|e| {
                    e.session.endpoint.matches(&endpoint) && e.session.remote_path == canonical
                })
                .map(|e| e.session.id.clone())
        };
        if let Some(id) = existing_id {
            return self.refresh_existing(&id).await;
        }

        let stat = self.backend.stat(&canonical).await?;
        ensure_regular(&stat, &canonical)?;

        ensure_private_dir(&self.root)?;
        let id = Uuid::new_v4().to_string();
        let paths = self.paths(&id);
        paths.create()?;
        let work_path = paths.work_dir.join(&file_name);

        let (sha, fingerprint) = match self
            .download_verified(&canonical, &paths, &work_path, &stat.fingerprint)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                let _ = remove_session_dir(&self.root, &id);
                return Err(e);
            }
        };

        let now = now_ms();
        let session = RemoteEditSession {
            id: id.clone(),
            endpoint,
            profile_id,
            remote_path: canonical,
            file_name,
            local_path: work_path.to_string_lossy().into_owned(),
            app_id: None,
            app_name: None,
            baseline: fingerprint,
            baseline_sha256: sha.clone(),
            local_sha256: Some(sha),
            state: RemoteEditState::Clean,
            prompt_dismissed: false,
            has_unsynced_changes: false,
            created_at_ms: now,
            last_synced_at_ms: now,
            upload_count: 0,
            recovered: false,
        };

        if let Err(e) = write_manifest(&paths, &session) {
            let _ = remove_session_dir(&self.root, &id);
            return Err(e);
        }

        let observed = local_stat(&work_path);
        self.entries.lock().await.insert(
            id,
            Entry {
                session: session.clone(),
                observed,
                pending_hash: false,
                missing_polls: 0,
            },
        );
        self.sink.emit_updated(&session);
        Ok(session)
    }

    async fn refresh_existing(&self, id: &str) -> Result<RemoteEditSession, AppError> {
        // Ensure any recent local saves are detected first.
        let _ = self.scan_local_changes(false).await;
        let session = self.get(id).await?;
        let work_path = PathBuf::from(&session.local_path);
        let paths = self.paths(id);

        if session.may_lose_local_work() || session.state != RemoteEditState::Clean {
            // Never overwrite a local working copy that holds unsynced edits,
            // but check whether the remote file has also changed so conflicts are surfaced immediately.
            if let Ok(Some((reason, current))) =
                self.verify_remote_unchanged(&session, &paths).await
            {
                if !matches!(session.state, RemoteEditState::Conflict { .. }) {
                    if let Some(updated) = self
                        .set_state(
                            id,
                            RemoteEditState::Conflict {
                                reason,
                                remote_current: current,
                            },
                        )
                        .await
                    {
                        return Ok(updated);
                    }
                }
            }
            return Ok(session);
        }

        // Session is Clean: check if the remote file changed (fingerprint or content hash).
        if matches!(
            self.verify_remote_unchanged(&session, &paths).await,
            Ok(None)
        ) {
            return Ok(session);
        }

        let stat = self.backend.stat(&session.remote_path).await?;
        ensure_regular(&stat, &session.remote_path)?;
        let (sha, fingerprint) = self
            .download_verified(&session.remote_path, &paths, &work_path, &stat.fingerprint)
            .await?;

        let mut guard = self.entries.lock().await;
        let entry = guard.get_mut(id).ok_or_else(|| not_found(id))?;
        entry.session.baseline = fingerprint;
        entry.session.baseline_sha256 = sha.clone();
        entry.session.local_sha256 = Some(sha);
        entry.session.last_synced_at_ms = now_ms();
        entry.session.state = RemoteEditState::Clean;
        entry.session.refresh_derived();
        entry.observed = local_stat(&work_path);
        entry.pending_hash = false;
        let updated = entry.session.clone();
        drop(guard);
        self.publish(&updated);
        Ok(updated)
    }

    /// Replaces the local working copy with the latest version from the remote server,
    /// resetting the session baseline and clearing any local modifications or conflicts.
    pub async fn reload_from_remote(&self, id: &str) -> Result<RemoteEditSession, AppError> {
        validate_session_id(id)?;
        let session = self.get(id).await?;
        if session.state == RemoteEditState::Uploading {
            return Err(AppError::Validation(format!(
                "'{}' is currently being uploaded.",
                session.file_name
            )));
        }
        let (endpoint, _) = self.backend.current_endpoint().await?;
        if !endpoint.matches(&session.endpoint) {
            return Err(AppError::Validation(format!(
                "The active connection is {}, but this file belongs to {}.",
                endpoint.display(),
                session.endpoint.display()
            )));
        }

        let stat = self.backend.stat(&session.remote_path).await?;
        ensure_regular(&stat, &session.remote_path)?;
        let paths = self.paths(id);
        let work_path = PathBuf::from(&session.local_path);
        let (sha, fingerprint) = self
            .download_verified(&session.remote_path, &paths, &work_path, &stat.fingerprint)
            .await?;

        let mut guard = self.entries.lock().await;
        let entry = guard.get_mut(id).ok_or_else(|| not_found(id))?;
        entry.session.baseline = fingerprint;
        entry.session.baseline_sha256 = sha.clone();
        entry.session.local_sha256 = Some(sha);
        entry.session.last_synced_at_ms = now_ms();
        entry.session.state = RemoteEditState::Clean;
        entry.session.prompt_dismissed = false;
        entry.session.refresh_derived();
        entry.observed = local_stat(&work_path);
        entry.pending_hash = false;
        let updated = entry.session.clone();
        drop(guard);
        self.publish(&updated);
        Ok(updated)
    }

    /// Downloads to a scratch file, verifies size and that the remote did not change
    /// during the transfer, hashes, and atomically moves it into place.
    async fn download_verified(
        &self,
        remote_path: &str,
        paths: &SessionPaths,
        work_path: &Path,
        expected: &RemoteFileFingerprint,
    ) -> Result<(String, RemoteFileFingerprint), AppError> {
        let scratch = paths.scratch_file("download");
        let result: Result<(String, RemoteFileFingerprint), AppError> = async {
            let bytes = self.backend.download_to(remote_path, &scratch).await?;
            if let Some(size) = expected.size {
                if bytes != size {
                    return Err(AppError::Protocol(format!(
                        "Downloaded {} bytes of '{}' but the server reported {} bytes. The remote file may have changed during download; try again.",
                        bytes, remote_path, size
                    )));
                }
            }
            let after = self.backend.stat(remote_path).await?;
            if let FingerprintComparison::Changed { .. } =
                compare_fingerprints(expected, &after.fingerprint)
            {
                return Err(AppError::Protocol(format!(
                    "Remote file '{}' changed while it was being downloaded. Try opening it again.",
                    remote_path
                )));
            }
            fs::set_permissions(&scratch, fs::Permissions::from_mode(0o600))?;
            let sha = hash_file_blocking(scratch.clone()).await?;
            fs::rename(&scratch, work_path).map_err(|e| {
                AppError::Filesystem(format!(
                    "Failed to place working copy at '{}': {}",
                    work_path.display(),
                    e
                ))
            })?;
            Ok((sha, after.fingerprint))
        }
        .await;
        if result.is_err() {
            let _ = fs::remove_file(&scratch);
        }
        result
    }

    /// Records which application the working copy was opened with.
    pub async fn record_launch(
        &self,
        id: &str,
        app_id: &str,
        app_name: &str,
    ) -> Result<RemoteEditSession, AppError> {
        let mut guard = self.entries.lock().await;
        let entry = guard.get_mut(id).ok_or_else(|| not_found(id))?;
        entry.session.app_id = Some(app_id.to_string());
        entry.session.app_name = Some(app_name.to_string());
        let updated = entry.session.clone();
        drop(guard);
        self.publish(&updated);
        Ok(updated)
    }

    /// Inspects every working copy for local modifications.
    ///
    /// Detection is content-based and independent of the editor process: a stat change
    /// marks the file as pending, and once the stat is stable across two polls
    /// (`require_stable == true`) the content is hashed and compared with the baseline.
    /// Saves that do not change content (e.g. touch, save without edits) stay `Clean`.
    /// Returns the number of sessions whose state changed.
    pub async fn scan_local_changes(&self, require_stable: bool) -> usize {
        struct Target {
            id: String,
            path: PathBuf,
            observed: Option<LocalStat>,
            pending: bool,
        }

        let targets: Vec<Target> = {
            let guard = self.entries.lock().await;
            guard
                .values()
                .filter(|e| e.session.state != RemoteEditState::Uploading)
                .map(|e| Target {
                    id: e.session.id.clone(),
                    path: PathBuf::from(&e.session.local_path),
                    observed: e.observed,
                    pending: e.pending_hash,
                })
                .collect()
        };
        if targets.is_empty() {
            return 0;
        }

        let paths: Vec<PathBuf> = targets.iter().map(|t| t.path.clone()).collect();
        let stats: Vec<Option<LocalStat>> =
            tokio::task::spawn_blocking(move || paths.iter().map(|p| local_stat(p)).collect())
                .await
                .unwrap_or_default();

        let mut changed = 0usize;
        let mut to_hash: Vec<(String, PathBuf, LocalStat)> = Vec::new();
        {
            let mut guard = self.entries.lock().await;
            for (target, stat) in targets.iter().zip(stats.iter()) {
                let Some(entry) = guard.get_mut(&target.id) else {
                    continue;
                };
                if entry.session.state == RemoteEditState::Uploading {
                    continue;
                }
                match stat {
                    None => {
                        entry.missing_polls += 1;
                        entry.observed = None;
                        entry.pending_hash = true;
                        if entry.missing_polls >= self.config.missing_polls_threshold
                            && entry.session.state != RemoteEditState::LocalMissing
                        {
                            entry.session.state = RemoteEditState::LocalMissing;
                            entry.session.prompt_dismissed = false;
                            let s = entry.session.clone();
                            self.publish(&s);
                            changed += 1;
                        }
                    }
                    Some(st) => {
                        entry.missing_polls = 0;
                        let stat_changed = target.observed != Some(*st);
                        if stat_changed {
                            entry.observed = Some(*st);
                            entry.pending_hash = true;
                            if !require_stable {
                                to_hash.push((target.id.clone(), target.path.clone(), *st));
                            }
                        } else if target.pending || entry.pending_hash {
                            to_hash.push((target.id.clone(), target.path.clone(), *st));
                        }
                    }
                }
            }
        }

        let mode = self.upload_mode().await;
        let mut auto_upload_ids: Vec<String> = Vec::new();

        for (id, path, st) in to_hash {
            let hash = match hash_file_blocking(path.clone()).await {
                Ok(h) => h,
                Err(e) => {
                    tracing::warn!(session_id = %id, error = %e, "Could not hash remote edit working copy");
                    continue;
                }
            };
            let mut guard = self.entries.lock().await;
            let Some(entry) = guard.get_mut(&id) else {
                continue;
            };
            if entry.session.state == RemoteEditState::Uploading || entry.observed != Some(st) {
                // Changed again while hashing; the next poll will pick it up.
                continue;
            }
            entry.pending_hash = false;
            let previous_local = entry.session.local_sha256.clone();
            let previous_state = entry.session.state.clone();
            entry.session.local_sha256 = Some(hash.clone());
            entry.session.refresh_derived();

            let content_changed = previous_local.as_deref() != Some(hash.as_str());
            if hash == entry.session.baseline_sha256 {
                entry.session.state = RemoteEditState::Clean;
                entry.session.prompt_dismissed = false;
            } else {
                match previous_state {
                    RemoteEditState::Clean
                    | RemoteEditState::LocalMissing
                    | RemoteEditState::Modified => {
                        entry.session.state = RemoteEditState::Modified;
                    }
                    // Conflicts and failures still require an explicit decision.
                    _ => {}
                }
                if content_changed {
                    entry.session.prompt_dismissed = false;
                }
            }

            let should_auto_upload = mode == RemoteEditUploadMode::Auto
                && content_changed
                && entry.session.state == RemoteEditState::Modified
                && !entry.session.prompt_dismissed;

            if content_changed || entry.session.state != previous_state {
                let s = entry.session.clone();
                drop(guard);
                self.publish(&s);
                changed += 1;
                if should_auto_upload {
                    auto_upload_ids.push(id);
                }
            }
        }

        for id in auto_upload_ids {
            let _ = self.upload(&id, false).await;
        }

        changed
    }

    async fn set_state(&self, id: &str, state: RemoteEditState) -> Option<RemoteEditSession> {
        let mut guard = self.entries.lock().await;
        let entry = guard.get_mut(id)?;
        entry.session.state = state;
        entry.session.prompt_dismissed = false;
        let s = entry.session.clone();
        drop(guard);
        self.publish(&s);
        Some(s)
    }

    async fn fail(
        &self,
        id: &str,
        kind: UploadFailureKind,
        message: String,
    ) -> Result<RemoteEditUploadOutcome, AppError> {
        let session = self
            .set_state(id, RemoteEditState::UploadFailed { message, kind })
            .await
            .ok_or_else(|| not_found(id))?;
        Ok(RemoteEditUploadOutcome::Failed { session })
    }

    async fn conflict(
        &self,
        id: &str,
        reason: String,
        remote_current: Option<RemoteFileFingerprint>,
    ) -> Result<RemoteEditUploadOutcome, AppError> {
        let session = self
            .set_state(
                id,
                RemoteEditState::Conflict {
                    reason,
                    remote_current,
                },
            )
            .await
            .ok_or_else(|| not_found(id))?;
        Ok(RemoteEditUploadOutcome::Conflict { session })
    }

    /// Uploads the working copy after the user explicitly asked for it.
    ///
    /// - Unchanged content is never uploaded.
    /// - The active connection must belong to the same endpoint the file came from.
    /// - Unless `overwrite_remote_changes` is set, the remote fingerprint (and, for files
    ///   up to `verify_hash_max_bytes`, the remote content hash) must equal the baseline;
    ///   otherwise the session enters `Conflict` and nothing is written.
    /// - `overwrite_remote_changes` is only accepted for a session already in `Conflict`,
    ///   i.e. after the user has seen the conflict.
    /// - Failures leave the working copy untouched.
    pub async fn upload(
        &self,
        id: &str,
        overwrite_remote_changes: bool,
    ) -> Result<RemoteEditUploadOutcome, AppError> {
        validate_session_id(id)?;
        let paths = self.paths(id);

        // Phase 1: validate and claim the session.
        let session = {
            let guard = self.entries.lock().await;
            let entry = guard.get(id).ok_or_else(|| not_found(id))?;
            match &entry.session.state {
                RemoteEditState::Uploading => {
                    return Err(AppError::Validation(format!(
                        "'{}' is already being uploaded.",
                        entry.session.file_name
                    )))
                }
                RemoteEditState::Conflict { .. } => {}
                _ if overwrite_remote_changes => {
                    return Err(AppError::Validation(
                        "Overwriting remote changes is only allowed after a conflict has been reported for this file.".to_string(),
                    ))
                }
                _ => {}
            }
            entry.session.clone()
        };

        // Phase 2: snapshot the working copy so we upload (and hash) exactly one version,
        // even if the editor saves again during the upload.
        let work_path = PathBuf::from(&session.local_path);
        if local_stat(&work_path).is_none() {
            let s = self
                .set_state(id, RemoteEditState::LocalMissing)
                .await
                .ok_or_else(|| not_found(id))?;
            return Ok(RemoteEditUploadOutcome::Failed { session: s });
        }
        ensure_private_dir(&paths.scratch_dir)?;
        let snapshot = paths.scratch_file("upload");
        let snap_src = work_path.clone();
        let snap_dst = snapshot.clone();
        let snap_res = tokio::task::spawn_blocking(move || {
            copy_private(&snap_src, &snap_dst)?;
            sha256_file(&snap_dst)
        })
        .await
        .map_err(|e| AppError::Internal(format!("Snapshot task failed: {}", e)))?;
        let snapshot_sha = match snap_res {
            Ok(h) => h,
            Err(e) => {
                let _ = fs::remove_file(&snapshot);
                return self
                    .fail(
                        id,
                        UploadFailureKind::LocalError,
                        format!(
                            "Could not read the local working copy '{}': {}",
                            work_path.display(),
                            e
                        ),
                    )
                    .await;
            }
        };

        let outcome = self
            .upload_snapshot(
                id,
                &session,
                &paths,
                &snapshot,
                &snapshot_sha,
                overwrite_remote_changes,
            )
            .await;
        let _ = fs::remove_file(&snapshot);
        outcome
    }

    async fn upload_snapshot(
        &self,
        id: &str,
        session: &RemoteEditSession,
        paths: &SessionPaths,
        snapshot: &Path,
        snapshot_sha: &str,
        overwrite: bool,
    ) -> Result<RemoteEditUploadOutcome, AppError> {
        if snapshot_sha == session.baseline_sha256 {
            let mut guard = self.entries.lock().await;
            let entry = guard.get_mut(id).ok_or_else(|| not_found(id))?;
            entry.session.local_sha256 = Some(snapshot_sha.to_string());
            entry.session.state = RemoteEditState::Clean;
            entry.session.prompt_dismissed = false;
            entry.session.refresh_derived();
            let s = entry.session.clone();
            drop(guard);
            self.publish(&s);
            return Ok(RemoteEditUploadOutcome::Unchanged { session: s });
        }

        // Claim: mark Uploading so the poller and other uploads leave it alone.
        {
            let mut guard = self.entries.lock().await;
            let entry = guard.get_mut(id).ok_or_else(|| not_found(id))?;
            if entry.session.state == RemoteEditState::Uploading {
                return Err(AppError::Validation(format!(
                    "'{}' is already being uploaded.",
                    entry.session.file_name
                )));
            }
            entry.session.state = RemoteEditState::Uploading;
            let s = entry.session.clone();
            drop(guard);
            self.sink.emit_updated(&s);
        }

        let preserved = format!("Your edited copy is preserved at '{}'.", session.local_path);

        // Same server and account?
        let endpoint = match self.backend.current_endpoint().await {
            Ok((ep, _)) => ep,
            Err(e) => {
                return self
                    .fail(
                        id,
                        UploadFailureKind::Disconnected,
                        format!(
                            "Not connected to {}: {} Reconnect and retry the upload. {}",
                            session.endpoint.display(),
                            e.message(),
                            preserved
                        ),
                    )
                    .await
            }
        };
        if !endpoint.matches(&session.endpoint) {
            return self
                .fail(
                    id,
                    UploadFailureKind::WrongServer,
                    format!(
                        "The active connection is {}, but this file was opened from {}. Connect to {} to upload it. {}",
                        endpoint.display(),
                        session.endpoint.display(),
                        session.endpoint.display(),
                        preserved
                    ),
                )
                .await;
        }

        if !overwrite {
            match self.verify_remote_unchanged(session, paths).await {
                Ok(None) => {}
                Ok(Some((reason, current))) => return self.conflict(id, reason, current).await,
                Err(e) => {
                    return self
                        .fail(
                            id,
                            failure_kind(&e),
                            format!(
                                "Could not check the remote file for changes: {} {}",
                                e.message(),
                                preserved
                            ),
                        )
                        .await
                }
            }
        }

        if let Err(e) = self
            .backend
            .upload_replace(
                snapshot,
                &session.remote_path,
                session.baseline.permissions_mode,
            )
            .await
        {
            return self
                .fail(
                    id,
                    failure_kind(&e),
                    format!("Upload failed: {} {}", e.message(), preserved),
                )
                .await;
        }

        let uploaded_len = fs::metadata(snapshot).map(|m| m.len()).ok();
        let new_baseline = match self.backend.stat(&session.remote_path).await {
            Ok(st) => st.fingerprint,
            Err(e) => {
                tracing::warn!(error = %e.message(), "Uploaded edit but could not stat remote file afterwards");
                RemoteFileFingerprint {
                    size: uploaded_len,
                    modified_at_secs: None,
                    permissions_mode: session.baseline.permissions_mode,
                    uid: None,
                    gid: None,
                }
            }
        };

        let mut guard = self.entries.lock().await;
        let entry = guard.get_mut(id).ok_or_else(|| not_found(id))?;
        entry.session.baseline = new_baseline;
        entry.session.baseline_sha256 = snapshot_sha.to_string();
        entry.session.local_sha256 = Some(snapshot_sha.to_string());
        entry.session.state = RemoteEditState::Clean;
        entry.session.prompt_dismissed = false;
        entry.session.last_synced_at_ms = now_ms();
        entry.session.upload_count += 1;
        entry.session.refresh_derived();
        // Force re-inspection: if the editor saved during the upload, the next poll
        // will hash the new content and report it as Modified again.
        entry.observed = None;
        entry.pending_hash = true;
        let s = entry.session.clone();
        drop(guard);
        self.publish(&s);
        Ok(RemoteEditUploadOutcome::Uploaded { session: s })
    }

    /// Returns `Ok(None)` if the remote still matches the baseline, `Ok(Some(reason))` if it
    /// changed or cannot be verified.
    async fn verify_remote_unchanged(
        &self,
        session: &RemoteEditSession,
        paths: &SessionPaths,
    ) -> Result<Option<(String, Option<RemoteFileFingerprint>)>, BackendError> {
        let current = match self.backend.stat(&session.remote_path).await {
            Ok(st) => st,
            Err(BackendError::NotFound(_)) => {
                return Ok(Some((
                    "The remote file was deleted or moved since it was downloaded.".to_string(),
                    None,
                )))
            }
            Err(e) => return Err(e),
        };
        if current.kind != RemoteObjectKind::Regular {
            return Ok(Some((
                "The remote path is no longer a regular file.".to_string(),
                Some(current.fingerprint),
            )));
        }

        let comparison = compare_fingerprints(&session.baseline, &current.fingerprint);
        let unverifiable_reason = match comparison {
            FingerprintComparison::Changed { reason } => {
                return Ok(Some((reason, Some(current.fingerprint))))
            }
            FingerprintComparison::Same => None,
            FingerprintComparison::Unverifiable { reason } => Some(reason),
        };

        let size_for_hash = current.fingerprint.size.or(session.baseline.size);
        let can_hash = size_for_hash.is_some_and(|s| s <= self.config.verify_hash_max_bytes);
        if !can_hash {
            return Ok(unverifiable_reason.map(|r| (r, Some(current.fingerprint))));
        }

        // Supplementary content check: catches same-size edits within one mtime second.
        ensure_private_dir(&paths.scratch_dir).map_err(BackendError::Other)?;
        let scratch = paths.scratch_file("verify");
        let res = async {
            self.backend
                .download_to(&session.remote_path, &scratch)
                .await?;
            hash_file_blocking(scratch.clone())
                .await
                .map_err(|e| BackendError::Other(AppError::from(e)))
        }
        .await;
        let _ = fs::remove_file(&scratch);
        let remote_sha = match res {
            Ok(h) => h,
            Err(BackendError::NotFound(_)) => {
                return Ok(Some((
                    "The remote file was deleted or moved since it was downloaded.".to_string(),
                    None,
                )))
            }
            Err(e) => return Err(e),
        };
        if remote_sha != session.baseline_sha256 {
            return Ok(Some((
                "The remote file content changed since it was downloaded (size and modification time were unchanged).".to_string(),
                Some(current.fingerprint),
            )));
        }
        Ok(None)
    }

    /// "Keep local changes": stop prompting for the current modification. The working
    /// copy stays tracked and will prompt again when it changes.
    pub async fn keep_local(&self, id: &str) -> Result<RemoteEditSession, AppError> {
        let mut guard = self.entries.lock().await;
        let entry = guard.get_mut(id).ok_or_else(|| not_found(id))?;
        entry.session.prompt_dismissed = true;
        let s = entry.session.clone();
        drop(guard);
        self.publish(&s);
        Ok(s)
    }

    /// Copies the working copy into `destination_dir` under a non-conflicting name
    /// (e.g. for manual comparison after a conflict). Returns the new path.
    pub async fn save_copy(&self, id: &str, destination_dir: &str) -> Result<String, AppError> {
        let session = self.get(id).await?;
        let dest_dir = PathBuf::from(destination_dir.trim());
        let src = PathBuf::from(&session.local_path);
        let file_name = session.file_name.clone();
        tokio::task::spawn_blocking(move || {
            let meta = fs::metadata(&dest_dir).map_err(|e| {
                AppError::Filesystem(format!(
                    "Destination folder '{}' is not accessible: {}",
                    dest_dir.display(),
                    e
                ))
            })?;
            if !meta.is_dir() {
                return Err(AppError::Validation(format!(
                    "'{}' is not a folder.",
                    dest_dir.display()
                )));
            }
            let base = dest_dir.join(&file_name).to_string_lossy().into_owned();
            for attempt in 1..1000 {
                let candidate = PathBuf::from(generate_renamed_path(&base, attempt));
                match copy_private(&src, &candidate) {
                    Ok(_) => return Ok(candidate.to_string_lossy().into_owned()),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(e) => {
                        return Err(AppError::Filesystem(format!(
                            "Failed to save a copy to '{}': {}",
                            candidate.display(),
                            e
                        )))
                    }
                }
            }
            Err(AppError::Filesystem(
                "Could not find a free file name for the copy.".to_string(),
            ))
        })
        .await
        .map_err(|e| AppError::Internal(format!("Save copy task failed: {}", e)))?
    }

    /// Discards the working copy and stops tracking it. Requires
    /// `discard_local_changes` when the copy may hold edits not on the server.
    pub async fn discard(&self, id: &str, discard_local_changes: bool) -> Result<(), AppError> {
        validate_session_id(id)?;
        // Re-inspect first so a save made in the last poll interval is not missed.
        self.scan_local_changes(false).await;
        let mut guard = self.entries.lock().await;
        let entry = guard.get(id).ok_or_else(|| not_found(id))?;
        if entry.session.state == RemoteEditState::Uploading {
            return Err(AppError::Validation(
                "Cannot discard a working copy while it is being uploaded.".to_string(),
            ));
        }
        if entry.session.may_lose_local_work() && !discard_local_changes {
            return Err(AppError::Validation(format!(
                "'{}' has local changes that are not on the server. Confirm discarding them, or upload / save a copy first.",
                entry.session.file_name
            )));
        }
        remove_session_dir(&self.root, id)?;
        guard.remove(id);
        drop(guard);
        self.sink.emit_removed(id);
        Ok(())
    }

    /// Restores sessions persisted by a previous run. Directories without a manifest are
    /// incomplete downloads (the manifest is written before any application is launched)
    /// and are removed. Corrupt manifests are left on disk untouched.
    pub async fn restore_from_disk(&self) -> Result<usize, AppError> {
        if !self.root.exists() {
            return Ok(0);
        }
        ensure_private_dir(&self.root)?;
        let root = self.root.clone();
        let found: Vec<RemoteEditSession> = tokio::task::spawn_blocking(move || {
            let mut out = Vec::new();
            let Ok(read) = fs::read_dir(&root) else {
                return out;
            };
            for dirent in read.flatten() {
                let name = dirent.file_name().to_string_lossy().into_owned();
                if validate_session_id(&name).is_err() {
                    continue;
                }
                let manifest = dirent.path().join(MANIFEST_FILE);
                if !manifest.exists() {
                    if let Err(e) = remove_session_dir(&root, &name) {
                        tracing::warn!(error = %e, "Failed to remove incomplete remote edit directory");
                    }
                    continue;
                }
                match read_manifest(&manifest) {
                    Ok(s) if s.id == name => out.push(s),
                    Ok(_) => tracing::warn!(dir = %name, "Remote edit manifest id mismatch; leaving directory untouched"),
                    Err(e) => tracing::warn!(error = %e, "Unreadable remote edit manifest; leaving directory untouched"),
                }
            }
            out
        })
        .await
        .map_err(|e| AppError::Internal(format!("Restore task failed: {}", e)))?;

        let count = found.len();
        let mut guard = self.entries.lock().await;
        for mut session in found {
            session.recovered = true;
            if session.state == RemoteEditState::Uploading {
                // Interrupted upload: re-evaluate locally; any upload attempt will
                // re-check the remote fingerprint, so a half-finished upload shows as a conflict.
                session.state = RemoteEditState::Modified;
            }
            session.prompt_dismissed = false;
            guard.insert(
                session.id.clone(),
                Entry {
                    session,
                    observed: None,
                    pending_hash: true,
                    missing_polls: 0,
                },
            );
        }
        Ok(count)
    }

    /// Exit-time cleanup: removes working copies whose content is verified identical to
    /// the remote baseline. Anything modified, conflicting, failed or unverifiable is kept
    /// and restored on next launch.
    pub async fn cleanup_on_exit(&self) -> usize {
        let candidates: Vec<(String, PathBuf, String)> = {
            let guard = self.entries.lock().await;
            guard
                .values()
                .filter(|e| {
                    e.session.state == RemoteEditState::Clean && !e.session.has_unsynced_changes
                })
                .map(|e| {
                    (
                        e.session.id.clone(),
                        PathBuf::from(&e.session.local_path),
                        e.session.baseline_sha256.clone(),
                    )
                })
                .collect()
        };
        let mut removed = 0;
        for (id, path, baseline) in candidates {
            match hash_file_blocking(path).await {
                Ok(h) if h == baseline && remove_session_dir(&self.root, &id).is_ok() => {
                    self.entries.lock().await.remove(&id);
                    removed += 1;
                }
                _ => {}
            }
        }
        removed
    }
}

fn ensure_regular(stat: &RemoteFileStat, path: &str) -> Result<(), AppError> {
    match stat.kind {
        RemoteObjectKind::Regular => Ok(()),
        RemoteObjectKind::Directory => Err(AppError::Validation(format!(
            "'{}' is a directory and cannot be opened in an application.",
            path
        ))),
        RemoteObjectKind::Other => Err(AppError::Validation(format!(
            "'{}' is not a regular file and cannot be edited.",
            path
        ))),
    }
}

fn not_found(id: &str) -> AppError {
    AppError::Validation(format!("Remote edit session '{}' was not found.", id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    #[derive(Clone)]
    struct FakeFile {
        content: Vec<u8>,
        mtime: Option<u64>,
        mode: u32,
    }

    struct FakeState {
        connected: bool,
        endpoint: RemoteEndpointIdentity,
        files: HashMap<String, FakeFile>,
        fail_upload: Option<BackendError>,
        uploads: u32,
        clock: u64,
        report_mtime: bool,
    }

    /// In-memory remote used to exercise the manager's safety logic deterministically.
    struct FakeBackend {
        state: StdMutex<FakeState>,
    }

    fn endpoint(host: &str) -> RemoteEndpointIdentity {
        RemoteEndpointIdentity {
            host: host.to_string(),
            port: 22,
            username: "deploy".to_string(),
            elevated: false,
        }
    }

    impl FakeBackend {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                state: StdMutex::new(FakeState {
                    connected: true,
                    endpoint: endpoint("srv.example"),
                    files: HashMap::new(),
                    fail_upload: None,
                    uploads: 0,
                    clock: 1_700_000_000,
                    report_mtime: true,
                }),
            })
        }
        fn put(&self, path: &str, content: &[u8]) {
            let mut s = self.state.lock().unwrap();
            s.clock += 10;
            let mtime = Some(s.clock);
            s.files.insert(
                path.to_string(),
                FakeFile {
                    content: content.to_vec(),
                    mtime,
                    mode: 0o100640,
                },
            );
        }
        /// Replace content without changing size or mtime (same-second edit).
        fn put_same_meta(&self, path: &str, content: &[u8]) {
            let mut s = self.state.lock().unwrap();
            let f = s.files.get_mut(path).unwrap();
            assert_eq!(f.content.len(), content.len());
            f.content = content.to_vec();
        }
        fn content(&self, path: &str) -> Option<Vec<u8>> {
            self.state
                .lock()
                .unwrap()
                .files
                .get(path)
                .map(|f| f.content.clone())
        }
        fn uploads(&self) -> u32 {
            self.state.lock().unwrap().uploads
        }
        fn set_connected(&self, c: bool) {
            self.state.lock().unwrap().connected = c;
        }
        fn check(&self) -> Result<(), BackendError> {
            if self.state.lock().unwrap().connected {
                Ok(())
            } else {
                Err(BackendError::Disconnected(
                    "No active SFTP connection".into(),
                ))
            }
        }
    }

    #[async_trait]
    impl RemoteEditBackend for FakeBackend {
        async fn current_endpoint(
            &self,
        ) -> Result<(RemoteEndpointIdentity, Option<String>), BackendError> {
            self.check()?;
            Ok((
                self.state.lock().unwrap().endpoint.clone(),
                Some("p1".into()),
            ))
        }
        async fn canonicalize(&self, path: &str) -> Result<String, BackendError> {
            self.check()?;
            Ok(path.to_string())
        }
        async fn stat(&self, path: &str) -> Result<RemoteFileStat, BackendError> {
            self.check()?;
            let s = self.state.lock().unwrap();
            let f = s
                .files
                .get(path)
                .ok_or_else(|| BackendError::NotFound(format!("{} not found", path)))?;
            Ok(RemoteFileStat {
                kind: RemoteObjectKind::Regular,
                fingerprint: RemoteFileFingerprint {
                    size: Some(f.content.len() as u64),
                    modified_at_secs: if s.report_mtime { f.mtime } else { None },
                    permissions_mode: Some(f.mode),
                    uid: Some(1000),
                    gid: Some(1000),
                },
            })
        }
        async fn download_to(&self, path: &str, dest: &Path) -> Result<u64, BackendError> {
            self.check()?;
            let content = self
                .content(path)
                .ok_or_else(|| BackendError::NotFound(format!("{} not found", path)))?;
            fs::write(dest, &content).map_err(|e| BackendError::Other(e.into()))?;
            Ok(content.len() as u64)
        }
        async fn upload_replace(
            &self,
            src: &Path,
            path: &str,
            mode: Option<u32>,
        ) -> Result<(), BackendError> {
            self.check()?;
            if let Some(err) = self.state.lock().unwrap().fail_upload.clone() {
                return Err(err);
            }
            let data = fs::read(src).map_err(|e| BackendError::Other(e.into()))?;
            let mut s = self.state.lock().unwrap();
            s.clock += 10;
            let mtime = Some(s.clock);
            s.uploads += 1;
            s.files.insert(
                path.to_string(),
                FakeFile {
                    content: data,
                    mtime,
                    mode: mode.unwrap_or(0o100644),
                },
            );
            Ok(())
        }
    }

    struct CountingSink {
        removed: StdMutex<Vec<String>>,
    }
    impl RemoteEditEventSink for CountingSink {
        fn emit_updated(&self, _s: &RemoteEditSession) {}
        fn emit_removed(&self, id: &str) {
            self.removed.lock().unwrap().push(id.to_string());
        }
    }

    const REMOTE: &str = "/etc/app/config.ini";

    fn temp_root() -> PathBuf {
        std::env::temp_dir().join(format!("linuxscp-redit-test-{}", Uuid::new_v4()))
    }

    fn manager(backend: Arc<FakeBackend>, root: &Path) -> RemoteEditManager {
        RemoteEditManager::new(
            backend,
            Arc::new(NoopRemoteEditEventSink),
            root.to_path_buf(),
            RemoteEditConfig::default(),
        )
    }

    async fn setup() -> (
        Arc<FakeBackend>,
        RemoteEditManager,
        RemoteEditSession,
        PathBuf,
    ) {
        let backend = FakeBackend::new();
        backend.put(REMOTE, b"key=original\n");
        let root = temp_root();
        let mgr = manager(backend.clone(), &root);
        let session = mgr.open_session(REMOTE).await.unwrap();
        (backend, mgr, session, root)
    }

    /// Simulates an editor save; sleeps briefly so the mtime_ns observably changes.
    fn edit_local(session: &RemoteEditSession, content: &[u8]) {
        std::thread::sleep(std::time::Duration::from_millis(15));
        fs::write(&session.local_path, content).unwrap();
    }

    async fn settle(mgr: &RemoteEditManager) {
        mgr.scan_local_changes(true).await;
        mgr.scan_local_changes(true).await;
    }

    #[tokio::test]
    async fn open_creates_private_working_copy_and_manifest() {
        let (_b, mgr, s, root) = setup().await;
        assert_eq!(s.state, RemoteEditState::Clean);
        assert_eq!(fs::read(&s.local_path).unwrap(), b"key=original\n");
        let mode = fs::metadata(&s.local_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert!(s.local_path.ends_with("/work/config.ini"));
        assert!(root.join(&s.id).join(MANIFEST_FILE).exists());
        assert_eq!(s.baseline.size, Some(13));
        assert!(!s.has_unsynced_changes);
        // No scratch leftovers.
        assert_eq!(
            fs::read_dir(root.join(&s.id).join("scratch"))
                .unwrap()
                .count(),
            0
        );
        mgr.discard(&s.id, false).await.unwrap();
        assert!(!root.join(&s.id).exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn unchanged_file_is_never_uploaded() {
        let (b, mgr, s, root) = setup().await;
        settle(&mgr).await;
        assert_eq!(mgr.get(&s.id).await.unwrap().state, RemoteEditState::Clean);

        // A save that rewrites identical bytes (new mtime) is not a modification.
        edit_local(&s, b"key=original\n");
        settle(&mgr).await;
        assert_eq!(mgr.get(&s.id).await.unwrap().state, RemoteEditState::Clean);

        let out = mgr.upload(&s.id, false).await.unwrap();
        assert!(matches!(out, RemoteEditUploadOutcome::Unchanged { .. }));
        assert_eq!(b.uploads(), 0);
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn modified_file_detected_after_stable_window_and_uploaded() {
        let (b, mgr, s, root) = setup().await;
        edit_local(&s, b"key=changed\n");

        // First poll only observes the stat change (debounce), second hashes.
        mgr.scan_local_changes(true).await;
        assert_eq!(mgr.get(&s.id).await.unwrap().state, RemoteEditState::Clean);
        mgr.scan_local_changes(true).await;
        let modified = mgr.get(&s.id).await.unwrap();
        assert_eq!(modified.state, RemoteEditState::Modified);
        assert!(modified.has_unsynced_changes);
        assert_eq!(b.uploads(), 0, "detection must never upload automatically");

        let out = mgr.upload(&s.id, false).await.unwrap();
        let after = match out {
            RemoteEditUploadOutcome::Uploaded { session } => session,
            other => panic!("expected upload, got {:?}", other),
        };
        assert_eq!(b.content(REMOTE).unwrap(), b"key=changed\n");
        assert_eq!(after.state, RemoteEditState::Clean);
        assert_eq!(after.upload_count, 1);
        assert!(!after.has_unsynced_changes);
        assert_eq!(after.baseline.size, Some(12));

        // Re-upload without further edits does nothing.
        settle(&mgr).await;
        let out2 = mgr.upload(&s.id, false).await.unwrap();
        assert!(matches!(out2, RemoteEditUploadOutcome::Unchanged { .. }));
        assert_eq!(b.uploads(), 1);

        // Original permissions were requested for the replacement.
        assert_eq!(
            b.state.lock().unwrap().files.get(REMOTE).unwrap().mode,
            0o100640
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn remote_change_is_reported_as_conflict_and_not_overwritten() {
        let (b, mgr, s, root) = setup().await;
        b.put(REMOTE, b"key=someone-else\n");
        edit_local(&s, b"key=mine\n");
        settle(&mgr).await;

        let out = mgr.upload(&s.id, false).await.unwrap();
        let c = match out {
            RemoteEditUploadOutcome::Conflict { session } => session,
            other => panic!("expected conflict, got {:?}", other),
        };
        assert!(matches!(c.state, RemoteEditState::Conflict { .. }));
        assert_eq!(b.content(REMOTE).unwrap(), b"key=someone-else\n");
        assert_eq!(fs::read(&s.local_path).unwrap(), b"key=mine\n");
        assert_eq!(b.uploads(), 0);

        // Retrying a normal upload still refuses.
        let again = mgr.upload(&s.id, false).await.unwrap();
        assert!(matches!(again, RemoteEditUploadOutcome::Conflict { .. }));
        assert_eq!(b.uploads(), 0);

        // Explicit overwrite after seeing the conflict succeeds.
        let forced = mgr.upload(&s.id, true).await.unwrap();
        assert!(matches!(forced, RemoteEditUploadOutcome::Uploaded { .. }));
        assert_eq!(b.content(REMOTE).unwrap(), b"key=mine\n");
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn same_size_same_mtime_remote_change_caught_by_hash() {
        let (b, mgr, s, root) = setup().await;
        b.put_same_meta(REMOTE, b"key=ORIGINAL\n");
        edit_local(&s, b"key=mine\n");
        settle(&mgr).await;
        let out = mgr.upload(&s.id, false).await.unwrap();
        assert!(matches!(out, RemoteEditUploadOutcome::Conflict { .. }));
        assert_eq!(b.content(REMOTE).unwrap(), b"key=ORIGINAL\n");
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn unverifiable_large_file_without_mtime_is_conflict() {
        let backend = FakeBackend::new();
        backend.put(REMOTE, b"0123456789");
        backend.state.lock().unwrap().report_mtime = false;
        let root = temp_root();
        let mgr = RemoteEditManager::new(
            backend.clone(),
            Arc::new(NoopRemoteEditEventSink),
            root.clone(),
            RemoteEditConfig {
                verify_hash_max_bytes: 4,
                ..Default::default()
            },
        );
        let s = mgr.open_session(REMOTE).await.unwrap();
        edit_local(&s, b"9876543210");
        settle(&mgr).await;
        let out = mgr.upload(&s.id, false).await.unwrap();
        assert!(matches!(out, RemoteEditUploadOutcome::Conflict { .. }));
        assert_eq!(backend.uploads(), 0);
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn overwrite_flag_rejected_without_prior_conflict() {
        let (b, mgr, s, root) = setup().await;
        edit_local(&s, b"key=mine\n");
        settle(&mgr).await;
        assert!(matches!(
            mgr.upload(&s.id, true).await,
            Err(AppError::Validation(_))
        ));
        assert_eq!(b.uploads(), 0);
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn deleted_remote_file_is_conflict() {
        let (b, mgr, s, root) = setup().await;
        b.state.lock().unwrap().files.remove(REMOTE);
        edit_local(&s, b"key=mine\n");
        settle(&mgr).await;
        let out = mgr.upload(&s.id, false).await.unwrap();
        assert!(matches!(out, RemoteEditUploadOutcome::Conflict { .. }));
        assert!(b.content(REMOTE).is_none());
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn disconnected_upload_keeps_local_copy_and_succeeds_after_reconnect() {
        let (b, mgr, s, root) = setup().await;
        edit_local(&s, b"key=offline-edit\n");
        settle(&mgr).await;
        b.set_connected(false);

        let out = mgr.upload(&s.id, false).await.unwrap();
        let failed = match out {
            RemoteEditUploadOutcome::Failed { session } => session,
            other => panic!("expected failure, got {:?}", other),
        };
        assert!(matches!(
            failed.state,
            RemoteEditState::UploadFailed {
                kind: UploadFailureKind::Disconnected,
                ..
            }
        ));
        assert!(failed.has_unsynced_changes);
        assert_eq!(fs::read(&s.local_path).unwrap(), b"key=offline-edit\n");
        assert_eq!(b.content(REMOTE).unwrap(), b"key=original\n");

        // Detection keeps working while disconnected and does not clear the failure.
        settle(&mgr).await;
        assert!(matches!(
            mgr.get(&s.id).await.unwrap().state,
            RemoteEditState::UploadFailed { .. }
        ));

        // Discard requires explicit confirmation.
        assert!(mgr.discard(&s.id, false).await.is_err());
        assert!(PathBuf::from(&s.local_path).exists());

        b.set_connected(true);
        let out = mgr.upload(&s.id, false).await.unwrap();
        assert!(matches!(out, RemoteEditUploadOutcome::Uploaded { .. }));
        assert_eq!(b.content(REMOTE).unwrap(), b"key=offline-edit\n");
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn reconnect_to_different_server_blocks_upload() {
        let (b, mgr, s, root) = setup().await;
        edit_local(&s, b"key=mine\n");
        settle(&mgr).await;
        b.state.lock().unwrap().endpoint = endpoint("other.example");
        let out = mgr.upload(&s.id, false).await.unwrap();
        assert!(matches!(
            out.session().state,
            RemoteEditState::UploadFailed {
                kind: UploadFailureKind::WrongServer,
                ..
            }
        ));
        assert_eq!(b.uploads(), 0);
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn failed_upload_preserves_local_edit_and_remote_then_retries() {
        let (b, mgr, s, root) = setup().await;
        edit_local(&s, b"key=mine\n");
        settle(&mgr).await;
        b.state.lock().unwrap().fail_upload = Some(BackendError::Other(
            AppError::PermissionDenied("write denied".into()),
        ));

        let out = mgr.upload(&s.id, false).await.unwrap();
        match &out.session().state {
            RemoteEditState::UploadFailed { kind, message } => {
                assert_eq!(*kind, UploadFailureKind::RemoteError);
                assert!(message.contains("preserved"));
            }
            other => panic!("expected UploadFailed, got {:?}", other),
        }
        assert_eq!(fs::read(&s.local_path).unwrap(), b"key=mine\n");
        assert_eq!(b.content(REMOTE).unwrap(), b"key=original\n");
        assert!(out.session().has_unsynced_changes);

        // Exit cleanup must keep it.
        assert_eq!(mgr.cleanup_on_exit().await, 0);
        assert!(PathBuf::from(&s.local_path).exists());

        b.state.lock().unwrap().fail_upload = None;
        let retry = mgr.upload(&s.id, false).await.unwrap();
        assert!(matches!(retry, RemoteEditUploadOutcome::Uploaded { .. }));
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn keep_local_dismisses_prompt_until_next_change() {
        let (_b, mgr, s, root) = setup().await;
        edit_local(&s, b"key=v1\n");
        settle(&mgr).await;
        let kept = mgr.keep_local(&s.id).await.unwrap();
        assert!(kept.prompt_dismissed);
        assert_eq!(kept.state, RemoteEditState::Modified);
        assert!(PathBuf::from(&s.local_path).exists());

        edit_local(&s, b"key=v2-longer\n");
        settle(&mgr).await;
        assert!(!mgr.get(&s.id).await.unwrap().prompt_dismissed);
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn reverting_local_edit_returns_to_clean() {
        let (_b, mgr, s, root) = setup().await;
        edit_local(&s, b"key=v1\n");
        settle(&mgr).await;
        assert_eq!(
            mgr.get(&s.id).await.unwrap().state,
            RemoteEditState::Modified
        );
        edit_local(&s, b"key=original\n");
        settle(&mgr).await;
        assert_eq!(mgr.get(&s.id).await.unwrap().state, RemoteEditState::Clean);
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn missing_working_copy_is_reported_and_not_uploaded() {
        let (b, mgr, s, root) = setup().await;
        fs::remove_file(&s.local_path).unwrap();
        mgr.scan_local_changes(true).await;
        // A single missing poll may be an atomic-rename save in progress.
        assert_eq!(mgr.get(&s.id).await.unwrap().state, RemoteEditState::Clean);
        mgr.scan_local_changes(true).await;
        assert_eq!(
            mgr.get(&s.id).await.unwrap().state,
            RemoteEditState::LocalMissing
        );
        let out = mgr.upload(&s.id, false).await.unwrap();
        assert!(matches!(out, RemoteEditUploadOutcome::Failed { .. }));
        assert_eq!(b.uploads(), 0);
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn atomic_rename_save_is_detected() {
        let (_b, mgr, s, root) = setup().await;
        std::thread::sleep(std::time::Duration::from_millis(15));
        let tmp = PathBuf::from(&s.local_path).with_extension("swp");
        fs::write(&tmp, b"key=renamed-save\n").unwrap();
        fs::rename(&tmp, &s.local_path).unwrap();
        settle(&mgr).await;
        assert_eq!(
            mgr.get(&s.id).await.unwrap().state,
            RemoteEditState::Modified
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn reopen_reuses_session_and_never_redownloads_over_edits() {
        let (b, mgr, s, root) = setup().await;
        edit_local(&s, b"key=mine\n");
        settle(&mgr).await;
        b.put(REMOTE, b"key=remote-newer\n");
        let again = mgr.open_session(REMOTE).await.unwrap();
        assert_eq!(again.id, s.id);
        assert_eq!(fs::read(&s.local_path).unwrap(), b"key=mine\n");
        assert!(matches!(again.state, RemoteEditState::Conflict { .. }));
        assert_eq!(mgr.list().await.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn auto_upload_mode_uploads_clean_save_and_blocks_conflicts() {
        let (b, mgr, s, root) = setup().await;
        mgr.set_upload_mode(RemoteEditUploadMode::Auto).await;

        // 1. Local save when remote is unchanged -> automatically uploaded once stable.
        edit_local(&s, b"key=auto-saved\n");
        settle(&mgr).await;
        let updated = mgr.get(&s.id).await.unwrap();
        assert_eq!(updated.state, RemoteEditState::Clean);
        assert_eq!(updated.upload_count, 1);
        assert_eq!(b.content(REMOTE).unwrap(), b"key=auto-saved\n");

        // 2. Remote modified externally AND local modified -> Conflict, never overwritten automatically.
        b.put(REMOTE, b"key=external-remote-edit\n");
        edit_local(&s, b"key=second-local-edit\n");
        settle(&mgr).await;
        let conflicted = mgr.get(&s.id).await.unwrap();
        assert!(matches!(conflicted.state, RemoteEditState::Conflict { .. }));
        assert_eq!(b.content(REMOTE).unwrap(), b"key=external-remote-edit\n");
        assert_eq!(fs::read(&s.local_path).unwrap(), b"key=second-local-edit\n");

        // 3. Reload from remote replaces local working copy with latest remote content.
        let reloaded = mgr.reload_from_remote(&s.id).await.unwrap();
        assert_eq!(reloaded.state, RemoteEditState::Clean);
        assert_eq!(
            fs::read(&s.local_path).unwrap(),
            b"key=external-remote-edit\n"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn reopen_clean_session_refreshes_same_size_same_mtime_remote_change() {
        let (b, mgr, s, root) = setup().await;
        b.put_same_meta(REMOTE, b"key=ORIGINAL\n");
        let again = mgr.open_session(REMOTE).await.unwrap();
        assert_eq!(again.id, s.id);
        assert_eq!(fs::read(&s.local_path).unwrap(), b"key=ORIGINAL\n");
        assert_eq!(again.state, RemoteEditState::Clean);
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn reopen_clean_session_refreshes_from_newer_remote() {
        let (b, mgr, s, root) = setup().await;
        b.put(REMOTE, b"key=remote-newer\n");
        let again = mgr.open_session(REMOTE).await.unwrap();
        assert_eq!(again.id, s.id);
        assert_eq!(fs::read(&s.local_path).unwrap(), b"key=remote-newer\n");
        assert_eq!(again.state, RemoteEditState::Clean);
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn opening_while_disconnected_fails_without_leftovers() {
        let backend = FakeBackend::new();
        backend.put(REMOTE, b"x");
        backend.set_connected(false);
        let root = temp_root();
        let mgr = manager(backend, &root);
        assert!(matches!(
            mgr.open_session(REMOTE).await,
            Err(AppError::Network(_))
        ));
        assert!(mgr.list().await.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn restart_restores_modified_sessions_and_exit_cleans_only_clean_ones() {
        let backend = FakeBackend::new();
        backend.put(REMOTE, b"key=original\n");
        backend.put("/srv/clean.txt", b"clean\n");
        let root = temp_root();
        let mgr = manager(backend.clone(), &root);
        let dirty = mgr.open_session(REMOTE).await.unwrap();
        let clean = mgr.open_session("/srv/clean.txt").await.unwrap();
        edit_local(&dirty, b"key=unsaved-to-server\n");
        settle(&mgr).await;

        // Simulated crash: incomplete download directory without manifest.
        let orphan = Uuid::new_v4().to_string();
        fs::create_dir_all(root.join(&orphan).join("work")).unwrap();

        // New process.
        let mgr2 = manager(backend.clone(), &root);
        assert_eq!(mgr2.restore_from_disk().await.unwrap(), 2);
        assert!(!root.join(&orphan).exists());
        settle(&mgr2).await;
        let restored = mgr2.get(&dirty.id).await.unwrap();
        assert!(restored.recovered);
        assert_eq!(restored.state, RemoteEditState::Modified);
        assert_eq!(
            mgr2.get(&clean.id).await.unwrap().state,
            RemoteEditState::Clean
        );

        assert_eq!(mgr2.cleanup_on_exit().await, 1);
        assert!(!root.join(&clean.id).exists());
        assert!(PathBuf::from(&dirty.local_path).exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn exit_cleanup_rechecks_content_before_deleting() {
        let (_b, mgr, s, root) = setup().await;
        // Saved after the last poll: the in-memory state is still Clean.
        edit_local(&s, b"key=last-second-save\n");
        assert_eq!(mgr.cleanup_on_exit().await, 0);
        assert!(PathBuf::from(&s.local_path).exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn discard_clean_session_without_confirmation_and_emit_removed() {
        let backend = FakeBackend::new();
        backend.put(REMOTE, b"a");
        let root = temp_root();
        let sink = Arc::new(CountingSink {
            removed: StdMutex::new(Vec::new()),
        });
        let mgr = RemoteEditManager::new(
            backend,
            sink.clone(),
            root.clone(),
            RemoteEditConfig::default(),
        );
        let s = mgr.open_session(REMOTE).await.unwrap();
        mgr.discard(&s.id, false).await.unwrap();
        assert_eq!(
            sink.removed.lock().unwrap().as_slice(),
            std::slice::from_ref(&s.id)
        );
        assert!(mgr.list().await.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn discard_detects_unpolled_save_and_requires_confirmation() {
        let (_b, mgr, s, root) = setup().await;
        edit_local(&s, b"key=just-saved\n");
        assert!(mgr.discard(&s.id, false).await.is_err());
        mgr.discard(&s.id, true).await.unwrap();
        assert!(!root.join(&s.id).exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn save_copy_uses_non_conflicting_name() {
        let (_b, mgr, s, root) = setup().await;
        edit_local(&s, b"key=mine\n");
        let dest = temp_root();
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("config.ini"), b"existing").unwrap();
        let copy = mgr.save_copy(&s.id, &dest.to_string_lossy()).await.unwrap();
        assert!(copy.ends_with("config (1).ini"));
        assert_eq!(fs::read(&copy).unwrap(), b"key=mine\n");
        assert_eq!(fs::read(dest.join("config.ini")).unwrap(), b"existing");
        let _ = fs::remove_dir_all(&dest);
        let _ = fs::remove_dir_all(&root);
    }
}
