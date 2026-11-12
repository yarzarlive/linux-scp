use crate::errors::AppError;
use crate::models::{
    ConnectionProfile, ConnectionSession, FileOwnership, ProfileTransferSettings,
    ProtocolCapabilities, SessionStatus, UnixPermissions,
};
use crate::persistence::{Database, KnownHostEntry, RemoteStateRepository};
use crate::protocol::sftp::{
    apply_remote_permissions_with_adapter, check_sftp_path_is_dir, establish_sftp_connection,
    join_posix_path, list_sftp_directory, normalize_posix_path, parent_posix_path,
    parse_octal_mode, resolve_name_to_id_via_getent, resolve_remote_identity_via_getent,
    validate_remote_entry_name, ActiveSftpConnection, ConnectOutcome, RemoteDirectoryListing,
    SftpFileSystemAdapter,
};
use crate::protocol::RemoteFileSystem;
use russh_sftp::client::SftpSession;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Active SFTP context needed by background transfer jobs.
#[derive(Clone)]
pub struct ActiveTransferContext {
    pub session_id: String,
    pub sftp: Arc<SftpSession>,
    pub capabilities: ProtocolCapabilities,
    pub transfer_settings: ProfileTransferSettings,
    pub timeout_duration: Duration,
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
    active: Option<ActiveSftpConnection>,
    last_profile: Option<ConnectionProfile>,
}

/// Thread-safe manager tracking the SFTP connection lifecycle, cancellation, and active session handle.
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
                active: None,
                last_profile: None,
            })),
            generation: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Returns a snapshot of the current connection lifecycle state.
    pub async fn snapshot(&self) -> SessionStateSnapshot {
        let guard = self.inner.read().await;
        SessionStateSnapshot {
            status: guard.status.clone(),
            session: guard.active.as_ref().map(|a| a.session_meta.clone()),
            trusted_host_key: guard.active.as_ref().map(|a| a.trusted_host_key.clone()),
        }
    }

    /// Connects to an SFTP server using the given profile, replacing any previous connection.
    pub async fn connect(
        &self,
        profile: ConnectionProfile,
        secret_override: Option<String>,
        is_reconnect: bool,
    ) -> Result<ConnectOutcome, AppError> {
        let my_gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;

        // Close any existing session first
        let old_conn = {
            let mut guard = self.inner.write().await;
            guard.status = if is_reconnect {
                SessionStatus::Reconnecting
            } else {
                SessionStatus::Connecting
            };
            guard.last_profile = Some(profile.clone());
            guard.active.take()
        };

        if let Some(conn) = old_conn {
            let _ = conn.close().await;
        }

        let res = establish_sftp_connection(&profile, secret_override).await;

        // Check if cancelled or superseded while connecting
        if self.generation.load(Ordering::SeqCst) != my_gen {
            if let Ok(Ok(conn)) = res {
                let _ = conn.close().await;
            }
            return Err(AppError::Protocol(
                "Connection attempt was cancelled".to_string(),
            ));
        }

        match res {
            Ok(Ok(active_conn)) => {
                let outcome = ConnectOutcome::Connected {
                    session: Box::new(active_conn.session_meta.clone()),
                    trusted_host_key: active_conn.trusted_host_key.clone(),
                };
                let mut guard = self.inner.write().await;
                guard.status = SessionStatus::Connected;
                guard.active = Some(active_conn);
                Ok(outcome)
            }
            Ok(Err(host_key_outcome)) => {
                let mut guard = self.inner.write().await;
                guard.status = match &host_key_outcome {
                    ConnectOutcome::HostKeyChanged { .. } => SessionStatus::Failed {
                        error_message: "Remote host key has changed! Connection blocked."
                            .to_string(),
                    },
                    _ => SessionStatus::Disconnected,
                };
                Ok(host_key_outcome)
            }
            Err(err) => {
                let mut guard = self.inner.write().await;
                guard.status = SessionStatus::Failed {
                    error_message: err.to_string(),
                };
                Err(err)
            }
        }
    }

    /// Gracefully disconnects the active SFTP session or cancels an in-progress connection.
    pub async fn disconnect(&self) -> Result<SessionStateSnapshot, AppError> {
        self.generation.fetch_add(1, Ordering::SeqCst);

        let old_conn = {
            let mut guard = self.inner.write().await;
            guard.status = SessionStatus::Disconnected;
            guard.active.take()
        };

        if let Some(conn) = old_conn {
            let _ = conn.close().await;
        }

        Ok(self.snapshot().await)
    }

    /// Reconnects using the last attempted connection profile.
    pub async fn reconnect(
        &self,
        secret_override: Option<String>,
    ) -> Result<ConnectOutcome, AppError> {
        let profile = {
            let guard = self.inner.read().await;
            guard.last_profile.clone()
        }
        .ok_or_else(|| {
            AppError::Validation("No previous connection profile to reconnect".to_string())
        })?;

        self.connect(profile, secret_override, true).await
    }

    /// Lists a remote directory using the active SFTP session without holding the manager lock across I/O.
    pub async fn list_remote_directory(
        &self,
        path: Option<String>,
        show_hidden: bool,
    ) -> Result<RemoteDirectoryListing, AppError> {
        let (sftp, current_path, home_path, profile_id, timeout_duration) = {
            let guard = self.inner.read().await;
            if guard.status != SessionStatus::Connected {
                return Err(AppError::Protocol(
                    "No active SFTP connection established".to_string(),
                ));
            }
            let active = guard.active.as_ref().ok_or_else(|| {
                AppError::Protocol("No active SFTP session handle available".to_string())
            })?;
            let timeout_secs = u64::from(active.profile.connection_settings.timeout_seconds.max(5));
            (
                Arc::clone(&active.sftp_session),
                active.session_meta.current_remote_path.clone(),
                active.session_meta.home_remote_path.clone(),
                active.session_meta.profile_id.clone(),
                Duration::from_secs(timeout_secs),
            )
        };

        let listing = list_sftp_directory(
            &sftp,
            path.as_deref(),
            &current_path,
            home_path.as_deref(),
            show_hidden,
            timeout_duration,
        )
        .await?;

        // Update current_remote_path in session metadata
        {
            let mut guard = self.inner.write().await;
            if let Some(active) = guard.active.as_mut() {
                active.session_meta.current_remote_path = listing.current_path.clone();
                active.session_meta.last_active_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(active.session_meta.last_active_ms);
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
    pub async fn check_remote_path_is_dir(&self, path: String) -> Result<bool, AppError> {
        let (sftp, timeout_duration) = {
            let guard = self.inner.read().await;
            if guard.status != SessionStatus::Connected {
                return Err(AppError::Protocol(
                    "No active SFTP connection established".to_string(),
                ));
            }
            let active = guard.active.as_ref().ok_or_else(|| {
                AppError::Protocol("No active SFTP session handle available".to_string())
            })?;
            let timeout_secs = u64::from(active.profile.connection_settings.timeout_seconds.max(5));
            (
                Arc::clone(&active.sftp_session),
                Duration::from_secs(timeout_secs),
            )
        };

        check_sftp_path_is_dir(&sftp, &path, timeout_duration).await
    }

    /// Returns the active SFTP session and transfer settings for background transfer execution.
    pub async fn active_transfer_context(&self) -> Result<ActiveTransferContext, AppError> {
        let guard = self.inner.read().await;
        if guard.status != SessionStatus::Connected {
            return Err(AppError::Protocol(
                "No active SFTP connection established".to_string(),
            ));
        }
        let active = guard.active.as_ref().ok_or_else(|| {
            AppError::Protocol("No active SFTP session handle available".to_string())
        })?;
        let timeout_secs = u64::from(active.profile.connection_settings.timeout_seconds.max(5));
        Ok(ActiveTransferContext {
            session_id: active.session_meta.session_id.to_string(),
            sftp: Arc::clone(&active.sftp_session),
            capabilities: active.session_meta.capabilities.clone(),
            transfer_settings: active.profile.transfer_settings.clone(),
            timeout_duration: Duration::from_secs(timeout_secs),
        })
    }

    async fn active_fs_adapter(&self) -> Result<Box<dyn RemoteFileSystem>, AppError> {
        let ctx = self.active_transfer_context().await?;
        Ok(Box::new(SftpFileSystemAdapter::new(
            ctx.sftp,
            ctx.capabilities,
            ctx.timeout_duration,
        )))
    }

    /// Creates a directory on the active remote filesystem behind the `RemoteFileSystem` adapter.
    pub async fn create_remote_directory(
        &self,
        parent_path: &str,
        dir_name: &str,
    ) -> Result<String, AppError> {
        let valid_name = validate_remote_entry_name(dir_name)?;
        let parent_norm = normalize_posix_path(parent_path.trim());
        let target = join_posix_path(&parent_norm, &valid_name);
        let adapter = self.active_fs_adapter().await?;
        adapter.create_directory(&target).await?;
        Ok(target)
    }

    /// Creates an empty file on the active remote filesystem behind the `RemoteFileSystem` adapter.
    pub async fn create_remote_file(
        &self,
        parent_path: &str,
        file_name: &str,
    ) -> Result<String, AppError> {
        let valid_name = validate_remote_entry_name(file_name)?;
        let parent_norm = normalize_posix_path(parent_path.trim());
        let target = join_posix_path(&parent_norm, &valid_name);
        let adapter = self.active_fs_adapter().await?;
        adapter.create_file(&target).await?;
        Ok(target)
    }

    /// Renames a remote file or directory within its parent directory.
    pub async fn rename_remote_entry(
        &self,
        source_path: &str,
        new_name: &str,
    ) -> Result<String, AppError> {
        let valid_name = validate_remote_entry_name(new_name)?;
        let source_norm = normalize_posix_path(source_path.trim());
        let parent = parent_posix_path(&source_norm).ok_or_else(|| {
            AppError::Validation("Cannot rename remote root directory '/'".to_string())
        })?;
        let target = join_posix_path(&parent, &valid_name);
        let adapter = self.active_fs_adapter().await?;
        adapter.rename(&source_norm, &target).await?;
        Ok(target)
    }

    /// Deletes a single remote file behind the `RemoteFileSystem` adapter.
    pub async fn delete_remote_file(&self, path: &str) -> Result<(), AppError> {
        let adapter = self.active_fs_adapter().await?;
        adapter.delete_file(path).await
    }

    /// Deletes a remote directory (non-recursive unless `recursive == true`).
    pub async fn delete_remote_directory(
        &self,
        path: &str,
        recursive: bool,
    ) -> Result<(), AppError> {
        let adapter = self.active_fs_adapter().await?;
        adapter.delete_directory(path, recursive).await
    }

    /// Sets Unix permissions on one or more remote paths via `RemoteFileSystem`.
    pub async fn set_remote_permissions(
        &self,
        paths: Vec<String>,
        octal_mode: &str,
        recursive: bool,
    ) -> Result<(), AppError> {
        if paths.is_empty() {
            return Err(AppError::Validation(
                "At least one remote path is required".to_string(),
            ));
        }
        let perms: UnixPermissions = parse_octal_mode(octal_mode)?;
        let ctx = self.active_transfer_context().await?;
        let adapter = SftpFileSystemAdapter::new(
            Arc::clone(&ctx.sftp),
            ctx.capabilities,
            ctx.timeout_duration,
        );
        apply_remote_permissions_with_adapter(
            &adapter,
            &ctx.sftp,
            &paths,
            perms,
            recursive,
            ctx.timeout_duration,
        )
        .await
    }

    /// Sets numeric UID/GID (or resolves names via optional `getent`) on a remote path.
    pub async fn set_remote_ownership(
        &self,
        path: &str,
        uid: Option<u32>,
        gid: Option<u32>,
        user_name: Option<String>,
        group_name: Option<String>,
    ) -> Result<(), AppError> {
        let (resolved_uid, resolved_gid) = {
            let guard = self.inner.read().await;
            if guard.status != SessionStatus::Connected {
                return Err(AppError::Protocol(
                    "No active SFTP connection established".to_string(),
                ));
            }
            let active = guard.active.as_ref().ok_or_else(|| {
                AppError::Protocol("No active SFTP session handle available".to_string())
            })?;

            let u = match uid {
                Some(id) => Some(id),
                None => match user_name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                    Some(name) => Some(
                        resolve_name_to_id_via_getent(&active.ssh_handle, "passwd", name)
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
                        resolve_name_to_id_via_getent(&active.ssh_handle, "group", name)
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

        let adapter = self.active_fs_adapter().await?;
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
    ) -> Result<FileOwnership, AppError> {
        let guard = self.inner.read().await;
        if guard.status != SessionStatus::Connected {
            return Err(AppError::Protocol(
                "No active SFTP connection established".to_string(),
            ));
        }
        let active = guard.active.as_ref().ok_or_else(|| {
            AppError::Protocol("No active SFTP session handle available".to_string())
        })?;
        Ok(resolve_remote_identity_via_getent(&active.ssh_handle, uid, gid).await)
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
}
