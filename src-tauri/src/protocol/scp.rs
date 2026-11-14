use crate::errors::AppError;
use crate::models::{
    ConnectionProfile, ConnectionSession, FileEntry, FileMetadata, FileOwnership,
    ProtocolCapabilities, ProtocolType, UnixPermissions,
};
use crate::persistence::KnownHostEntry;
use crate::protocol::sftp::{establish_ssh_transport, ConnectOutcome, SshClientHandler};
use crate::protocol::RemoteFileSystem;
use async_trait::async_trait;
use russh::client::Handle;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::sync::{watch, Mutex};
use uuid::Uuid;

/// Infallible POSIX single-quoted shell escaping to prevent command injection across SSH channels.
pub fn posix_shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Active SCP connection holding the underlying SSH transport handle and session metadata.
pub struct ActiveScpConnection {
    pub session_meta: ConnectionSession,
    pub trusted_host_key: KnownHostEntry,
    pub ssh_handle: Arc<Mutex<Handle<SshClientHandler>>>,
    pub timeout_duration: Duration,
    pub profile: ConnectionProfile,
}

impl ActiveScpConnection {
    /// Gracefully closes the SSH transport channel and connection.
    pub async fn close(self) -> Result<(), AppError> {
        let handle = self.ssh_handle.lock().await;
        let _ = handle
            .disconnect(russh::Disconnect::ByApplication, "User disconnected", "en")
            .await;
        Ok(())
    }
}

/// Establishes an SCP connection over SSH using strict host-key verification and secure credential storage.
/// Does NOT silently fall back to SFTP or vice versa.
pub async fn establish_scp_connection(
    profile: &ConnectionProfile,
    secret_override: Option<String>,
) -> Result<Result<ActiveScpConnection, ConnectOutcome>, AppError> {
    profile.validate()?;

    if profile.protocol != ProtocolType::Scp {
        return Err(AppError::Protocol(format!(
            "Protocol {} is not supported by the SCP connection adapter",
            profile.protocol.as_str()
        )));
    }

    if profile.sudo_sftp.enabled {
        return Err(AppError::Validation(
            "Sudo elevation is unsupported on SCP connections. Use SFTP for sudo-elevated sessions."
                .to_string(),
        ));
    }

    let (ssh_handle, trusted_host_key, timeout_duration) =
        match establish_ssh_transport(profile, secret_override).await? {
            Ok(tuple) => tuple,
            Err(outcome) => return Ok(Err(outcome)),
        };

    let session_meta = ConnectionSession::new(
        Some(profile.id.clone()),
        ProtocolType::Scp,
        profile.host.clone(),
        profile.port,
        profile.username.clone(),
        Some("~".to_string()),
    );

    Ok(Ok(ActiveScpConnection {
        session_meta,
        trusted_host_key,
        ssh_handle: Arc::new(Mutex::new(ssh_handle)),
        timeout_duration,
        profile: profile.clone(),
    }))
}

/// Adapter implementing `RemoteFileSystem` for the SCP backend.
/// Explicitly rejects unsupported filesystem operations without simulated shell execution.
pub struct ScpFileSystemAdapter {
    capabilities: ProtocolCapabilities,
}

impl Default for ScpFileSystemAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ScpFileSystemAdapter {
    pub fn new() -> Self {
        Self {
            capabilities: ProtocolCapabilities::for_scp(),
        }
    }
}

#[async_trait]
impl RemoteFileSystem for ScpFileSystemAdapter {
    fn capabilities(&self) -> &ProtocolCapabilities {
        &self.capabilities
    }

    async fn connect(&mut self) -> Result<(), AppError> {
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), AppError> {
        Ok(())
    }

    async fn do_list_directory(&self, _path: &str) -> Result<Vec<FileEntry>, AppError> {
        Err(AppError::Protocol(
            "Directory browsing is not supported by SCP protocol. SCP is a transfer-only protocol without interactive directory listing."
                .to_string(),
        ))
    }

    async fn do_stat(&self, _path: &str) -> Result<FileMetadata, AppError> {
        Err(AppError::Protocol(
            "Stat metadata queries are not supported by SCP protocol.".to_string(),
        ))
    }

    async fn do_create_file(&self, _path: &str) -> Result<(), AppError> {
        Err(AppError::Protocol(
            "Remote file creation is not supported by SCP protocol.".to_string(),
        ))
    }

    async fn do_create_directory(&self, _path: &str) -> Result<(), AppError> {
        Err(AppError::Protocol(
            "Remote directory creation is not supported by SCP protocol.".to_string(),
        ))
    }

    async fn do_rename(&self, _old: &str, _new: &str) -> Result<(), AppError> {
        Err(AppError::Protocol(
            "Remote file rename is not supported by SCP protocol.".to_string(),
        ))
    }

    async fn do_delete_file(&self, _path: &str) -> Result<(), AppError> {
        Err(AppError::Protocol(
            "Remote file deletion is not supported by SCP protocol.".to_string(),
        ))
    }

    async fn do_delete_directory(&self, _path: &str, _rec: bool) -> Result<(), AppError> {
        Err(AppError::Protocol(
            "Remote directory deletion is not supported by SCP protocol.".to_string(),
        ))
    }

    async fn do_set_permissions(&self, _path: &str, _p: UnixPermissions) -> Result<(), AppError> {
        Err(AppError::Protocol(
            "Remote permissions modification (chmod) is not supported by SCP protocol.".to_string(),
        ))
    }

    async fn do_set_owner_group(&self, _path: &str, _o: FileOwnership) -> Result<(), AppError> {
        Err(AppError::Protocol(
            "Remote ownership modification (chown) is not supported by SCP protocol.".to_string(),
        ))
    }
}

// ---------------------------------------------------------------------------
// SCP Wire Protocol Transfer Engine
// ---------------------------------------------------------------------------

/// Reads a 1-byte SCP response status from the remote server.
/// 0x00 is OK, 0x01 is Warning/Error, 0x02 is Fatal Error.
pub async fn read_scp_ack<R: AsyncRead + Unpin>(reader: &mut R) -> Result<(), AppError> {
    let mut status_byte = [0u8; 1];
    reader
        .read_exact(&mut status_byte)
        .await
        .map_err(|e| AppError::Protocol(format!("Failed to read SCP response: {}", e)))?;

    match status_byte[0] {
        0 => Ok(()),
        1 | 2 => {
            let msg = read_scp_line(reader).await.unwrap_or_default();
            let clean_msg = msg.trim();
            if clean_msg.contains("Permission denied") {
                Err(AppError::PermissionDenied(clean_msg.to_string()))
            } else {
                Err(AppError::Protocol(format!(
                    "SCP error from remote server: {}",
                    clean_msg
                )))
            }
        }
        other => Err(AppError::Protocol(format!(
            "Unexpected response byte 0x{:02x} from SCP server",
            other
        ))),
    }
}

/// Reads a single line (up to \n) from the SCP stream.
pub async fn read_scp_line<R: AsyncRead + Unpin>(reader: &mut R) -> Result<String, AppError> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    while buf.len() < 4096 {
        reader
            .read_exact(&mut byte)
            .await
            .map_err(|e| AppError::Protocol(format!("Failed reading SCP line: {}", e)))?;
        if byte[0] == b'\n' {
            break;
        }
        buf.push(byte[0]);
    }
    String::from_utf8(buf)
        .map_err(|e| AppError::Protocol(format!("Invalid UTF-8 in SCP line: {}", e)))
}

/// Uploads a single local file to the remote target path using the SCP sink protocol (`scp -t`).
#[allow(clippy::too_many_arguments)]
pub async fn upload_single_file_scp<F>(
    ssh_handle: &Handle<SshClientHandler>,
    local_path: &Path,
    remote_target_path: &str,
    file_size: u64,
    mode: u32,
    mtime_secs: Option<u64>,
    preserve_timestamps: bool,
    cancel_rx: &mut watch::Receiver<bool>,
    mut on_progress: F,
    timeout_duration: Duration,
) -> Result<(), AppError>
where
    F: FnMut(u64),
{
    let channel = tokio::time::timeout(timeout_duration, ssh_handle.channel_open_session())
        .await
        .map_err(|_| {
            AppError::Protocol("Timed out opening SSH channel for SCP upload".to_string())
        })?
        .map_err(|e| AppError::Protocol(format!("Failed to open SSH channel: {}", e)))?;

    let scp_cmd = if preserve_timestamps {
        format!("scp -p -t {}", posix_shell_quote(remote_target_path))
    } else {
        format!("scp -t {}", posix_shell_quote(remote_target_path))
    };

    tokio::time::timeout(timeout_duration, channel.exec(true, scp_cmd.as_str()))
        .await
        .map_err(|_| AppError::Protocol("Timed out starting SCP sink process".to_string()))?
        .map_err(|e| AppError::Protocol(format!("Failed executing SCP sink: {}", e)))?;

    let mut stream = channel.into_stream();

    // 1. Initial ACK from server
    read_scp_ack(&mut stream).await?;

    // 2. Timestamp header if preserving timestamps
    if preserve_timestamps {
        if let Some(mtime) = mtime_secs {
            let t_hdr = format!("T{} 0 {} 0\n", mtime, mtime);
            stream.write_all(t_hdr.as_bytes()).await.map_err(|e| {
                AppError::Protocol(format!("Failed writing SCP timestamp header: {}", e))
            })?;
            stream.flush().await.map_err(|e| {
                AppError::Protocol(format!("Failed flushing SCP timestamp header: {}", e))
            })?;
            read_scp_ack(&mut stream).await?;
        }
    }

    // 3. File header: C<mode> <size> <filename>\n
    let file_name = local_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    let octal_mode = format!("{:04o}", mode & 0o7777);
    let c_hdr = format!("C{} {} {}\n", octal_mode, file_size, file_name);
    stream
        .write_all(c_hdr.as_bytes())
        .await
        .map_err(|e| AppError::Protocol(format!("Failed writing SCP file header: {}", e)))?;
    stream
        .flush()
        .await
        .map_err(|e| AppError::Protocol(format!("Failed flushing SCP file header: {}", e)))?;
    read_scp_ack(&mut stream).await?;

    // 4. Stream file content
    let mut local_file = tokio::fs::File::open(local_path).await.map_err(|e| {
        AppError::Filesystem(format!("Failed opening local file for SCP upload: {}", e))
    })?;

    let mut buf = vec![0u8; 64 * 1024];
    let mut remaining = file_size;

    while remaining > 0 {
        if *cancel_rx.borrow() {
            return Err(AppError::Protocol(
                "Transfer was cancelled by user".to_string(),
            ));
        }

        let to_read = (buf.len() as u64).min(remaining) as usize;
        let n = local_file.read(&mut buf[..to_read]).await.map_err(|e| {
            AppError::Filesystem(format!("Error reading local file during SCP upload: {}", e))
        })?;
        if n == 0 {
            return Err(AppError::Filesystem(
                "Unexpected EOF reading local file during SCP upload".to_string(),
            ));
        }

        stream
            .write_all(&buf[..n])
            .await
            .map_err(|e| AppError::Protocol(format!("Error writing to SCP stream: {}", e)))?;
        stream
            .flush()
            .await
            .map_err(|e| AppError::Protocol(format!("Error flushing SCP stream: {}", e)))?;

        remaining -= n as u64;
        on_progress(n as u64);
    }

    // 5. Send completion byte 0x00 and read server final ACK
    stream
        .write_u8(0)
        .await
        .map_err(|e| AppError::Protocol(format!("Failed writing SCP EOF byte: {}", e)))?;
    stream
        .flush()
        .await
        .map_err(|e| AppError::Protocol(format!("Failed flushing SCP EOF byte: {}", e)))?;
    read_scp_ack(&mut stream).await?;

    Ok(())
}

/// Downloads a single remote file to the local destination path using the SCP source protocol (`scp -f`).
/// Uses an atomic temp file (`.linuxscp-tmp-<uuid>`) before final rename.
#[allow(clippy::too_many_arguments)]
pub async fn download_single_file_scp<F>(
    ssh_handle: &Handle<SshClientHandler>,
    remote_source_path: &str,
    local_dest_path: &Path,
    preserve_timestamps: bool,
    cancel_rx: &mut watch::Receiver<bool>,
    mut on_progress: F,
    timeout_duration: Duration,
) -> Result<(), AppError>
where
    F: FnMut(u64),
{
    let channel = tokio::time::timeout(timeout_duration, ssh_handle.channel_open_session())
        .await
        .map_err(|_| {
            AppError::Protocol("Timed out opening SSH channel for SCP download".to_string())
        })?
        .map_err(|e| AppError::Protocol(format!("Failed to open SSH channel: {}", e)))?;

    let scp_cmd = if preserve_timestamps {
        format!("scp -p -f {}", posix_shell_quote(remote_source_path))
    } else {
        format!("scp -f {}", posix_shell_quote(remote_source_path))
    };

    tokio::time::timeout(timeout_duration, channel.exec(true, scp_cmd.as_str()))
        .await
        .map_err(|_| AppError::Protocol("Timed out starting SCP source process".to_string()))?
        .map_err(|e| AppError::Protocol(format!("Failed executing SCP source: {}", e)))?;

    let mut stream = channel.into_stream();

    // 1. Send initial 0x00 byte to trigger remote transmission
    stream.write_u8(0).await.map_err(|e| {
        AppError::Protocol(format!("Failed sending initial SCP trigger byte: {}", e))
    })?;
    stream.flush().await.map_err(|e| {
        AppError::Protocol(format!("Failed flushing initial SCP trigger byte: {}", e))
    })?;

    let mut mtime_to_apply: Option<u64> = None;
    let temp_dest_path = {
        let parent = local_dest_path.parent().unwrap_or_else(|| Path::new("."));
        let filename = local_dest_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file");
        parent.join(format!(".linuxscp-tmp-{}-{}", Uuid::new_v4(), filename))
    };

    loop {
        let mut first_byte = [0u8; 1];
        stream
            .read_exact(&mut first_byte)
            .await
            .map_err(|e| AppError::Protocol(format!("Failed reading SCP control byte: {}", e)))?;

        match first_byte[0] {
            0 => {
                // No-op synchronization byte
                continue;
            }
            1 | 2 => {
                let err_msg = read_scp_line(&mut stream).await.unwrap_or_default();
                let clean_msg = err_msg.trim();
                let _ = tokio::fs::remove_file(&temp_dest_path).await;
                if clean_msg.contains("No such file") {
                    return Err(AppError::Filesystem(format!(
                        "Remote file not found: {}",
                        clean_msg
                    )));
                } else if clean_msg.contains("Permission denied") {
                    return Err(AppError::PermissionDenied(clean_msg.to_string()));
                }
                return Err(AppError::Protocol(format!(
                    "SCP download error: {}",
                    clean_msg
                )));
            }
            b'T' => {
                // Timestamp line: T<mtime> 0 <atime> 0
                let line = read_scp_line(&mut stream).await?;
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(mtime_str) = parts.first() {
                    if let Ok(m) = mtime_str.parse::<u64>() {
                        mtime_to_apply = Some(m);
                    }
                }
                // ACK timestamp line
                stream.write_u8(0).await.map_err(|e| {
                    AppError::Protocol(format!("Failed acknowledging SCP timestamp: {}", e))
                })?;
                stream.flush().await.map_err(|e| {
                    AppError::Protocol(format!("Failed flushing SCP timestamp ACK: {}", e))
                })?;
            }
            b'C' => {
                // File header: C<mode> <length> <filename>
                let line = read_scp_line(&mut stream).await?;
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 2 {
                    let _ = tokio::fs::remove_file(&temp_dest_path).await;
                    return Err(AppError::Protocol(format!(
                        "Malformed SCP file header line: {}",
                        line
                    )));
                }

                let mode = u32::from_str_radix(parts[0], 8).unwrap_or(0o644);
                let length: u64 = parts[1].parse().map_err(|e| {
                    AppError::Protocol(format!("Invalid file length in SCP header: {}", e))
                })?;

                // Acknowledge file header to start streaming content
                stream.write_u8(0).await.map_err(|e| {
                    AppError::Protocol(format!("Failed acknowledging SCP file header: {}", e))
                })?;
                stream.flush().await.map_err(|e| {
                    AppError::Protocol(format!("Failed flushing SCP file header ACK: {}", e))
                })?;

                // Stream into atomic temp file
                let mut temp_file =
                    tokio::fs::File::create(&temp_dest_path)
                        .await
                        .map_err(|e| {
                            AppError::Filesystem(format!(
                                "Failed creating atomic temporary file: {}",
                                e
                            ))
                        })?;

                let mut remaining = length;
                let mut buf = vec![0u8; 64 * 1024];

                while remaining > 0 {
                    if *cancel_rx.borrow() {
                        let _ = tokio::fs::remove_file(&temp_dest_path).await;
                        return Err(AppError::Protocol(
                            "Transfer was cancelled by user".to_string(),
                        ));
                    }

                    let to_read = (buf.len() as u64).min(remaining) as usize;
                    let n = stream.read(&mut buf[..to_read]).await.map_err(|e| {
                        let _ = std::fs::remove_file(&temp_dest_path);
                        AppError::Protocol(format!("Error reading SCP stream: {}", e))
                    })?;

                    if n == 0 {
                        let _ = tokio::fs::remove_file(&temp_dest_path).await;
                        return Err(AppError::Protocol(
                            "Unexpected EOF reading SCP stream from remote server".to_string(),
                        ));
                    }

                    temp_file.write_all(&buf[..n]).await.map_err(|e| {
                        let _ = std::fs::remove_file(&temp_dest_path);
                        AppError::Filesystem(format!("Failed writing local temp file: {}", e))
                    })?;

                    remaining -= n as u64;
                    on_progress(n as u64);
                }

                temp_file.flush().await.map_err(|e| {
                    let _ = std::fs::remove_file(&temp_dest_path);
                    AppError::Filesystem(format!("Failed flushing local temp file: {}", e))
                })?;
                drop(temp_file);

                // Read trailing completion byte 0x00 from server
                read_scp_ack(&mut stream).await?;

                // Acknowledge completion back to server
                stream.write_u8(0).await.map_err(|e| {
                    AppError::Protocol(format!("Failed writing final SCP ACK byte: {}", e))
                })?;
                stream.flush().await.map_err(|e| {
                    AppError::Protocol(format!("Failed flushing final SCP ACK byte: {}", e))
                })?;

                // Atomic rename to final destination
                tokio::fs::rename(&temp_dest_path, local_dest_path)
                    .await
                    .map_err(|e| {
                        let _ = std::fs::remove_file(&temp_dest_path);
                        AppError::Filesystem(format!(
                            "Failed renaming atomic temp file to final destination: {}",
                            e
                        ))
                    })?;

                // Set local permissions and mtime if configured
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = tokio::fs::set_permissions(
                        local_dest_path,
                        std::fs::Permissions::from_mode(mode),
                    )
                    .await;

                    if let Some(mtime) = mtime_to_apply {
                        use std::os::unix::ffi::OsStrExt;
                        if let Ok(c_path) =
                            std::ffi::CString::new(local_dest_path.as_os_str().as_bytes())
                        {
                            let times = [
                                libc::timeval {
                                    tv_sec: mtime as libc::time_t,
                                    tv_usec: 0,
                                },
                                libc::timeval {
                                    tv_sec: mtime as libc::time_t,
                                    tv_usec: 0,
                                },
                            ];
                            unsafe {
                                libc::utimes(c_path.as_ptr(), times.as_ptr());
                            }
                        }
                    }
                }

                return Ok(());
            }
            other => {
                let _ = tokio::fs::remove_file(&temp_dest_path).await;
                return Err(AppError::Protocol(format!(
                    "Unexpected command byte 0x{:02x} ('{}') in SCP stream",
                    other, other as char
                )));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Unit and Integration Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AuthMethod, ConnectionProfile, ProtocolCapabilities, ProtocolType};
    use crate::persistence::{Database, KnownHostsRepository};

    #[test]
    fn test_scp_capabilities_matrix_matches_honest_boundaries() {
        let caps = ProtocolCapabilities::for_scp();
        assert_eq!(caps.protocol, ProtocolType::Scp);
        assert!(!caps.can_list_directory);
        assert!(!caps.can_stat_metadata);
        assert!(caps.can_upload);
        assert!(caps.can_download);
        assert!(!caps.can_create_file);
        assert!(!caps.can_create_directory);
        assert!(!caps.can_rename);
        assert!(!caps.can_delete_file);
        assert!(!caps.can_delete_directory);
        assert!(!caps.can_set_permissions);
        assert!(!caps.can_set_owner_group);
        assert!(!caps.can_elevate_sudo);
        assert!(caps.can_preserve_timestamps);
        assert!(!caps.can_resume_transfer);
        assert!(!caps.can_symlink);
    }

    #[tokio::test]
    async fn test_scp_adapter_enforces_all_capability_rejections() {
        let adapter = ScpFileSystemAdapter::new();

        // 1. Directory listing is strictly unsupported
        let list_err = adapter.list_directory("/tmp").await.unwrap_err();
        assert!(matches!(list_err, AppError::Protocol(ref msg) if msg.contains("list_directory")));

        // 2. Stat metadata is strictly unsupported
        let stat_err = adapter.stat("/tmp/file.txt").await.unwrap_err();
        assert!(matches!(stat_err, AppError::Protocol(ref msg) if msg.contains("stat_metadata")));

        // 3. File creation is strictly unsupported
        let create_err = adapter.create_file("/tmp/new.txt").await.unwrap_err();
        assert!(matches!(create_err, AppError::Protocol(ref msg) if msg.contains("create_file")));

        // 4. Directory creation is strictly unsupported
        let mkdir_err = adapter.create_directory("/tmp/new_dir").await.unwrap_err();
        assert!(
            matches!(mkdir_err, AppError::Protocol(ref msg) if msg.contains("create_directory"))
        );

        // 5. Rename is strictly unsupported
        let rename_err = adapter.rename("/tmp/a", "/tmp/b").await.unwrap_err();
        assert!(matches!(rename_err, AppError::Protocol(ref msg) if msg.contains("rename")));

        // 6. Delete file is strictly unsupported
        let del_err = adapter.delete_file("/tmp/del.txt").await.unwrap_err();
        assert!(matches!(del_err, AppError::Protocol(ref msg) if msg.contains("delete_file")));

        // 7. Delete directory is strictly unsupported
        let rmdir_err = adapter
            .delete_directory("/tmp/del_dir", false)
            .await
            .unwrap_err();
        assert!(
            matches!(rmdir_err, AppError::Protocol(ref msg) if msg.contains("delete_directory"))
        );

        // 8. Chmod is strictly unsupported
        let chmod_err = adapter
            .set_permissions("/tmp/file", UnixPermissions::from_mode(0o755))
            .await
            .unwrap_err();
        assert!(
            matches!(chmod_err, AppError::Protocol(ref msg) if msg.contains("set_permissions"))
        );

        // 9. Chown is strictly unsupported
        let chown_err = adapter
            .set_owner_group(
                "/tmp/file",
                FileOwnership {
                    uid: Some(1000),
                    gid: Some(1000),
                    user_name: None,
                    group_name: None,
                },
            )
            .await
            .unwrap_err();
        assert!(
            matches!(chown_err, AppError::Protocol(ref msg) if msg.contains("set_owner_group"))
        );
    }

    #[tokio::test]
    async fn test_scp_rejects_non_scp_protocol_and_sudo_elevation() {
        // Rejects non-SCP protocol
        let sftp_profile = ConnectionProfile::new(
            "Test SFTP".to_string(),
            ProtocolType::Sftp,
            "127.0.0.1".to_string(),
            "user".to_string(),
            AuthMethod::Password,
        );
        let res = establish_scp_connection(&sftp_profile, Some("pass".to_string())).await;
        assert!(
            matches!(res, Err(AppError::Protocol(ref msg)) if msg.contains("not supported by the SCP connection adapter"))
        );

        // Rejects sudo elevation on SCP
        let mut scp_profile = ConnectionProfile::new(
            "Test SCP Sudo".to_string(),
            ProtocolType::Scp,
            "127.0.0.1".to_string(),
            "user".to_string(),
            AuthMethod::Password,
        );
        scp_profile.sudo_sftp.enabled = true;
        let res_sudo = establish_scp_connection(&scp_profile, Some("pass".to_string())).await;
        assert!(
            matches!(res_sudo, Err(AppError::Validation(ref msg)) if msg.contains("does not support sudo"))
        );
    }

    #[tokio::test]
    async fn test_live_scp_connection_and_transfers_against_test_vm() {
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

        let db = Database::open_default().expect("Should open DB");
        let kh_repo = KnownHostsRepository::new(&db);

        let profile = ConnectionProfile::new(
            "Ubuntu Test VM SCP".to_string(),
            ProtocolType::Scp,
            test_host.to_string(),
            "ubuntu".to_string(),
            AuthMethod::PublicKey {
                private_key_path: "~/.ssh/id_rsa".to_string(),
                requires_passphrase: false,
            },
        );

        // Ensure host key is trusted
        let conn_res = establish_scp_connection(&profile, None).await.unwrap();
        let conn = match conn_res {
            Ok(c) => c,
            Err(ConnectOutcome::HostKeyUnknown {
                host,
                port,
                algorithm,
                fingerprint_sha256,
                public_key_openssh,
            }) => {
                kh_repo
                    .trust_unknown_host_key(
                        &host,
                        port,
                        &algorithm,
                        &fingerprint_sha256,
                        &public_key_openssh,
                    )
                    .unwrap();
                match establish_scp_connection(&profile, None).await.unwrap() {
                    Ok(c) => c,
                    Err(other) => panic!("Expected Connected, got {:?}", other),
                }
            }
            Err(other) => panic!("Expected Connected or HostKeyUnknown, got {:?}", other),
        };

        assert_eq!(conn.session_meta.protocol, ProtocolType::Scp);
        assert!(!conn.session_meta.capabilities.can_list_directory);
        assert!(conn.session_meta.capabilities.can_upload);
        assert!(conn.session_meta.capabilities.can_download);

        // 1. Test SCP File Upload
        let temp_dir = std::env::temp_dir().join(format!("linuxscp_scp_test_{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();
        let local_upload_file = temp_dir.join("test_upload_file.txt");
        let payload = b"Hello from LinuxSCP SCP Engine!\n";
        tokio::fs::write(&local_upload_file, payload).await.unwrap();

        let (_cancel_tx, mut cancel_rx) = watch::channel(false);
        let mut uploaded_bytes = 0u64;

        let handle_guard = conn.ssh_handle.lock().await;
        let upload_res = upload_single_file_scp(
            &handle_guard,
            &local_upload_file,
            "/tmp",
            payload.len() as u64,
            0o644,
            Some(1700000000),
            true,
            &mut cancel_rx,
            |bytes| uploaded_bytes += bytes,
            Duration::from_secs(10),
        )
        .await;

        assert!(upload_res.is_ok(), "SCP upload failed: {:?}", upload_res);
        assert_eq!(uploaded_bytes, payload.len() as u64);

        // 2. Test SCP File Download
        let local_download_file = temp_dir.join("test_download_file.txt");
        let mut downloaded_bytes = 0u64;

        let download_res = download_single_file_scp(
            &handle_guard,
            "/tmp/test_upload_file.txt",
            &local_download_file,
            true,
            &mut cancel_rx,
            |bytes| downloaded_bytes += bytes,
            Duration::from_secs(10),
        )
        .await;

        assert!(
            download_res.is_ok(),
            "SCP download failed: {:?}",
            download_res
        );
        assert_eq!(downloaded_bytes, payload.len() as u64);

        let downloaded_content = tokio::fs::read(&local_download_file).await.unwrap();
        assert_eq!(downloaded_content, payload);

        // Clean up remote and local
        drop(handle_guard);
        let _ = conn.close().await;
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }
}
