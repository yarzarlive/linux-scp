//! SFTP implementation of [`super::manager::RemoteEditBackend`] and Tauri event sink.

use super::manager::{BackendError, RemoteEditBackend, RemoteEditEventSink, RemoteEditManager};
use super::types::{
    RemoteEditSession, RemoteEndpointIdentity, RemoteFileFingerprint, RemoteFileStat,
    RemoteObjectKind,
};
use crate::errors::AppError;
use crate::models::SessionStatus;
use crate::protocol::sftp::{classify_sftp_error, normalize_posix_path, resolve_remote_input_path};
use crate::protocol::SessionManager;
use async_trait::async_trait;
use russh_sftp::client::error::Error as SftpClientError;
use russh_sftp::protocol::{FileAttributes, OpenFlags, StatusCode};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

const CHUNK_SIZE: usize = 64 * 1024;

/// Tauri event sink broadcasting remote edit session updates to the frontend.
pub struct TauriRemoteEditEventSink {
    app_handle: AppHandle,
}

impl TauriRemoteEditEventSink {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }
}

impl RemoteEditEventSink for TauriRemoteEditEventSink {
    fn emit_updated(&self, session: &RemoteEditSession) {
        let _ = self.app_handle.emit("remote-edit://updated", session);
    }

    fn emit_removed(&self, session_id: &str) {
        let _ = self.app_handle.emit("remote-edit://removed", session_id);
    }
}

/// Spawns a background poller that inspects local working copies every `interval`.
pub fn spawn_background_watcher(manager: RemoteEditManager, interval: Duration) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let _ = manager.scan_local_changes(true).await;
        }
    });
}

/// SFTP-backed implementation of [`RemoteEditBackend`] bound to [`SessionManager`].
pub struct SftpRemoteEditBackend {
    session_manager: SessionManager,
}

impl SftpRemoteEditBackend {
    pub fn new(session_manager: SessionManager) -> Arc<Self> {
        Arc::new(Self { session_manager })
    }
}

fn map_sftp_err(err: SftpClientError, path: &str) -> BackendError {
    match &err {
        SftpClientError::Status(status) => match status.status_code {
            StatusCode::NoSuchFile => BackendError::NotFound(format!(
                "Remote path does not exist '{}': {}",
                path, status.error_message
            )),
            StatusCode::NoConnection | StatusCode::ConnectionLost => {
                BackendError::Disconnected(format!(
                    "SFTP connection lost while accessing '{}': {}",
                    path, status.error_message
                ))
            }
            _ => BackendError::Other(classify_sftp_error(err, path)),
        },
        SftpClientError::Timeout => {
            BackendError::Disconnected(format!("SFTP request timed out while accessing '{}'", path))
        }
        SftpClientError::IO(msg) => BackendError::Disconnected(format!(
            "Network I/O error while accessing '{}': {}",
            path, msg
        )),
        _ => BackendError::Other(classify_sftp_error(err, path)),
    }
}

#[async_trait]
impl RemoteEditBackend for SftpRemoteEditBackend {
    async fn current_endpoint(
        &self,
    ) -> Result<(RemoteEndpointIdentity, Option<String>), BackendError> {
        let snap = self.session_manager.snapshot().await;
        if snap.status != SessionStatus::Connected {
            return Err(BackendError::Disconnected(
                "No active SFTP connection established.".to_string(),
            ));
        }
        let session = snap.session.ok_or_else(|| {
            BackendError::Disconnected("No active SFTP session handle available.".to_string())
        })?;
        Ok((
            RemoteEndpointIdentity {
                host: session.host,
                port: session.port,
                username: session.username,
                elevated: session.elevation.is_elevated(),
            },
            session.profile_id,
        ))
    }

    async fn canonicalize(&self, path: &str) -> Result<String, BackendError> {
        let snap = self.session_manager.snapshot().await;
        let ctx = self
            .session_manager
            .active_transfer_context()
            .await
            .map_err(|e| BackendError::Disconnected(e.to_string()))?;

        let (current_dir, home_dir) = snap
            .session
            .map(|s| (s.current_remote_path, s.home_remote_path))
            .unwrap_or_else(|| ("/".to_string(), None));

        let resolved = resolve_remote_input_path(path, &current_dir, home_dir.as_deref());
        let canon = tokio::time::timeout(ctx.timeout_duration, ctx.sftp.canonicalize(&resolved))
            .await
            .map_err(|_| {
                BackendError::Disconnected(format!(
                    "Timed out resolving remote path '{}'",
                    resolved
                ))
            })?
            .map_err(|e| map_sftp_err(e, &resolved))?;
        Ok(normalize_posix_path(&canon))
    }

    async fn stat(&self, path: &str) -> Result<RemoteFileStat, BackendError> {
        let ctx = self
            .session_manager
            .active_transfer_context()
            .await
            .map_err(|e| BackendError::Disconnected(e.to_string()))?;

        let attrs = tokio::time::timeout(ctx.timeout_duration, ctx.sftp.metadata(path))
            .await
            .map_err(|_| {
                BackendError::Disconnected(format!("Timed out stating remote path '{}'", path))
            })?
            .map_err(|e| map_sftp_err(e, path))?;

        let kind = if attrs.is_dir() {
            RemoteObjectKind::Directory
        } else if attrs.is_regular() || attrs.permissions.is_none() {
            RemoteObjectKind::Regular
        } else {
            RemoteObjectKind::Other
        };

        Ok(RemoteFileStat {
            kind,
            fingerprint: RemoteFileFingerprint {
                size: attrs.size,
                modified_at_secs: attrs.mtime.map(u64::from),
                permissions_mode: attrs.permissions.map(|m| m & 0o7777),
                uid: attrs.uid,
                gid: attrs.gid,
            },
        })
    }

    async fn download_to(&self, path: &str, dest: &Path) -> Result<u64, BackendError> {
        let ctx = self
            .session_manager
            .active_transfer_context()
            .await
            .map_err(|e| BackendError::Disconnected(e.to_string()))?;

        let mut remote_file = tokio::time::timeout(ctx.timeout_duration, ctx.sftp.open(path))
            .await
            .map_err(|_| {
                BackendError::Disconnected(format!("Timed out opening remote file '{}'", path))
            })?
            .map_err(|e| map_sftp_err(e, path))?;

        let mut local_file = tokio::fs::File::create(dest).await.map_err(|e| {
            BackendError::Other(AppError::Filesystem(format!(
                "Failed to create local file '{}': {}",
                dest.display(),
                e
            )))
        })?;

        let mut buf = vec![0u8; CHUNK_SIZE];
        let mut total = 0u64;

        let res: Result<(), BackendError> = async {
            loop {
                let n = tokio::time::timeout(ctx.timeout_duration, remote_file.read(&mut buf))
                    .await
                    .map_err(|_| {
                        BackendError::Disconnected(format!(
                            "Timed out reading remote file '{}'",
                            path
                        ))
                    })?
                    .map_err(|e| {
                        BackendError::Disconnected(format!(
                            "Failed reading remote file '{}': {}",
                            path, e
                        ))
                    })?;
                if n == 0 {
                    break;
                }
                local_file.write_all(&buf[..n]).await.map_err(|e| {
                    BackendError::Other(AppError::Filesystem(format!(
                        "Failed writing temporary file '{}': {}",
                        dest.display(),
                        e
                    )))
                })?;
                total += n as u64;
            }
            local_file.flush().await.map_err(|e| {
                BackendError::Other(AppError::Filesystem(format!(
                    "Failed flushing temporary file '{}': {}",
                    dest.display(),
                    e
                )))
            })?;
            local_file.sync_all().await.map_err(|e| {
                BackendError::Other(AppError::Filesystem(format!(
                    "Failed syncing temporary file '{}': {}",
                    dest.display(),
                    e
                )))
            })?;
            Ok(())
        }
        .await;

        drop(local_file);
        let _ = remote_file.close().await;

        if let Err(e) = res {
            let _ = tokio::fs::remove_file(dest).await;
            return Err(e);
        }

        Ok(total)
    }

    async fn upload_replace(
        &self,
        src: &Path,
        path: &str,
        mode: Option<u32>,
    ) -> Result<(), BackendError> {
        let ctx = self
            .session_manager
            .active_transfer_context()
            .await
            .map_err(|e| BackendError::Disconnected(e.to_string()))?;

        let expected_size = tokio::fs::metadata(src)
            .await
            .map_err(|e| {
                BackendError::Other(AppError::Filesystem(format!(
                    "Cannot inspect local upload source '{}': {}",
                    src.display(),
                    e
                )))
            })?
            .len();

        let temp_remote = format!("{}.linuxscp-edit-tmp-{}", path, Uuid::new_v4());

        // Fast & safest path: write to a sibling temporary file and promote into place.
        let stage_res = self
            .write_remote_stream(&ctx, src, &temp_remote, expected_size, true)
            .await;

        match stage_res {
            Ok(()) => {
                if let Some(m) = mode {
                    if ctx.capabilities.can_set_permissions {
                        let attrs = FileAttributes {
                            permissions: Some(m & 0o7777),
                            ..Default::default()
                        };
                        let _ = tokio::time::timeout(
                            ctx.timeout_duration,
                            ctx.sftp.set_metadata(temp_remote.as_str(), attrs),
                        )
                        .await;
                    }
                }

                // SFTPv3 rename fails if destination exists; remove destination and rename temp into place.
                let _ =
                    tokio::time::timeout(ctx.timeout_duration, ctx.sftp.remove_file(path)).await;
                match tokio::time::timeout(
                    ctx.timeout_duration,
                    ctx.sftp.rename(temp_remote.as_str(), path),
                )
                .await
                {
                    Ok(Ok(())) => return Ok(()),
                    Ok(Err(e)) => {
                        let _ = ctx.sftp.remove_file(temp_remote.as_str()).await;
                        return Err(map_sftp_err(e, path));
                    }
                    Err(_) => {
                        let _ = ctx.sftp.remove_file(temp_remote.as_str()).await;
                        return Err(BackendError::Disconnected(format!(
                            "Timed out promoting uploaded file '{}'",
                            path
                        )));
                    }
                }
            }
            Err(BackendError::Other(AppError::PermissionDenied(_))) => {
                // Directory does not allow creating sibling files, but the target file itself
                // may be writable by the user. Fall back to in-place overwrite.
                self.write_remote_stream(&ctx, src, path, expected_size, false)
                    .await?;
                if let Some(m) = mode {
                    if ctx.capabilities.can_set_permissions {
                        let attrs = FileAttributes {
                            permissions: Some(m & 0o7777),
                            ..Default::default()
                        };
                        let _ = tokio::time::timeout(
                            ctx.timeout_duration,
                            ctx.sftp.set_metadata(path, attrs),
                        )
                        .await;
                    }
                }
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

impl SftpRemoteEditBackend {
    async fn write_remote_stream(
        &self,
        ctx: &crate::protocol::ActiveTransferContext,
        src: &Path,
        dest_remote: &str,
        expected_size: u64,
        remove_on_error: bool,
    ) -> Result<(), BackendError> {
        let mut local_file = tokio::fs::File::open(src).await.map_err(|e| {
            BackendError::Other(AppError::Filesystem(format!(
                "Failed to open local file '{}': {}",
                src.display(),
                e
            )))
        })?;

        let flags = OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE;
        let mut remote_file = tokio::time::timeout(
            ctx.timeout_duration,
            ctx.sftp.open_with_flags(dest_remote, flags),
        )
        .await
        .map_err(|_| {
            BackendError::Disconnected(format!(
                "Timed out opening remote destination '{}'",
                dest_remote
            ))
        })?
        .map_err(|e| map_sftp_err(e, dest_remote))?;

        let mut buf = vec![0u8; CHUNK_SIZE];
        let mut written = 0u64;

        let res: Result<(), BackendError> = async {
            loop {
                let n = local_file.read(&mut buf).await.map_err(|e| {
                    BackendError::Other(AppError::Filesystem(format!(
                        "Failed reading local file '{}': {}",
                        src.display(),
                        e
                    )))
                })?;
                if n == 0 {
                    break;
                }
                tokio::time::timeout(ctx.timeout_duration, remote_file.write_all(&buf[..n]))
                    .await
                    .map_err(|_| {
                        BackendError::Disconnected(format!(
                            "Timed out writing to remote file '{}'",
                            dest_remote
                        ))
                    })?
                    .map_err(|e| {
                        BackendError::Disconnected(format!(
                            "Failed writing to remote file '{}': {}",
                            dest_remote, e
                        ))
                    })?;
                written += n as u64;
            }
            tokio::time::timeout(ctx.timeout_duration, remote_file.flush())
                .await
                .map_err(|_| {
                    BackendError::Disconnected(format!(
                        "Timed out flushing remote file '{}'",
                        dest_remote
                    ))
                })?
                .map_err(|e| {
                    BackendError::Disconnected(format!(
                        "Failed flushing remote file '{}': {}",
                        dest_remote, e
                    ))
                })?;
            tokio::time::timeout(ctx.timeout_duration, remote_file.shutdown())
                .await
                .map_err(|_| {
                    BackendError::Disconnected(format!(
                        "Timed out closing remote file '{}'",
                        dest_remote
                    ))
                })?
                .map_err(|e| {
                    BackendError::Disconnected(format!(
                        "Failed closing remote file '{}': {}",
                        dest_remote, e
                    ))
                })?;
            Ok(())
        }
        .await;

        if let Err(err) = res {
            drop(remote_file);
            if remove_on_error {
                let _ = ctx.sftp.remove_file(dest_remote).await;
            }
            return Err(err);
        }

        if written != expected_size {
            if remove_on_error {
                let _ = ctx.sftp.remove_file(dest_remote).await;
            }
            return Err(BackendError::Other(AppError::Protocol(format!(
                "Upload byte mismatch for '{}': expected {} bytes, wrote {}",
                dest_remote, expected_size, written
            ))));
        }

        Ok(())
    }
}
