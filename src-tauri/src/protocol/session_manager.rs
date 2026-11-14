use crate::errors::AppError;
use crate::models::{
    ConnectionProfile, ConnectionSession, FileOwnership, ProfileTransferSettings,
    ProtocolCapabilities, ProtocolType, SessionStatus, UnixPermissions,
};
use crate::persistence::{Database, KnownHostEntry, RemoteStateRepository};
use crate::protocol::scp::{establish_scp_connection, ActiveScpConnection, ScpFileSystemAdapter};
use crate::protocol::sftp::{
    apply_remote_permissions_with_adapter, check_sftp_path_is_dir, establish_sftp_connection,
    join_posix_path, list_sftp_directory, normalize_posix_path, parent_posix_path,
    parse_octal_mode, resolve_identities_for_entries, resolve_name_to_id_via_getent,
    resolve_remote_identity_via_getent, validate_remote_entry_name, ActiveSftpConnection,
    ConnectOutcome, RemoteDirectoryListing, SftpFileSystemAdapter,
};
use crate::protocol::RemoteFileSystem;
use russh::client::Handle;
use russh_sftp::client::SftpSession;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Protocol client handle available to active transfer workers.
#[derive(Clone)]
pub enum TransferClient {
    Sftp(Arc<SftpSession>),
    Scp {
        ssh_handle: Arc<tokio::sync::Mutex<Handle<crate::protocol::sftp::SshClientHandler>>>,
    },
}

/// Active transfer context needed by background transfer jobs.
#[derive(Clone)]
pub struct ActiveTransferContext {
    pub session_id: String,
    pub client: TransferClient,
    pub sftp: Option<Arc<SftpSession>>,
    pub capabilities: ProtocolCapabilities,
    pub transfer_settings: ProfileTransferSettings,
    pub timeout_duration: Duration,
}

/// Unified active connection enum holding either an SFTP or an SCP connection.
pub enum ActiveBackendConnection {
    Sftp(ActiveSftpConnection),
    Scp(ActiveScpConnection),
}

impl ActiveBackendConnection {
    pub fn session_meta(&self) -> &ConnectionSession {
        match self {
            Self::Sftp(s) => &s.session_meta,
            Self::Scp(s) => &s.session_meta,
        }
    }

    pub fn session_meta_mut(&mut self) -> &mut ConnectionSession {
        match self {
            Self::Sftp(s) => &mut s.session_meta,
            Self::Scp(s) => &mut s.session_meta,
        }
    }

    pub fn trusted_host_key(&self) -> &KnownHostEntry {
        match self {
            Self::Sftp(s) => &s.trusted_host_key,
            Self::Scp(s) => &s.trusted_host_key,
        }
    }

    pub fn profile(&self) -> &ConnectionProfile {
        match self {
            Self::Sftp(s) => &s.profile,
            Self::Scp(s) => &s.profile,
        }
    }

    pub async fn close(self) -> Result<(), AppError> {
        match self {
            Self::Sftp(s) => s.close().await,
            Self::Scp(s) => s.close().await,
        }
    }
}

/// Snapshot of the current session manager state returned to the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionStateSnapshot {
    pub status: SessionStatus,
    pub session: Option<ConnectionSession>,
    pub trusted_host_key: Option<KnownHostEntry>,
}

struct InnerSessionState {
    status: SessionStatus,
    active_session_id: Option<String>,
    last_profile: Option<ConnectionProfile>,
    sessions: std::collections::HashMap<String, ActiveBackendConnection>,
    session_statuses: std::collections::HashMap<String, SessionStatus>,
    session_profiles: std::collections::HashMap<String, ConnectionProfile>,
}

/// Thread-safe manager tracking connection lifecycle, cancellation, and active session handles.
#[derive(Clone)]
pub struct SessionManager {
    inner: Arc<RwLock<InnerSessionState>>,
    generation: Arc<AtomicU64>,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(InnerSessionState {
                status: SessionStatus::Disconnected,
                active_session_id: None,
                last_profile: None,
                sessions: std::collections::HashMap::new(),
                session_statuses: std::collections::HashMap::new(),
                session_profiles: std::collections::HashMap::new(),
            })),
            generation: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Returns a snapshot of the current connection lifecycle state (defaults to active session).
    pub async fn snapshot(&self) -> SessionStateSnapshot {
        self.snapshot_for(None).await
    }

    /// Returns a snapshot of the specified session ID (or active session if None).
    pub async fn snapshot_for(&self, session_id: Option<&str>) -> SessionStateSnapshot {
        let guard = self.inner.read().await;
        let target_id = session_id.or(guard.active_session_id.as_deref());
        if let Some(sid) = target_id {
            if let Some(active) = guard.sessions.get(sid) {
                let status = guard
                    .session_statuses
                    .get(sid)
                    .cloned()
                    .unwrap_or(SessionStatus::Connected);
                return SessionStateSnapshot {
                    status,
                    session: Some(active.session_meta().clone()),
                    trusted_host_key: Some(active.trusted_host_key().clone()),
                };
            }
            if let Some(status) = guard.session_statuses.get(sid) {
                return SessionStateSnapshot {
                    status: status.clone(),
                    session: None,
                    trusted_host_key: None,
                };
            }
        }
        SessionStateSnapshot {
            status: guard.status.clone(),
            session: None,
            trusted_host_key: None,
        }
    }

    /// Sets the currently focused/active session ID.
    pub async fn set_active_session_id(&self, session_id: &str) {
        let mut guard = self.inner.write().await;
        if guard.sessions.contains_key(session_id) {
            guard.active_session_id = Some(session_id.to_string());
            guard.status = SessionStatus::Connected;
        }
    }

    /// Returns the active session ID, if any.
    pub async fn active_session_id(&self) -> Option<String> {
        self.inner.read().await.active_session_id.clone()
    }

    /// Checks whether a specific session ID is actively connected.
    pub async fn has_session(&self, session_id: &str) -> bool {
        self.inner.read().await.sessions.contains_key(session_id)
    }

    /// Returns the total number of connected remote sessions.
    pub async fn session_count(&self) -> usize {
        self.inner.read().await.sessions.len()
    }

    /// Connects to a remote server using the given profile, replacing any previous connection.
    /// Strictly adheres to the configured protocol without silent fallbacks.
    pub async fn connect(
        &self,
        profile: ConnectionProfile,
        secret_override: Option<String>,
        is_reconnect: bool,
    ) -> Result<ConnectOutcome, AppError> {
        self.connect_with_session_id(profile, secret_override, is_reconnect, None)
            .await
    }

    /// Connects to a remote server using the given profile, optionally replacing a specific existing session ID.
    pub async fn connect_with_session_id(
        &self,
        profile: ConnectionProfile,
        secret_override: Option<String>,
        is_reconnect: bool,
        replace_session_id: Option<String>,
    ) -> Result<ConnectOutcome, AppError> {
        let _my_gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;

        // Close any existing session for replace_session_id if specified
        let old_conn = {
            let mut guard = self.inner.write().await;
            if let Some(ref old_sid) = replace_session_id {
                guard.session_statuses.insert(
                    old_sid.clone(),
                    if is_reconnect {
                        SessionStatus::Reconnecting
                    } else {
                        SessionStatus::Connecting
                    },
                );
                guard.sessions.remove(old_sid)
            } else {
                guard.status = if is_reconnect {
                    SessionStatus::Reconnecting
                } else {
                    SessionStatus::Connecting
                };
                guard.last_profile = Some(profile.clone());
                None
            }
        };

        if let Some(conn) = old_conn {
            let _ = conn.close().await;
        }

        let res = match profile.protocol {
            ProtocolType::Sftp => establish_sftp_connection(&profile, secret_override)
                .await
                .map(|r| r.map(ActiveBackendConnection::Sftp)),
            ProtocolType::Scp => establish_scp_connection(&profile, secret_override)
                .await
                .map(|r| r.map(ActiveBackendConnection::Scp)),
            other => {
                return Err(AppError::Protocol(format!(
                    "Protocol {} is not yet supported for connection establishment",
                    other.as_str()
                )));
            }
        };

        match res {
            Ok(Ok(active_conn)) => {
                let session_id = active_conn.session_meta().session_id.0.clone();
                let outcome = ConnectOutcome::Connected {
                    session: Box::new(active_conn.session_meta().clone()),
                    trusted_host_key: active_conn.trusted_host_key().clone(),
                };
                let mut guard = self.inner.write().await;
                guard.status = SessionStatus::Connected;
                guard.active_session_id = Some(session_id.clone());
                guard.last_profile = Some(profile.clone());
                guard
                    .session_statuses
                    .insert(session_id.clone(), SessionStatus::Connected);
                guard.session_profiles.insert(session_id.clone(), profile);
                guard.sessions.insert(session_id, active_conn);
                Ok(outcome)
            }
            Ok(Err(host_key_outcome)) => {
                let mut guard = self.inner.write().await;
                let status = match &host_key_outcome {
                    ConnectOutcome::HostKeyChanged { .. } => SessionStatus::Failed {
                        error_message: "Remote host key has changed! Connection blocked."
                            .to_string(),
                    },
                    _ => SessionStatus::Disconnected,
                };
                guard.status = status.clone();
                if let Some(ref old_sid) = replace_session_id {
                    guard.session_statuses.insert(old_sid.clone(), status);
                }
                Ok(host_key_outcome)
            }
            Err(err) => {
                let mut guard = self.inner.write().await;
                let status = SessionStatus::Failed {
                    error_message: err.to_string(),
                };
                guard.status = status.clone();
                if let Some(ref old_sid) = replace_session_id {
                    guard.session_statuses.insert(old_sid.clone(), status);
                }
                Err(err)
            }
        }
    }

    /// Gracefully disconnects the active session or cancels an in-progress connection.
    pub async fn disconnect(&self) -> Result<SessionStateSnapshot, AppError> {
        self.disconnect_session(None).await
    }

    /// Gracefully disconnects a specific session (or active session if None).
    pub async fn disconnect_session(
        &self,
        session_id: Option<&str>,
    ) -> Result<SessionStateSnapshot, AppError> {
        self.generation.fetch_add(1, Ordering::SeqCst);

        let old_conn = {
            let mut guard = self.inner.write().await;
            let target_id = match session_id {
                Some(sid) => Some(sid.to_string()),
                None => guard.active_session_id.clone(),
            };

            if let Some(sid) = target_id {
                guard
                    .session_statuses
                    .insert(sid.clone(), SessionStatus::Disconnected);
                if guard.active_session_id.as_deref() == Some(&sid) {
                    guard.active_session_id = guard.sessions.keys().find(|&k| k != &sid).cloned();
                    if guard.active_session_id.is_none() {
                        guard.status = SessionStatus::Disconnected;
                    }
                }
                guard.sessions.remove(&sid)
            } else {
                guard.status = SessionStatus::Disconnected;
                None
            }
        };

        if let Some(conn) = old_conn {
            let _ = conn.close().await;
        }

        Ok(self.snapshot_for(session_id).await)
    }

    /// Reconnects to the last used profile (defaults to active session).
    pub async fn reconnect(
        &self,
        secret_override: Option<String>,
    ) -> Result<ConnectOutcome, AppError> {
        self.reconnect_session(secret_override, None).await
    }

    /// Reconnects a specific session (or active session if None).
    pub async fn reconnect_session(
        &self,
        secret_override: Option<String>,
        session_id: Option<&str>,
    ) -> Result<ConnectOutcome, AppError> {
        let (profile, old_sid) = {
            let guard = self.inner.read().await;
            let target_sid = session_id.or(guard.active_session_id.as_deref());
            let prof = target_sid
                .and_then(|sid| guard.session_profiles.get(sid).cloned())
                .or_else(|| guard.last_profile.clone());
            (prof, target_sid.map(|s| s.to_string()))
        };

        let profile = profile.ok_or_else(|| {
            AppError::Validation("No previous connection profile to reconnect".to_string())
        })?;

        self.connect_with_session_id(profile, secret_override, true, old_sid)
            .await
    }

    /// Lists a remote directory using the active or specified session.
    /// Strictly rejects listing if active protocol is SCP (which does not support directory listing).
    pub async fn list_remote_directory(
        &self,
        path: Option<String>,
        show_hidden: bool,
        session_id: Option<&str>,
    ) -> Result<RemoteDirectoryListing, AppError> {
        let (sftp, current_path, home_path, profile_id, timeout_duration, target_session_id) = {
            let guard = self.inner.read().await;
            let target_sid = match session_id {
                Some(sid) => sid,
                None => guard.active_session_id.as_deref().ok_or_else(|| {
                    AppError::Protocol("No active connection established".to_string())
                })?,
            };

            let active = guard.sessions.get(target_sid).ok_or_else(|| {
                AppError::Protocol(format!(
                    "No active connection handle available for session '{}'",
                    target_sid
                ))
            })?;

            match active {
                ActiveBackendConnection::Sftp(sftp_conn) => {
                    let timeout_secs =
                        u64::from(sftp_conn.profile.connection_settings.timeout_seconds.max(5));
                    (
                        Arc::clone(&sftp_conn.sftp_session),
                        sftp_conn.session_meta.current_remote_path.clone(),
                        sftp_conn.session_meta.home_remote_path.clone(),
                        sftp_conn.session_meta.profile_id.clone(),
                        Duration::from_secs(timeout_secs),
                        target_sid.to_string(),
                    )
                }
                ActiveBackendConnection::Scp(_) => {
                    return Err(AppError::Protocol(
                        "Operation 'list_directory' is unsupported by active protocol Scp. SCP is a transfer-only protocol without remote directory browsing capabilities."
                            .to_string(),
                    ));
                }
            }
        };

        let mut listing = list_sftp_directory(
            &sftp,
            path.as_deref(),
            &current_path,
            home_path.as_deref(),
            show_hidden,
            timeout_duration,
        )
        .await?;

        // Resolve user and group names for remote entries using session getent cache
        {
            let guard = self.inner.read().await;
            if let Some(ActiveBackendConnection::Sftp(ref sftp_conn)) =
                guard.sessions.get(&target_session_id)
            {
                resolve_identities_for_entries(
                    &sftp_conn.ssh_handle,
                    &sftp_conn.identity_cache,
                    &mut listing.entries,
                )
                .await;
            }
        }

        // Update current_remote_path in session metadata
        {
            let mut guard = self.inner.write().await;
            if let Some(active) = guard.sessions.get_mut(&target_session_id) {
                active.session_meta_mut().current_remote_path = listing.current_path.clone();
                active.session_meta_mut().last_active_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(active.session_meta().last_active_ms);
            }
        }

        // Persist last remote directory for saved connections
        if let Some(pid) = profile_id {
            let path_to_save = listing.current_path.clone();
            let _ = tokio::task::spawn_blocking(move || {
                if let Ok(db) = Database::open_default() {
                    let repo = RemoteStateRepository::new(&db);
                    let _ = repo.set_last_remote_directory(&pid, &path_to_save);
                }
            })
            .await;
        }

        Ok(listing)
    }

    /// Checks whether a remote path (following symlinks) points to a directory.
    pub async fn check_remote_path_is_dir(
        &self,
        path: String,
        session_id: Option<&str>,
    ) -> Result<bool, AppError> {
        let (sftp, timeout_duration) = {
            let guard = self.inner.read().await;
            let target_sid = match session_id {
                Some(sid) => sid,
                None => guard.active_session_id.as_deref().ok_or_else(|| {
                    AppError::Protocol("No active connection established".to_string())
                })?,
            };
            let active = guard.sessions.get(target_sid).ok_or_else(|| {
                AppError::Protocol(format!(
                    "No active connection handle available for session '{}'",
                    target_sid
                ))
            })?;
            match active {
                ActiveBackendConnection::Sftp(sftp_conn) => {
                    let timeout_secs =
                        u64::from(sftp_conn.profile.connection_settings.timeout_seconds.max(5));
                    (
                        Arc::clone(&sftp_conn.sftp_session),
                        Duration::from_secs(timeout_secs),
                    )
                }
                ActiveBackendConnection::Scp(_) => {
                    return Err(AppError::Protocol(
                        "Checking remote directory type is unsupported by active protocol Scp."
                            .to_string(),
                    ));
                }
            }
        };

        check_sftp_path_is_dir(&sftp, &path, timeout_duration).await
    }

    /// Returns the active transfer context and settings for background transfer execution.
    pub async fn active_transfer_context(&self) -> Result<ActiveTransferContext, AppError> {
        let guard = self.inner.read().await;
        let target_sid = guard
            .active_session_id
            .clone()
            .ok_or_else(|| AppError::Protocol("No active connection established".to_string()))?;
        drop(guard);
        self.transfer_context_for(&target_sid).await
    }

    /// Returns the transfer context for a specific session ID.
    pub async fn transfer_context_for(
        &self,
        session_id: &str,
    ) -> Result<ActiveTransferContext, AppError> {
        let guard = self.inner.read().await;
        let active = guard.sessions.get(session_id).ok_or_else(|| {
            AppError::Protocol(format!(
                "No active connection handle available for session '{}'",
                session_id
            ))
        })?;
        let timeout_secs = u64::from(active.profile().connection_settings.timeout_seconds.max(5));
        let timeout_duration = Duration::from_secs(timeout_secs);

        match active {
            ActiveBackendConnection::Sftp(sftp_conn) => Ok(ActiveTransferContext {
                session_id: sftp_conn.session_meta.session_id.to_string(),
                client: TransferClient::Sftp(Arc::clone(&sftp_conn.sftp_session)),
                sftp: Some(Arc::clone(&sftp_conn.sftp_session)),
                capabilities: sftp_conn.session_meta.capabilities.clone(),
                transfer_settings: sftp_conn.profile.transfer_settings.clone(),
                timeout_duration,
            }),
            ActiveBackendConnection::Scp(scp_conn) => Ok(ActiveTransferContext {
                session_id: scp_conn.session_meta.session_id.to_string(),
                client: TransferClient::Scp {
                    ssh_handle: Arc::clone(&scp_conn.ssh_handle),
                },
                sftp: None,
                capabilities: scp_conn.session_meta.capabilities.clone(),
                transfer_settings: scp_conn.profile.transfer_settings.clone(),
                timeout_duration,
            }),
        }
    }

    async fn active_fs_adapter_for(
        &self,
        session_id: Option<&str>,
    ) -> Result<Box<dyn RemoteFileSystem>, AppError> {
        let guard = self.inner.read().await;
        let target_sid = match session_id {
            Some(sid) => sid,
            None => guard.active_session_id.as_deref().ok_or_else(|| {
                AppError::Protocol("No active connection established".to_string())
            })?,
        };
        let active = guard.sessions.get(target_sid).ok_or_else(|| {
            AppError::Protocol(format!(
                "No active connection handle available for session '{}'",
                target_sid
            ))
        })?;
        match active {
            ActiveBackendConnection::Sftp(sftp_conn) => {
                let timeout_secs =
                    u64::from(sftp_conn.profile.connection_settings.timeout_seconds.max(5));
                Ok(Box::new(SftpFileSystemAdapter::new(
                    Arc::clone(&sftp_conn.sftp_session),
                    sftp_conn.session_meta.capabilities.clone(),
                    Duration::from_secs(timeout_secs),
                )))
            }
            ActiveBackendConnection::Scp(_) => Ok(Box::new(ScpFileSystemAdapter::new())),
        }
    }

    /// Creates a directory on the active remote filesystem behind the `RemoteFileSystem` adapter.
    pub async fn create_remote_directory(
        &self,
        parent_path: &str,
        dir_name: &str,
        session_id: Option<&str>,
    ) -> Result<String, AppError> {
        let valid_name = validate_remote_entry_name(dir_name)?;
        let parent_norm = normalize_posix_path(parent_path.trim());
        let target = join_posix_path(&parent_norm, &valid_name);
        let adapter = self.active_fs_adapter_for(session_id).await?;
        adapter.create_directory(&target).await?;
        Ok(target)
    }

    /// Creates an empty file on the active remote filesystem behind the `RemoteFileSystem` adapter.
    pub async fn create_remote_file(
        &self,
        parent_path: &str,
        file_name: &str,
        session_id: Option<&str>,
    ) -> Result<String, AppError> {
        let valid_name = validate_remote_entry_name(file_name)?;
        let parent_norm = normalize_posix_path(parent_path.trim());
        let target = join_posix_path(&parent_norm, &valid_name);
        let adapter = self.active_fs_adapter_for(session_id).await?;
        adapter.create_file(&target).await?;
        Ok(target)
    }

    /// Renames a remote file or directory within its parent directory.
    pub async fn rename_remote_entry(
        &self,
        source_path: &str,
        new_name: &str,
        session_id: Option<&str>,
    ) -> Result<String, AppError> {
        let valid_name = validate_remote_entry_name(new_name)?;
        let source_norm = normalize_posix_path(source_path.trim());
        let parent = parent_posix_path(&source_norm).ok_or_else(|| {
            AppError::Validation("Cannot rename remote root directory '/'".to_string())
        })?;
        let target = join_posix_path(&parent, &valid_name);
        let adapter = self.active_fs_adapter_for(session_id).await?;
        adapter.rename(&source_norm, &target).await?;
        Ok(target)
    }

    /// Deletes a single remote file behind the `RemoteFileSystem` adapter.
    pub async fn delete_remote_file(
        &self,
        path: &str,
        session_id: Option<&str>,
    ) -> Result<(), AppError> {
        let adapter = self.active_fs_adapter_for(session_id).await?;
        adapter.delete_file(path).await
    }

    /// Deletes a remote directory (non-recursive unless `recursive == true`).
    pub async fn delete_remote_directory(
        &self,
        path: &str,
        recursive: bool,
        session_id: Option<&str>,
    ) -> Result<(), AppError> {
        let adapter = self.active_fs_adapter_for(session_id).await?;
        adapter.delete_directory(path, recursive).await
    }

    /// Sets Unix permissions on one or more remote paths via `RemoteFileSystem`.
    pub async fn set_remote_permissions(
        &self,
        paths: Vec<String>,
        octal_mode: &str,
        recursive: bool,
        session_id: Option<&str>,
    ) -> Result<(), AppError> {
        if paths.is_empty() {
            return Err(AppError::Validation(
                "At least one remote path is required".to_string(),
            ));
        }
        let perms: UnixPermissions = parse_octal_mode(octal_mode)?;
        let adapter = self.active_fs_adapter_for(session_id).await?;
        adapter
            .capabilities()
            .verify_operation("set_permissions")
            .map_err(|e| AppError::Protocol(e.to_string()))?;

        let ctx = match session_id {
            Some(sid) => self.transfer_context_for(sid).await?,
            None => self.active_transfer_context().await?,
        };
        match &ctx.client {
            TransferClient::Sftp(sftp) => {
                let sftp_adapter = SftpFileSystemAdapter::new(
                    Arc::clone(sftp),
                    ctx.capabilities,
                    ctx.timeout_duration,
                );
                apply_remote_permissions_with_adapter(
                    &sftp_adapter,
                    sftp,
                    &paths,
                    perms,
                    recursive,
                    ctx.timeout_duration,
                )
                .await
            }
            TransferClient::Scp { .. } => Err(AppError::Protocol(
                "Setting permissions is not supported by SCP protocol".to_string(),
            )),
        }
    }

    /// Sets numeric UID/GID (or resolves names via optional `getent`) on a remote path.
    pub async fn set_remote_ownership(
        &self,
        path: &str,
        uid: Option<u32>,
        gid: Option<u32>,
        user_name: Option<String>,
        group_name: Option<String>,
        session_id: Option<&str>,
    ) -> Result<(), AppError> {
        let adapter = self.active_fs_adapter_for(session_id).await?;
        adapter
            .capabilities()
            .verify_operation("set_owner_group")
            .map_err(|e| AppError::Protocol(e.to_string()))?;

        let (resolved_uid, resolved_gid) = {
            let guard = self.inner.read().await;
            let target_sid = match session_id {
                Some(sid) => sid,
                None => guard.active_session_id.as_deref().ok_or_else(|| {
                    AppError::Protocol("No active connection established".to_string())
                })?,
            };
            let active = guard.sessions.get(target_sid).ok_or_else(|| {
                AppError::Protocol(format!(
                    "No active connection handle available for session '{}'",
                    target_sid
                ))
            })?;

            let ssh_handle = match active {
                ActiveBackendConnection::Sftp(sftp) => &sftp.ssh_handle,
                ActiveBackendConnection::Scp(_) => {
                    return Err(AppError::Protocol(
                        "Setting remote ownership is unsupported by SCP protocol".to_string(),
                    ));
                }
            };

            let u = match uid {
                Some(id) => Some(id),
                None => match user_name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                    Some(name) => Some(
                        resolve_name_to_id_via_getent(ssh_handle, "passwd", name)
                            .await
                            .map_err(|e| {
                                AppError::Validation(format!(
                                    "Could not resolve user '{}' via remote getent ({}). Enter a numeric UID instead.",
                                    name, e
                                ))
                            })?,
                    ),
                    None => None,
                },
            };

            let g = match gid {
                Some(id) => Some(id),
                None => match group_name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                    Some(name) => Some(
                        resolve_name_to_id_via_getent(ssh_handle, "group", name)
                            .await
                            .map_err(|e| {
                                AppError::Validation(format!(
                                    "Could not resolve group '{}' via remote getent ({}). Enter a numeric GID instead.",
                                    name, e
                                ))
                            })?,
                    ),
                    None => None,
                },
            };

            (u, g)
        };

        adapter
            .set_owner_group(
                path,
                FileOwnership {
                    uid: resolved_uid,
                    gid: resolved_gid,
                    user_name: None,
                    group_name: None,
                },
            )
            .await
    }

    /// Optional, controlled `getent` lookup to display user/group names for numeric UID/GID.
    pub async fn resolve_remote_identity(
        &self,
        uid: Option<u32>,
        gid: Option<u32>,
        session_id: Option<&str>,
    ) -> Result<FileOwnership, AppError> {
        let guard = self.inner.read().await;
        let target_sid = match session_id {
            Some(sid) => sid,
            None => guard.active_session_id.as_deref().ok_or_else(|| {
                AppError::Protocol("No active connection established".to_string())
            })?,
        };
        let active = guard.sessions.get(target_sid).ok_or_else(|| {
            AppError::Protocol(format!(
                "No active connection handle available for session '{}'",
                target_sid
            ))
        })?;
        match active {
            ActiveBackendConnection::Sftp(sftp) => {
                let cached = {
                    let cache = sftp.identity_cache.lock().await;
                    let u = uid.and_then(|u| cache.uids.get(&u).cloned().flatten());
                    let g = gid.and_then(|g| cache.gids.get(&g).cloned().flatten());
                    (u, g)
                };

                let user_name = if let Some(un) = cached.0 {
                    Some(un)
                } else if let Some(u) = uid {
                    let res =
                        resolve_remote_identity_via_getent(&sftp.ssh_handle, Some(u), None).await;
                    let mut cache = sftp.identity_cache.lock().await;
                    cache.uids.insert(u, res.user_name.clone());
                    res.user_name
                } else {
                    None
                };

                let group_name = if let Some(gn) = cached.1 {
                    Some(gn)
                } else if let Some(g) = gid {
                    let res =
                        resolve_remote_identity_via_getent(&sftp.ssh_handle, None, Some(g)).await;
                    let mut cache = sftp.identity_cache.lock().await;
                    cache.gids.insert(g, res.group_name.clone());
                    res.group_name
                } else {
                    None
                };

                Ok(FileOwnership {
                    uid,
                    gid,
                    user_name,
                    group_name,
                })
            }
            ActiveBackendConnection::Scp(scp) => {
                let handle = scp.ssh_handle.lock().await;
                Ok(resolve_remote_identity_via_getent(&handle, uid, gid).await)
            }
        }
    }

    #[cfg(test)]
    pub async fn set_test_status(&self, status: SessionStatus) {
        let mut guard = self.inner.write().await;
        guard.status = status;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_manager_initial_and_disconnect_states() {
        let manager = SessionManager::new();
        let snap = manager.snapshot().await;
        assert_eq!(snap.status, SessionStatus::Disconnected);
        assert!(snap.session.is_none());

        manager.set_test_status(SessionStatus::Connecting).await;
        assert_eq!(manager.snapshot().await.status, SessionStatus::Connecting);

        let after_disc = manager.disconnect().await.unwrap();
        assert_eq!(after_disc.status, SessionStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_reconnect_without_prior_profile_fails_cleanly() {
        let manager = SessionManager::new();
        let res = manager.reconnect(None).await;
        assert!(res.is_err());
        assert!(matches!(res.unwrap_err(), AppError::Validation(_)));
    }

    #[tokio::test]
    async fn test_unsupported_protocols_fail_safely_without_unverified_fallback() {
        let manager = SessionManager::new();

        let mut ftps_profile = ConnectionProfile::new(
            "Test FTPS".to_string(),
            crate::models::ProtocolType::Ftps,
            "ftps.example.com".to_string(),
            "user".to_string(),
            crate::models::AuthMethod::Password,
        );
        ftps_profile.id = "ftps-test-id".to_string();

        let ftps_res = manager.connect(ftps_profile, None, false).await;
        assert!(ftps_res.is_err());
        match ftps_res.unwrap_err() {
            AppError::Protocol(msg) => {
                assert!(msg.contains("FTPS is not yet supported"));
            }
            other => panic!("Expected Protocol error for FTPS, got {:?}", other),
        }

        let mut ftp_profile = ConnectionProfile::new(
            "Test FTP".to_string(),
            crate::models::ProtocolType::Ftp,
            "ftp.example.com".to_string(),
            "user".to_string(),
            crate::models::AuthMethod::Password,
        );
        ftp_profile.id = "ftp-test-id".to_string();

        let ftp_res = manager.connect(ftp_profile, None, false).await;
        assert!(ftp_res.is_err());
        match ftp_res.unwrap_err() {
            AppError::Protocol(msg) => {
                assert!(msg.contains("FTP is not yet supported"));
            }
            other => panic!("Expected Protocol error for FTP, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_multi_session_state_and_disconnect_isolation() {
        let manager = SessionManager::new();
        assert_eq!(manager.session_count().await, 0);
        assert!(manager.active_session_id().await.is_none());

        // Test snapshot_for unknown returns Disconnected
        let snap = manager.snapshot_for(Some("non-existent-tab")).await;
        assert_eq!(snap.status, SessionStatus::Disconnected);

        // Inject simulated tab statuses
        {
            let mut guard = manager.inner.write().await;
            guard
                .session_statuses
                .insert("tab-1".to_string(), SessionStatus::Connected);
            guard
                .session_statuses
                .insert("tab-2".to_string(), SessionStatus::Connected);
            guard.active_session_id = Some("tab-1".to_string());
            guard.status = SessionStatus::Connected;
        }

        assert_eq!(manager.active_session_id().await.as_deref(), Some("tab-1"));
        assert_eq!(
            manager.snapshot_for(Some("tab-1")).await.status,
            SessionStatus::Connected
        );
        assert_eq!(
            manager.snapshot_for(Some("tab-2")).await.status,
            SessionStatus::Connected
        );

        // Disconnecting tab-1 isolates tab-2
        let snap1 = manager.disconnect_session(Some("tab-1")).await.unwrap();
        assert_eq!(snap1.status, SessionStatus::Disconnected);
        assert_eq!(
            manager.snapshot_for(Some("tab-2")).await.status,
            SessionStatus::Connected
        );
    }

    #[tokio::test]
    async fn test_concurrent_live_sessions_and_tab_disconnect_isolation() {
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

        let manager = SessionManager::new();

        let profile_1 = ConnectionProfile::new(
            "Tab 1 VM".to_string(),
            ProtocolType::Sftp,
            test_host.to_string(),
            "ubuntu".to_string(),
            crate::models::AuthMethod::PublicKey {
                private_key_path: "~/.ssh/id_rsa".to_string(),
                requires_passphrase: false,
            },
        );

        let profile_2 = ConnectionProfile::new(
            "Tab 2 VM".to_string(),
            ProtocolType::Sftp,
            test_host.to_string(),
            "ubuntu".to_string(),
            crate::models::AuthMethod::PublicKey {
                private_key_path: "~/.ssh/id_rsa".to_string(),
                requires_passphrase: false,
            },
        );

        // Connect Session 1
        let outcome_1 = manager
            .connect_with_session_id(profile_1, None, false, None)
            .await;
        let sid_1 = match outcome_1 {
            Ok(ConnectOutcome::Connected { session, .. }) => session.session_id.0,
            _ => return,
        };

        // Connect Session 2 concurrently
        let outcome_2 = manager
            .connect_with_session_id(profile_2, None, false, None)
            .await;
        let sid_2 = match outcome_2 {
            Ok(ConnectOutcome::Connected { session, .. }) => session.session_id.0,
            _ => return,
        };

        assert_ne!(
            sid_1, sid_2,
            "Concurrent tabs must have distinct session IDs"
        );
        assert_eq!(manager.session_count().await, 2);
        assert!(manager.has_session(&sid_1).await);
        assert!(manager.has_session(&sid_2).await);

        // Both sessions can list remote directory independently
        let listing_1 = manager
            .list_remote_directory(Some("/tmp".to_string()), false, Some(&sid_1))
            .await;
        assert!(listing_1.is_ok());

        let listing_2 = manager
            .list_remote_directory(Some("/etc".to_string()), false, Some(&sid_2))
            .await;
        assert!(listing_2.is_ok());

        // Tab switching: set active session to sid_1
        manager.set_active_session_id(&sid_1).await;
        assert_eq!(
            manager.active_session_id().await.as_deref(),
            Some(sid_1.as_str())
        );

        // Disconnect Tab 1 ONLY
        let snap_disc_1 = manager.disconnect_session(Some(&sid_1)).await.unwrap();
        assert_eq!(snap_disc_1.status, SessionStatus::Disconnected);

        // Tab 1 is gone, but Tab 2 remains actively connected!
        assert!(!manager.has_session(&sid_1).await);
        assert!(manager.has_session(&sid_2).await);
        assert_eq!(manager.session_count().await, 1);
        assert_eq!(
            manager.snapshot_for(Some(&sid_2)).await.status,
            SessionStatus::Connected
        );

        // Tab 2 can still execute remote commands
        let listing_2_again = manager
            .list_remote_directory(Some("/tmp".to_string()), false, Some(&sid_2))
            .await;
        assert!(listing_2_again.is_ok());

        // Now disconnect Tab 2
        let snap_disc_2 = manager.disconnect_session(Some(&sid_2)).await.unwrap();
        assert_eq!(snap_disc_2.status, SessionStatus::Disconnected);
        assert_eq!(manager.session_count().await, 0);
    }
}
