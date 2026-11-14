use crate::errors::AppError;
use crate::models::{
    AuthMethod, ConnectionProfile, ConnectionSession, ElevationState, FileEntry, FileMetadata,
    FileOwnership, FileType, ProtocolCapabilities, ProtocolType, SessionStatus, UnixPermissions,
};
use crate::persistence::{
    Database, HostKeyDecision, KeyringService, KnownHostEntry, KnownHostsRepository,
    RemoteStateRepository,
};
use crate::protocol::RemoteFileSystem;
use async_trait::async_trait;
use russh::client::{self, Handle, Handler};
use russh::keys::{HashAlg, PrivateKeyWithHashAlg, PublicKeyOrCertificate};
use russh_sftp::client::error::Error as SftpClientError;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::{FileAttributes, OpenFlags, StatusCode};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::AsyncWriteExt;

/// Directory listing returned by remote SFTP browsing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteDirectoryListing {
    pub current_path: String,
    pub parent_path: Option<String>,
    pub home_path: Option<String>,
    pub entries: Vec<FileEntry>,
}

/// Outcome of an SFTP connection attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome")]
pub enum ConnectOutcome {
    /// Connection and SFTP subsystem initialization succeeded.
    Connected {
        session: Box<ConnectionSession>,
        trusted_host_key: KnownHostEntry,
    },
    /// Connection paused/blocked because the host key is unknown and requires user approval.
    HostKeyUnknown {
        host: String,
        port: u16,
        algorithm: String,
        fingerprint_sha256: String,
        public_key_openssh: String,
    },
    /// Connection strictly blocked because the presented host key does not match the stored key.
    HostKeyChanged {
        host: String,
        port: u16,
        expected_algorithm: String,
        expected_fingerprint: String,
        presented_algorithm: String,
        presented_fingerprint: String,
    },
}

/// Internal russh client handler that enforces strict host-key verification.
pub struct SshClientHandler {
    host: String,
    port: u16,
    decision: Arc<Mutex<Option<HostKeyDecision>>>,
}

impl SshClientHandler {
    pub fn new(host: String, port: u16, decision: Arc<Mutex<Option<HostKeyDecision>>>) -> Self {
        Self {
            host,
            port,
            decision,
        }
    }
}

impl Handler for SshClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let key = server_public_key.public_key();
        let algorithm = key.algorithm().to_string();
        let fingerprint_sha256 = key
            .fingerprint(russh::keys::ssh_key::HashAlg::Sha256)
            .to_string();
        let public_key_openssh = key.to_openssh().unwrap_or_default();

        let verification = match Database::open_default() {
            Ok(db) => {
                let repo = KnownHostsRepository::new(&db);
                repo.verify_host_key(
                    &self.host,
                    self.port,
                    &algorithm,
                    &fingerprint_sha256,
                    &public_key_openssh,
                )
            }
            Err(e) => {
                tracing::error!(
                    "Failed to open known_hosts database during handshake: {}",
                    e
                );
                return Ok(false);
            }
        };

        match verification {
            Ok(HostKeyDecision::Trusted { entry }) => {
                if let Ok(mut guard) = self.decision.lock() {
                    *guard = Some(HostKeyDecision::Trusted { entry });
                }
                Ok(true)
            }
            Ok(other_decision) => {
                if let Ok(mut guard) = self.decision.lock() {
                    *guard = Some(other_decision);
                }
                // Reject handshake before any user credentials are sent
                Ok(false)
            }
            Err(e) => {
                tracing::error!("Host-key verification query error: {}", e);
                Ok(false)
            }
        }
    }
}

/// Live SFTP session holding the underlying SSH handle and initialized SFTP client.
pub struct ActiveSftpConnection {
    pub session_meta: ConnectionSession,
    pub profile: ConnectionProfile,
    pub trusted_host_key: KnownHostEntry,
    pub ssh_handle: Handle<SshClientHandler>,
    pub sftp_session: Arc<SftpSession>,
}

impl ActiveSftpConnection {
    /// Gracefully closes the SFTP channel and SSH transport.
    pub async fn close(self) -> Result<(), AppError> {
        let _ = self.sftp_session.close().await;
        let _ = self
            .ssh_handle
            .disconnect(russh::Disconnect::ByApplication, "User disconnected", "en")
            .await;
        Ok(())
    }
}

/// Establishes the underlying SSH transport connection, verifies the host key,
/// and completes authentication. Both SFTP and SCP backends reuse this function
/// so host key verification and credential handling are 100% consistent.
pub async fn establish_ssh_transport(
    profile: &ConnectionProfile,
    secret_override: Option<String>,
) -> Result<Result<(Handle<SshClientHandler>, KnownHostEntry, Duration), ConnectOutcome>, AppError>
{
    let timeout_secs = u64::from(profile.connection_settings.timeout_seconds.max(5));
    let timeout_duration = Duration::from_secs(timeout_secs);

    let mut ssh_config = client::Config {
        inactivity_timeout: Some(timeout_duration),
        ..Default::default()
    };
    if let Some(keepalive) = profile.connection_settings.keepalive_interval_seconds {
        if keepalive > 0 {
            ssh_config.keepalive_interval = Some(Duration::from_secs(u64::from(keepalive)));
        }
    }

    let config = Arc::new(ssh_config);
    let decision_holder: Arc<Mutex<Option<HostKeyDecision>>> = Arc::new(Mutex::new(None));
    let handler = SshClientHandler::new(
        profile.host.trim().to_string(),
        profile.port,
        Arc::clone(&decision_holder),
    );

    let addr = (profile.host.trim(), profile.port);

    // 1. Connect TCP + SSH Handshake + Host-Key Check
    let connect_res =
        tokio::time::timeout(timeout_duration, client::connect(config, addr, handler))
            .await
            .map_err(|_| {
                AppError::Protocol(format!(
                    "Connection to {}:{} timed out after {} seconds",
                    profile.host, profile.port, timeout_secs
                ))
            })?;

    let mut ssh_handle = match connect_res {
        Ok(handle) => handle,
        Err(ssh_err) => {
            // Check if the handshake was halted due to an Unknown or Changed host key
            if let Ok(mut guard) = decision_holder.lock() {
                if let Some(decision) = guard.take() {
                    match decision {
                        HostKeyDecision::Unknown {
                            host,
                            port,
                            algorithm,
                            fingerprint_sha256,
                            public_key_openssh,
                        } => {
                            return Ok(Err(ConnectOutcome::HostKeyUnknown {
                                host,
                                port,
                                algorithm,
                                fingerprint_sha256,
                                public_key_openssh,
                            }));
                        }
                        HostKeyDecision::Changed {
                            host,
                            port,
                            expected_algorithm,
                            expected_fingerprint,
                            presented_algorithm,
                            presented_fingerprint,
                        } => {
                            return Ok(Err(ConnectOutcome::HostKeyChanged {
                                host,
                                port,
                                expected_algorithm,
                                expected_fingerprint,
                                presented_algorithm,
                                presented_fingerprint,
                            }));
                        }
                        HostKeyDecision::Trusted { .. } => {}
                    }
                }
            }

            return Err(AppError::Protocol(format!(
                "SSH transport connection to {}:{} failed: {}",
                profile.host, profile.port, ssh_err
            )));
        }
    };

    let trusted_host_key = match decision_holder.lock().ok().and_then(|mut g| g.take()) {
        Some(HostKeyDecision::Trusted { entry }) => entry,
        _ => {
            return Err(AppError::Security(
                "SSH connection proceeded without a verified host key decision".to_string(),
            ));
        }
    };

    // 2. Authenticate
    authenticate_ssh_session(&mut ssh_handle, profile, secret_override, timeout_duration).await?;

    Ok(Ok((ssh_handle, trusted_host_key, timeout_duration)))
}

/// Establishes an SSH connection, verifies the host key, authenticates, and initializes the SFTP subsystem.
pub async fn establish_sftp_connection(
    profile: &ConnectionProfile,
    secret_override: Option<String>,
) -> Result<Result<ActiveSftpConnection, ConnectOutcome>, AppError> {
    profile.validate()?;

    if profile.protocol != ProtocolType::Sftp {
        return Err(AppError::Protocol(format!(
            "Protocol {} is not supported by the SFTP connection adapter",
            profile.protocol.as_str()
        )));
    }

    let (ssh_handle, trusted_host_key, timeout_duration) =
        match establish_ssh_transport(profile, secret_override).await? {
            Ok(tuple) => tuple,
            Err(outcome) => return Ok(Err(outcome)),
        };

    // 3. Open SFTP Subsystem Channel (or elevated sudo-SFTP channel)
    let channel = tokio::time::timeout(timeout_duration, ssh_handle.channel_open_session())
        .await
        .map_err(|_| AppError::Protocol("Timed out opening SSH session channel".to_string()))?
        .map_err(|e| AppError::Protocol(format!("Failed to open SSH session channel: {}", e)))?;

    let elevation_state = if profile.sudo_sftp.enabled {
        let cmd = profile.sudo_sftp.effective_command();
        tokio::time::timeout(timeout_duration, channel.exec(true, cmd.as_str()))
            .await
            .map_err(|_| AppError::Protocol("Timed out executing sudo-SFTP command".to_string()))?
            .map_err(|e| {
                AppError::Protocol(format!(
                    "Failed to execute sudo-SFTP command '{}': {}",
                    cmd, e
                ))
            })?;
        ElevationState::ElevatedSudo {
            effective_user: "root".to_string(),
            executed_command: cmd,
        }
    } else {
        tokio::time::timeout(timeout_duration, channel.request_subsystem(true, "sftp"))
            .await
            .map_err(|_| AppError::Protocol("Timed out requesting SFTP subsystem".to_string()))?
            .map_err(|e| AppError::Protocol(format!("Failed to start SFTP subsystem: {}", e)))?;
        ElevationState::None
    };

    let sftp_session = tokio::time::timeout(timeout_duration, SftpSession::new(channel.into_stream()))
        .await
        .map_err(|_| {
            if profile.sudo_sftp.enabled {
                let bin = profile.sudo_sftp.effective_binary_path();
                AppError::Protocol(format!(
                    "Timed out waiting for elevated SFTP protocol handshake. When sudo-SFTP is enabled, sudo must not prompt for a password. Please ensure a narrow non-interactive sudoers rule is configured on the server, e.g.: '{} ALL=(root) NOPASSWD: {}'",
                    profile.username, bin
                ))
            } else {
                AppError::Protocol("Timed out initializing SFTP protocol stream".to_string())
            }
        })?
        .map_err(|e| {
            if profile.sudo_sftp.enabled {
                let bin = profile.sudo_sftp.effective_binary_path();
                AppError::Protocol(format!(
                    "Elevated SFTP protocol handshake failed: {}. Sudo elevation requires the server binary '{}' to be executable via non-interactive sudo. If sudo prompted for an interactive password or emitted diagnostic messages on stdout, the binary SFTP handshake was rejected. Please verify the binary path and ensure your server-side sudoers configuration has: '{} ALL=(root) NOPASSWD: {}'",
                    e, bin, profile.username, bin
                ))
            } else {
                AppError::Protocol(format!("SFTP subsystem initialization failed: {}", e))
            }
        })?;

    // 4. Verify SFTP subsystem readiness by resolving the home directory and initial working directory
    let home_path = tokio::time::timeout(timeout_duration, sftp_session.canonicalize("."))
        .await
        .map_err(|_| AppError::Protocol("Timed out verifying SFTP working directory".to_string()))?
        .map_err(|e| {
            AppError::Protocol(format!(
                "SFTP subsystem failed working directory check: {}",
                e
            ))
        })?;

    let last_remote_dir = Database::open_default()
        .ok()
        .and_then(|db| {
            RemoteStateRepository::new(&db)
                .get_last_remote_directory(&profile.id)
                .ok()
        })
        .flatten();

    let mut candidates: Vec<String> = Vec::new();
    if let Some(last) = last_remote_dir.filter(|s| !s.trim().is_empty()) {
        candidates.push(last);
    }
    if let Some(init) = profile
        .initial_remote_directory
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        candidates.push(init.to_string());
    }

    let mut resolved_path = home_path.clone();
    for candidate in candidates {
        let resolved_candidate =
            resolve_remote_input_path(&candidate, &home_path, Some(&home_path));
        if let Ok(Ok(canon)) = tokio::time::timeout(
            timeout_duration,
            sftp_session.canonicalize(resolved_candidate.as_str()),
        )
        .await
        {
            if let Ok(Ok(meta)) =
                tokio::time::timeout(timeout_duration, sftp_session.metadata(canon.as_str())).await
            {
                if meta.is_dir() {
                    resolved_path = canon;
                    break;
                }
            }
        }
    }

    let mut session_meta = ConnectionSession::new(
        Some(profile.id.clone()),
        profile.protocol,
        profile.host.clone(),
        profile.port,
        profile.username.clone(),
        Some(resolved_path),
    );
    session_meta.home_remote_path = Some(home_path);
    session_meta.status = SessionStatus::Connected;
    session_meta.elevation = elevation_state;

    Ok(Ok(ActiveSftpConnection {
        session_meta,
        profile: profile.clone(),
        trusted_host_key,
        ssh_handle,
        sftp_session: Arc::new(sftp_session),
    }))
}

/// Performs SSH authentication according to the profile's configured AuthMethod.
/// Secrets are never logged or included in error strings.
async fn authenticate_ssh_session(
    ssh_handle: &mut Handle<SshClientHandler>,
    profile: &ConnectionProfile,
    secret_override: Option<String>,
    timeout_duration: Duration,
) -> Result<(), AppError> {
    let username = profile.username.trim();

    match &profile.auth_method {
        AuthMethod::Password | AuthMethod::Interactive => {
            let password = match secret_override.filter(|s| !s.is_empty()) {
                Some(sec) => sec,
                None => KeyringService::get_secret(&profile.id)?.ok_or_else(|| {
                    AppError::Authentication(
                        "Password is required. Please store the password in Site Manager or provide it on connect.".to_string(),
                    )
                })?,
            };

            let auth_res = tokio::time::timeout(
                timeout_duration,
                ssh_handle.authenticate_password(username, password),
            )
            .await
            .map_err(|_| AppError::Authentication("Password authentication timed out".to_string()))?
            .map_err(|e| {
                AppError::Authentication(format!("Password authentication failed: {}", e))
            })?;

            if !auth_res.success() {
                return Err(AppError::Authentication(
                    "Server rejected password authentication for user".to_string(),
                ));
            }
            Ok(())
        }

        AuthMethod::PublicKey {
            private_key_path,
            requires_passphrase,
        } => {
            let expanded_path = shellexpand::tilde(private_key_path.trim()).to_string();
            let path = Path::new(&expanded_path);
            if !path.exists() {
                return Err(AppError::Authentication(format!(
                    "Private key file not found at '{}'",
                    expanded_path
                )));
            }

            let passphrase = if *requires_passphrase {
                match secret_override.filter(|s| !s.is_empty()) {
                    Some(sec) => Some(sec),
                    None => KeyringService::get_secret(&profile.id)?,
                }
            } else {
                None
            };

            let key_pair =
                russh::keys::load_secret_key(path, passphrase.as_deref()).map_err(|e| {
                    AppError::Authentication(format!(
                        "Failed to load private key '{}' (check file format or passphrase): {}",
                        expanded_path, e
                    ))
                })?;

            let hash_alg = if key_pair.algorithm().is_rsa() {
                Some(HashAlg::Sha512)
            } else {
                None
            };

            let priv_key = PrivateKeyWithHashAlg::new(Arc::new(key_pair), hash_alg);

            let auth_res = tokio::time::timeout(
                timeout_duration,
                ssh_handle.authenticate_publickey(username, priv_key),
            )
            .await
            .map_err(|_| {
                AppError::Authentication("Public key authentication timed out".to_string())
            })?
            .map_err(|e| {
                AppError::Authentication(format!("Public key authentication failed: {}", e))
            })?;

            if !auth_res.success() {
                return Err(AppError::Authentication(
                    "Server rejected public key authentication".to_string(),
                ));
            }
            Ok(())
        }

        AuthMethod::Agent => {
            let mut agent = russh::keys::agent::client::AgentClient::connect_env()
                .await
                .map_err(|e| {
                    AppError::Authentication(format!(
                        "Failed to connect to SSH agent via SSH_AUTH_SOCK: {}",
                        e
                    ))
                })?;

            let identities = agent.request_identities().await.map_err(|e| {
                AppError::Authentication(format!("Failed to list identities from SSH agent: {}", e))
            })?;

            if identities.is_empty() {
                return Err(AppError::Authentication(
                    "SSH agent is running but has no loaded identities".to_string(),
                ));
            }

            for identity in identities {
                let pub_key = identity.public_key().into_owned();
                let hash_alg = if pub_key.algorithm().is_rsa() {
                    Some(HashAlg::Sha512)
                } else {
                    None
                };

                if let Ok(Ok(auth_res)) = tokio::time::timeout(
                    timeout_duration,
                    ssh_handle.authenticate_publickey_with(username, pub_key, hash_alg, &mut agent),
                )
                .await
                {
                    if auth_res.success() {
                        return Ok(());
                    }
                }
            }

            Err(AppError::Authentication(
                "Server rejected all identities offered by the SSH agent".to_string(),
            ))
        }
    }
}

/// Normalizes a POSIX remote path: collapses duplicate slashes, resolves `.` and `..`
/// (clamped at `/`), and strips trailing slashes except for `/`.
/// Preserves spaces, Unicode characters, and valid special characters.
pub fn normalize_posix_path(raw_path: &str) -> String {
    let mut stack: Vec<&str> = Vec::new();
    for segment in raw_path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                stack.pop();
            }
            other => stack.push(other),
        }
    }
    if stack.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", stack.join("/"))
    }
}

/// Joins a base POSIX directory path and a child entry name without duplicate slashes.
pub fn join_posix_path(base: &str, name: &str) -> String {
    let trimmed_base = base.trim_end_matches('/');
    if trimmed_base.is_empty() {
        format!("/{}", name.trim_start_matches('/'))
    } else {
        format!("{}/{}", trimmed_base, name.trim_start_matches('/'))
    }
}

/// Computes the parent POSIX directory path, or `None` when already at `/`.
pub fn parent_posix_path(path: &str) -> Option<String> {
    let norm = normalize_posix_path(path);
    if norm == "/" {
        None
    } else {
        match norm.rfind('/') {
            Some(0) => Some("/".to_string()),
            Some(idx) => Some(norm[..idx].to_string()),
            None => Some("/".to_string()),
        }
    }
}

/// Resolves user-entered remote path (absolute, relative, or `~`/`~/...`) against `current_dir` and `home_dir`.
pub fn resolve_remote_input_path(input: &str, current_dir: &str, home_dir: Option<&str>) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed == "." {
        return normalize_posix_path(current_dir);
    }

    if trimmed == "~" {
        return home_dir
            .map(normalize_posix_path)
            .unwrap_or_else(|| ".".to_string());
    }

    if let Some(rest) = trimmed.strip_prefix("~/") {
        let base = home_dir.unwrap_or(current_dir);
        return normalize_posix_path(&join_posix_path(base, rest));
    }

    if trimmed.starts_with('/') {
        normalize_posix_path(trimmed)
    } else {
        normalize_posix_path(&join_posix_path(current_dir, trimmed))
    }
}

/// Maps SFTP `FileAttributes` into `FileType` and `FileMetadata`.
/// Never fabricates `Changed`, `Rights`, or `Owner` values if omitted by the server,
/// and preserves distinction between unavailable owner/group names and numeric UID/GID.
pub fn map_sftp_attributes(
    attrs: &FileAttributes,
    symlink_target: Option<String>,
) -> (FileType, FileMetadata) {
    let file_type = if attrs.is_dir() {
        FileType::Directory
    } else if attrs.is_symlink() {
        FileType::Symlink {
            target: symlink_target,
        }
    } else if attrs.is_regular() {
        FileType::Regular
    } else if attrs.permissions.is_some() {
        FileType::Special
    } else {
        FileType::Regular
    };

    let permissions = attrs.permissions.map(UnixPermissions::from_mode);

    let user_name = attrs
        .user
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from);
    let group_name = attrs
        .group
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from);

    let ownership = if attrs.uid.is_none()
        && attrs.gid.is_none()
        && user_name.is_none()
        && group_name.is_none()
    {
        None
    } else {
        Some(FileOwnership {
            uid: attrs.uid,
            gid: attrs.gid,
            user_name,
            group_name,
        })
    };

    let size = if file_type.is_dir() { None } else { attrs.size };

    let metadata = FileMetadata {
        size,
        modified_at_secs: attrs.mtime.map(u64::from),
        accessed_at_secs: attrs.atime.map(u64::from),
        permissions,
        ownership,
    };

    (file_type, metadata)
}

/// Classifies an SFTP client error into a typed `AppError` for accurate UI state handling.
pub fn classify_sftp_error(err: SftpClientError, path: &str) -> AppError {
    match err {
        SftpClientError::Status(status) => match status.status_code {
            StatusCode::PermissionDenied => AppError::PermissionDenied(format!(
                "Permission denied accessing '{}': {}",
                path, status.error_message
            )),
            StatusCode::NoSuchFile => AppError::Filesystem(format!(
                "Remote path does not exist '{}': {}",
                path, status.error_message
            )),
            StatusCode::OpUnsupported => AppError::Protocol(format!(
                "Operation unsupported by remote SFTP server on '{}': {}",
                path, status.error_message
            )),
            StatusCode::NoConnection | StatusCode::ConnectionLost => AppError::Network(format!(
                "SFTP connection lost while accessing '{}': {}",
                path, status.error_message
            )),
            _ => AppError::Protocol(format!(
                "SFTP error on '{}' ({}): {}",
                path, status.status_code, status.error_message
            )),
        },
        SftpClientError::Timeout => {
            AppError::Network(format!("SFTP request timed out while accessing '{}'", path))
        }
        SftpClientError::IO(msg) => {
            AppError::Network(format!("Network I/O error accessing '{}': {}", path, msg))
        }
        other => AppError::Protocol(format!("SFTP protocol error on '{}': {}", path, other)),
    }
}

/// Lists a remote directory over an active `SftpSession` without blocking the UI.
pub async fn list_sftp_directory(
    sftp: &SftpSession,
    target_path: Option<&str>,
    current_path: &str,
    home_path: Option<&str>,
    show_hidden: bool,
    timeout_duration: Duration,
) -> Result<RemoteDirectoryListing, AppError> {
    let raw_target = target_path
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(current_path);

    let resolved_input = resolve_remote_input_path(raw_target, current_path, home_path);

    let canonical_path =
        tokio::time::timeout(timeout_duration, sftp.canonicalize(resolved_input.as_str()))
            .await
            .map_err(|_| {
                AppError::Network(format!(
                    "Timed out resolving remote path '{}'",
                    resolved_input
                ))
            })?
            .map_err(|e| classify_sftp_error(e, &resolved_input))?;

    let canonical_norm = normalize_posix_path(&canonical_path);

    let read_dir = tokio::time::timeout(timeout_duration, sftp.read_dir(canonical_norm.as_str()))
        .await
        .map_err(|_| {
            AppError::Network(format!(
                "Timed out listing remote directory '{}'",
                canonical_norm
            ))
        })?
        .map_err(|e| classify_sftp_error(e, &canonical_norm))?;

    let link_timeout = timeout_duration.min(Duration::from_secs(3));
    let mut entries = Vec::new();

    for item in read_dir {
        let name = item.file_name();
        let is_hidden = name.starts_with('.') && name != "." && name != "..";
        if !show_hidden && is_hidden {
            continue;
        }

        let entry_path = join_posix_path(&canonical_norm, &name);
        let attrs = item.metadata();

        let symlink_target = if attrs.is_symlink() {
            tokio::time::timeout(link_timeout, sftp.read_link(entry_path.as_str()))
                .await
                .ok()
                .and_then(|res| res.ok())
        } else {
            None
        };

        let (file_type, metadata) = map_sftp_attributes(&attrs, symlink_target);
        entries.push(FileEntry::new(name, entry_path, file_type, metadata));
    }

    Ok(RemoteDirectoryListing {
        parent_path: parent_posix_path(&canonical_norm),
        current_path: canonical_norm,
        home_path: home_path.map(String::from),
        entries,
    })
}

/// Checks via `stat` (following symlinks) whether a remote path points to a directory.
pub async fn check_sftp_path_is_dir(
    sftp: &SftpSession,
    path: &str,
    timeout_duration: Duration,
) -> Result<bool, AppError> {
    let attrs = tokio::time::timeout(timeout_duration, sftp.metadata(path))
        .await
        .map_err(|_| AppError::Network(format!("Timed out checking remote path '{}'", path)))?
        .map_err(|e| classify_sftp_error(e, path))?;
    Ok(attrs.is_dir())
}

/// Validates a single remote file or directory name (no slashes, NULs, `.` or `..`).
pub fn validate_remote_entry_name(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation("Name cannot be empty".to_string()));
    }
    if trimmed == "." || trimmed == ".." {
        return Err(AppError::Validation(format!(
            "Invalid name '{}': '.' and '..' are reserved",
            trimmed
        )));
    }
    if trimmed.contains('/') || trimmed.contains('\0') {
        return Err(AppError::Validation(
            "Name cannot contain '/' or null bytes".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_remote_path(path: &str) -> Result<String, AppError> {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed.contains('\0') {
        return Err(AppError::Validation("Invalid remote path".to_string()));
    }
    Ok(normalize_posix_path(trimmed))
}

/// SFTP implementation of the `RemoteFileSystem` protocol adapter boundary.
pub struct SftpFileSystemAdapter {
    sftp: Arc<SftpSession>,
    capabilities: ProtocolCapabilities,
    timeout: Duration,
}

impl SftpFileSystemAdapter {
    pub fn new(
        sftp: Arc<SftpSession>,
        capabilities: ProtocolCapabilities,
        timeout: Duration,
    ) -> Self {
        Self {
            sftp,
            capabilities,
            timeout,
        }
    }

    async fn path_exists_lstat(&self, path: &str) -> Result<bool, AppError> {
        let res = tokio::time::timeout(self.timeout, self.sftp.symlink_metadata(path))
            .await
            .map_err(|_| AppError::Network(format!("Timed out checking '{}'", path)))?;
        match res {
            Ok(_) => Ok(true),
            Err(SftpClientError::Status(status))
                if status.status_code == StatusCode::NoSuchFile =>
            {
                Ok(false)
            }
            Err(e) => Err(classify_sftp_error(e, path)),
        }
    }
}

#[async_trait]
impl RemoteFileSystem for SftpFileSystemAdapter {
    fn capabilities(&self) -> &ProtocolCapabilities {
        &self.capabilities
    }

    async fn connect(&mut self) -> Result<(), AppError> {
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), AppError> {
        Ok(())
    }

    async fn do_list_directory(&self, path: &str) -> Result<Vec<FileEntry>, AppError> {
        let listing =
            list_sftp_directory(&self.sftp, Some(path), path, None, true, self.timeout).await?;
        Ok(listing.entries)
    }

    async fn do_stat(&self, path: &str) -> Result<FileMetadata, AppError> {
        let norm = validate_remote_path(path)?;
        let attrs = tokio::time::timeout(self.timeout, self.sftp.metadata(norm.as_str()))
            .await
            .map_err(|_| AppError::Network(format!("Timed out stating '{}'", norm)))?
            .map_err(|e| classify_sftp_error(e, &norm))?;
        let (_, metadata) = map_sftp_attributes(&attrs, None);
        Ok(metadata)
    }

    async fn do_create_file(&self, path: &str) -> Result<(), AppError> {
        let norm = validate_remote_path(path)?;
        if norm == "/" {
            return Err(AppError::Validation(
                "Cannot create file at root '/'".to_string(),
            ));
        }
        if self.path_exists_lstat(&norm).await? {
            return Err(AppError::Filesystem(format!(
                "Cannot create remote file '{}': path already exists",
                norm
            )));
        }

        let mut file = tokio::time::timeout(
            self.timeout,
            self.sftp.open_with_flags(
                norm.as_str(),
                OpenFlags::CREATE | OpenFlags::EXCLUDE | OpenFlags::WRITE,
            ),
        )
        .await
        .map_err(|_| AppError::Network(format!("Timed out creating remote file '{}'", norm)))?
        .map_err(|e| classify_sftp_error(e, &norm))?;

        tokio::time::timeout(self.timeout, file.shutdown())
            .await
            .map_err(|_| AppError::Network(format!("Timed out closing remote file '{}'", norm)))?
            .map_err(|e| AppError::Filesystem(format!("Failed to finalize '{}': {}", norm, e)))?;

        Ok(())
    }

    async fn do_create_directory(&self, path: &str) -> Result<(), AppError> {
        let norm = validate_remote_path(path)?;
        if norm == "/" {
            return Err(AppError::Validation(
                "Cannot create root directory '/'".to_string(),
            ));
        }
        if self.path_exists_lstat(&norm).await? {
            return Err(AppError::Filesystem(format!(
                "Cannot create remote directory '{}': path already exists",
                norm
            )));
        }

        tokio::time::timeout(self.timeout, self.sftp.create_dir(norm.as_str()))
            .await
            .map_err(|_| {
                AppError::Network(format!("Timed out creating remote directory '{}'", norm))
            })?
            .map_err(|e| classify_sftp_error(e, &norm))?;

        Ok(())
    }

    async fn do_rename(&self, old_path: &str, new_path: &str) -> Result<(), AppError> {
        let old_norm = validate_remote_path(old_path)?;
        let new_norm = validate_remote_path(new_path)?;
        if old_norm == "/" || new_norm == "/" {
            return Err(AppError::Validation(
                "Cannot rename root directory '/'".to_string(),
            ));
        }
        if old_norm == new_norm {
            return Err(AppError::Validation(
                "Source and destination names are identical".to_string(),
            ));
        }
        if self.path_exists_lstat(&new_norm).await? {
            return Err(AppError::Filesystem(format!(
                "Cannot rename to '{}': destination already exists",
                new_norm
            )));
        }

        tokio::time::timeout(
            self.timeout,
            self.sftp.rename(old_norm.as_str(), new_norm.as_str()),
        )
        .await
        .map_err(|_| AppError::Network(format!("Timed out renaming '{}'", old_norm)))?
        .map_err(|e| classify_sftp_error(e, &old_norm))?;

        Ok(())
    }

    async fn do_delete_file(&self, path: &str) -> Result<(), AppError> {
        let norm = validate_remote_path(path)?;
        if norm == "/" {
            return Err(AppError::Validation(
                "Refusing to delete remote root directory '/'".to_string(),
            ));
        }

        tokio::time::timeout(self.timeout, self.sftp.remove_file(norm.as_str()))
            .await
            .map_err(|_| AppError::Network(format!("Timed out deleting remote file '{}'", norm)))?
            .map_err(|e| classify_sftp_error(e, &norm))?;

        Ok(())
    }

    async fn do_delete_directory(&self, path: &str, recursive: bool) -> Result<(), AppError> {
        let norm = validate_remote_path(path)?;
        if norm == "/" {
            return Err(AppError::Validation(
                "Refusing to delete remote root directory '/'".to_string(),
            ));
        }

        if !recursive {
            let res = tokio::time::timeout(self.timeout, self.sftp.remove_dir(norm.as_str()))
                .await
                .map_err(|_| {
                    AppError::Network(format!("Timed out deleting remote directory '{}'", norm))
                })?;

            match res {
                Ok(()) => return Ok(()),
                Err(SftpClientError::Status(status))
                    if status.status_code == StatusCode::Failure =>
                {
                    // Check if directory is non-empty so caller can prompt for explicit recursive confirmation
                    if let Ok(Ok(mut entries)) =
                        tokio::time::timeout(self.timeout, self.sftp.read_dir(norm.as_str())).await
                    {
                        if entries.next().is_some() {
                            return Err(AppError::Filesystem(format!(
                                "Remote directory '{}' is not empty. Recursive deletion requires explicit confirmation.",
                                norm
                            )));
                        }
                    }
                    return Err(classify_sftp_error(SftpClientError::Status(status), &norm));
                }
                Err(e) => return Err(classify_sftp_error(e, &norm)),
            }
        }

        // Recursive bottom-up deletion (bounded depth, never follows symlinks)
        const MAX_DEPTH: usize = 64;
        struct Frame {
            dir_path: String,
            depth: usize,
            visited: bool,
        }

        let mut stack = vec![Frame {
            dir_path: norm,
            depth: 0,
            visited: false,
        }];

        while let Some(frame) = stack.pop() {
            if frame.depth > MAX_DEPTH {
                return Err(AppError::Filesystem(format!(
                    "Refusing recursive deletion beyond max depth ({}) at '{}'",
                    MAX_DEPTH, frame.dir_path
                )));
            }

            if frame.visited {
                tokio::time::timeout(self.timeout, self.sftp.remove_dir(frame.dir_path.as_str()))
                    .await
                    .map_err(|_| {
                        AppError::Network(format!(
                            "Timed out deleting directory '{}'",
                            frame.dir_path
                        ))
                    })?
                    .map_err(|e| classify_sftp_error(e, &frame.dir_path))?;
                continue;
            }

            let read_dir =
                tokio::time::timeout(self.timeout, self.sftp.read_dir(frame.dir_path.as_str()))
                    .await
                    .map_err(|_| {
                        AppError::Network(format!(
                            "Timed out reading directory '{}'",
                            frame.dir_path
                        ))
                    })?
                    .map_err(|e| classify_sftp_error(e, &frame.dir_path))?;

            let mut subdirs = Vec::new();
            for item in read_dir {
                let name = item.file_name();
                if name == "." || name == ".." {
                    continue;
                }
                let child_path = join_posix_path(&frame.dir_path, &name);
                let attrs = item.metadata();
                // Never follow symlinks during recursive deletion; remove the link itself
                if attrs.is_dir() && !attrs.is_symlink() {
                    subdirs.push(child_path);
                } else {
                    tokio::time::timeout(self.timeout, self.sftp.remove_file(child_path.as_str()))
                        .await
                        .map_err(|_| {
                            AppError::Network(format!("Timed out deleting '{}'", child_path))
                        })?
                        .map_err(|e| classify_sftp_error(e, &child_path))?;
                }
            }

            stack.push(Frame {
                dir_path: frame.dir_path,
                depth: frame.depth,
                visited: true,
            });
            for sub in subdirs {
                stack.push(Frame {
                    dir_path: sub,
                    depth: frame.depth + 1,
                    visited: false,
                });
            }
        }

        Ok(())
    }

    async fn do_set_permissions(
        &self,
        path: &str,
        permissions: UnixPermissions,
    ) -> Result<(), AppError> {
        let norm = validate_remote_path(path)?;
        let mode = permissions.to_octal_number();
        if mode > 0o7777 {
            return Err(AppError::Validation(format!(
                "Invalid Unix permission mode '{:o}': must be between 0000 and 7777",
                mode
            )));
        }

        let attrs = FileAttributes {
            permissions: Some(mode),
            ..Default::default()
        };

        tokio::time::timeout(self.timeout, self.sftp.set_metadata(norm.as_str(), attrs))
            .await
            .map_err(|_| AppError::Network(format!("Timed out setting permissions on '{}'", norm)))?
            .map_err(|e| classify_sftp_error(e, &norm))?;

        Ok(())
    }

    async fn do_set_owner_group(
        &self,
        path: &str,
        ownership: FileOwnership,
    ) -> Result<(), AppError> {
        let norm = validate_remote_path(path)?;
        if ownership.uid.is_none() && ownership.gid.is_none() {
            return Err(AppError::Validation(
                "At least one numeric UID or GID is required to change ownership".to_string(),
            ));
        }

        let attrs = FileAttributes {
            uid: ownership.uid,
            gid: ownership.gid,
            ..Default::default()
        };

        tokio::time::timeout(self.timeout, self.sftp.set_metadata(norm.as_str(), attrs))
            .await
            .map_err(|_| AppError::Network(format!("Timed out setting ownership on '{}'", norm)))?
            .map_err(|e| match e {
                SftpClientError::Status(status)
                    if status.status_code == StatusCode::PermissionDenied
                        || status.status_code == StatusCode::Failure =>
                {
                    AppError::PermissionDenied(format!(
                        "Server rejected ownership change on '{}': {} (requires privileged ownership or matching group membership)",
                        norm, status.error_message
                    ))
                }
                other => classify_sftp_error(other, &norm),
            })?;

        Ok(())
    }
}

/// Validates and parses a 3- or 4-digit Unix octal permission string (e.g. "755", "0644", "4755").
pub fn parse_octal_mode(input: &str) -> Result<UnixPermissions, AppError> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.len() > 4 {
        return Err(AppError::Validation(
            "Octal mode must be 3 or 4 digits (000–7777)".to_string(),
        ));
    }
    if !trimmed.chars().all(|c| ('0'..='7').contains(&c)) {
        return Err(AppError::Validation(format!(
            "Invalid octal mode '{}': only digits 0-7 are allowed",
            trimmed
        )));
    }
    let mode = u32::from_str_radix(trimmed, 8)
        .map_err(|_| AppError::Validation(format!("Invalid octal mode '{}'", trimmed)))?;
    if mode > 0o7777 {
        return Err(AppError::Validation(format!(
            "Octal mode '{}' exceeds 7777",
            trimmed
        )));
    }
    Ok(UnixPermissions::from_mode(mode))
}

/// Applies permissions via the `RemoteFileSystem` adapter, optionally recursing into directories
/// (bounded depth, never following symlinks, never silently retrying).
pub async fn apply_remote_permissions_with_adapter(
    adapter: &dyn RemoteFileSystem,
    sftp: &SftpSession,
    paths: &[String],
    permissions: UnixPermissions,
    recursive: bool,
    timeout: Duration,
) -> Result<(), AppError> {
    const MAX_DEPTH: usize = 64;

    for raw_path in paths {
        let norm = validate_remote_path(raw_path)?;
        adapter.set_permissions(&norm, permissions).await?;

        if recursive {
            let lstat = tokio::time::timeout(timeout, sftp.symlink_metadata(norm.as_str()))
                .await
                .map_err(|_| AppError::Network(format!("Timed out stating '{}'", norm)))?
                .map_err(|e| classify_sftp_error(e, &norm))?;

            if lstat.is_dir() && !lstat.is_symlink() {
                let mut stack = vec![(norm, 0usize)];
                while let Some((dir_path, depth)) = stack.pop() {
                    if depth >= MAX_DEPTH {
                        return Err(AppError::Filesystem(format!(
                            "Refusing recursive chmod beyond max depth ({}) at '{}'",
                            MAX_DEPTH, dir_path
                        )));
                    }

                    let entries = tokio::time::timeout(timeout, sftp.read_dir(dir_path.as_str()))
                        .await
                        .map_err(|_| {
                            AppError::Network(format!("Timed out reading '{}'", dir_path))
                        })?
                        .map_err(|e| classify_sftp_error(e, &dir_path))?;

                    for item in entries {
                        let name = item.file_name();
                        if name == "." || name == ".." {
                            continue;
                        }
                        let child_path = join_posix_path(&dir_path, &name);
                        let attrs = item.metadata();
                        if attrs.is_symlink() {
                            continue;
                        }
                        adapter.set_permissions(&child_path, permissions).await?;
                        if attrs.is_dir() {
                            stack.push((child_path, depth + 1));
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn validate_posix_identity_name(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.len() > 64 {
        return Err(AppError::Validation(
            "User/group name must be 1-64 characters".to_string(),
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return Err(AppError::Validation(format!(
            "Invalid user/group name '{}': only ASCII letters, digits, '.', '_', '-' allowed",
            trimmed
        )));
    }
    Ok(trimmed.to_string())
}

async fn exec_ssh_short_command(
    ssh_handle: &Handle<SshClientHandler>,
    command: &str,
    timeout: Duration,
) -> Result<String, AppError> {
    let mut channel = tokio::time::timeout(timeout, ssh_handle.channel_open_session())
        .await
        .map_err(|_| AppError::Network("Timed out opening SSH channel for identity lookup".into()))?
        .map_err(|e| AppError::Protocol(format!("SSH channel open failed: {}", e)))?;

    tokio::time::timeout(timeout, channel.exec(true, command))
        .await
        .map_err(|_| AppError::Network("Timed out executing remote getent lookup".into()))?
        .map_err(|e| AppError::Protocol(format!("Remote exec failed: {}", e)))?;

    let mut stdout = Vec::new();
    let mut exit_status: Option<u32> = None;

    let read_fut = async {
        while let Some(msg) = channel.wait().await {
            match msg {
                russh::ChannelMsg::Data { data } => {
                    if stdout.len() + data.len() <= 8192 {
                        stdout.extend_from_slice(&data);
                    }
                }
                russh::ChannelMsg::ExitStatus { exit_status: code } => {
                    exit_status = Some(code);
                }
                russh::ChannelMsg::Close => break,
                _ => {}
            }
        }
    };

    let _ = tokio::time::timeout(timeout, read_fut).await;
    let _ = channel.close().await;

    if exit_status.unwrap_or(1) != 0 {
        return Err(AppError::Filesystem(
            "Remote getent lookup returned no entry".to_string(),
        ));
    }

    Ok(String::from_utf8_lossy(&stdout).trim().to_string())
}

/// Optional, controlled `getent` lookup to resolve numeric UID/GID to names.
/// Returns `None` for any name that cannot be resolved without fabricating values.
pub async fn resolve_remote_identity_via_getent(
    ssh_handle: &Handle<SshClientHandler>,
    uid: Option<u32>,
    gid: Option<u32>,
) -> FileOwnership {
    let lookup_timeout = Duration::from_secs(3);

    let user_name = if let Some(u) = uid {
        let cmd = format!("getent passwd {}", u);
        exec_ssh_short_command(ssh_handle, &cmd, lookup_timeout)
            .await
            .ok()
            .and_then(|line| {
                line.split(':')
                    .next()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from)
            })
    } else {
        None
    };

    let group_name = if let Some(g) = gid {
        let cmd = format!("getent group {}", g);
        exec_ssh_short_command(ssh_handle, &cmd, lookup_timeout)
            .await
            .ok()
            .and_then(|line| {
                line.split(':')
                    .next()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from)
            })
    } else {
        None
    };

    FileOwnership {
        uid,
        gid,
        user_name,
        group_name,
    }
}

/// Resolves a user or group name to its numeric ID via remote `getent`, if supported.
pub async fn resolve_name_to_id_via_getent(
    ssh_handle: &Handle<SshClientHandler>,
    database: &str,
    name: &str,
) -> Result<u32, AppError> {
    let safe_name = validate_posix_identity_name(name)?;
    let cmd = format!("getent {} {}", database, safe_name);
    let line = exec_ssh_short_command(ssh_handle, &cmd, Duration::from_secs(3)).await?;
    let id_str = line.split(':').nth(2).ok_or_else(|| {
        AppError::Filesystem(format!("Unexpected getent output for '{}'", safe_name))
    })?;
    id_str.trim().parse::<u32>().map_err(|_| {
        AppError::Filesystem(format!(
            "Invalid numeric ID from getent for '{}'",
            safe_name
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rejects_non_sftp_protocol() {
        let profile = ConnectionProfile::new(
            "FTP Profile".to_string(),
            ProtocolType::Ftp,
            "127.0.0.1".to_string(),
            "user".to_string(),
            AuthMethod::Password,
        );

        let res = establish_sftp_connection(&profile, Some("pass".to_string())).await;
        assert!(res.is_err());
        assert!(matches!(res, Err(AppError::Protocol(_))));
    }

    #[tokio::test]
    async fn test_live_sftp_connection_and_host_key_verification_flow() {
        let test_host = "192.168.122.10";
        let test_port = 22u16;

        // Check if the local test VM is reachable before running the live network portion
        if tokio::time::timeout(
            Duration::from_secs(2),
            tokio::net::TcpStream::connect((test_host, test_port)),
        )
        .await
        .is_err()
        {
            return;
        }

        let db = Database::open_default().expect("Should open default DB");
        let kh_repo = KnownHostsRepository::new(&db);

        // Clean any existing entry for the test host so we test the full Unknown -> Trust -> Connected -> Changed flow
        let _ = kh_repo.delete_known_host(test_host, test_port);

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

        // 1. First connection attempt must halt with HostKeyUnknown
        let first_attempt = establish_sftp_connection(&profile, None)
            .await
            .expect("Handshake should return HostKeyUnknown outcome");

        let (alg, fp, pubkey) = match first_attempt {
            Err(ConnectOutcome::HostKeyUnknown {
                host,
                port,
                algorithm,
                fingerprint_sha256,
                public_key_openssh,
            }) => {
                assert_eq!(host, test_host);
                assert_eq!(port, test_port);
                assert!(fingerprint_sha256.starts_with("SHA256:"));
                (algorithm, fingerprint_sha256, public_key_openssh)
            }
            _ => panic!("Expected HostKeyUnknown on first connection to untrusted host"),
        };

        // 2. Explicitly trust the host key
        kh_repo
            .trust_unknown_host_key(test_host, test_port, &alg, &fp, &pubkey)
            .expect("Should store trusted host key");

        // 3. Second connection attempt must succeed and initialize the SFTP subsystem
        let second_attempt = establish_sftp_connection(&profile, None)
            .await
            .expect("Should establish SFTP connection");

        let active_conn = match second_attempt {
            Ok(conn) => conn,
            Err(other) => panic!("Expected Connected, got {:?}", other),
        };

        assert_eq!(active_conn.session_meta.status, SessionStatus::Connected);
        assert_eq!(active_conn.session_meta.current_remote_path, "/home/ubuntu");
        assert_eq!(active_conn.trusted_host_key.fingerprint_sha256, fp);

        // Close connection cleanly
        active_conn.close().await.expect("Should close cleanly");

        // 4. Simulate a changed host key in the store and verify connection is strictly blocked
        kh_repo.delete_known_host(test_host, test_port).unwrap();
        kh_repo
            .trust_unknown_host_key(
                test_host,
                test_port,
                "ssh-ed25519",
                "SHA256:mismatched_stored_fingerprint_000000",
                "ssh-ed25519 FAKEKEY",
            )
            .unwrap();

        let third_attempt = establish_sftp_connection(&profile, None)
            .await
            .expect("Handshake should return HostKeyChanged outcome");

        match third_attempt {
            Err(ConnectOutcome::HostKeyChanged {
                expected_fingerprint,
                presented_fingerprint,
                ..
            }) => {
                assert_eq!(
                    expected_fingerprint,
                    "SHA256:mismatched_stored_fingerprint_000000"
                );
                assert_eq!(presented_fingerprint, fp);
            }
            _ => panic!("Expected HostKeyChanged when stored fingerprint differs"),
        }

        // Restore the authentic trusted key
        kh_repo.delete_known_host(test_host, test_port).unwrap();
        kh_repo
            .trust_unknown_host_key(test_host, test_port, &alg, &fp, &pubkey)
            .unwrap();
    }

    #[test]
    fn test_normalize_posix_path() {
        assert_eq!(normalize_posix_path("/"), "/");
        assert_eq!(normalize_posix_path(""), "/");
        assert_eq!(normalize_posix_path("///"), "/");
        assert_eq!(normalize_posix_path("//var///www//html/"), "/var/www/html");
        assert_eq!(normalize_posix_path("/var/./www/."), "/var/www");
        assert_eq!(normalize_posix_path("/var/www/../log"), "/var/log");
        assert_eq!(normalize_posix_path("/../../etc"), "/etc");
        assert_eq!(normalize_posix_path("/.."), "/");
        assert_eq!(
            normalize_posix_path("/home/user/My Docs/café 日本語/"),
            "/home/user/My Docs/café 日本語"
        );
    }

    #[test]
    fn test_resolve_remote_input_path() {
        let current = "/home/ubuntu/projects";
        let home = Some("/home/ubuntu");

        assert_eq!(
            resolve_remote_input_path("", current, home),
            "/home/ubuntu/projects"
        );
        assert_eq!(
            resolve_remote_input_path(".", current, home),
            "/home/ubuntu/projects"
        );
        assert_eq!(
            resolve_remote_input_path("~", current, home),
            "/home/ubuntu"
        );
        assert_eq!(
            resolve_remote_input_path("~/docs/My File.txt", current, home),
            "/home/ubuntu/docs/My File.txt"
        );
        assert_eq!(
            resolve_remote_input_path("sub dir/日本語", current, home),
            "/home/ubuntu/projects/sub dir/日本語"
        );
        assert_eq!(
            resolve_remote_input_path("../other", current, home),
            "/home/ubuntu/other"
        );
        assert_eq!(
            resolve_remote_input_path("/var/log//nginx/", current, home),
            "/var/log/nginx"
        );
    }

    #[test]
    fn test_parent_posix_path() {
        assert_eq!(parent_posix_path("/"), None);
        assert_eq!(parent_posix_path("/home"), Some("/".to_string()));
        assert_eq!(
            parent_posix_path("/home/ubuntu/docs/"),
            Some("/home/ubuntu".to_string())
        );
    }

    #[test]
    fn test_map_sftp_attributes_does_not_invent_missing_metadata() {
        // Empty attributes from server -> no fabricated Changed, Rights, or Owner
        let empty_attrs = FileAttributes::default();
        let (ftype, meta) = map_sftp_attributes(&empty_attrs, None);
        assert_eq!(ftype, FileType::Regular);
        assert_eq!(meta.size, None);
        assert_eq!(meta.modified_at_secs, None);
        assert_eq!(meta.accessed_at_secs, None);
        assert_eq!(meta.permissions, None);
        assert_eq!(meta.ownership, None);

        // Numeric UID/GID without names -> user_name/group_name remain None
        let numeric_attrs = FileAttributes {
            size: Some(4096),
            uid: Some(1000),
            user: None,
            gid: Some(1001),
            group: Some("".to_string()),
            permissions: Some(0o100644),
            atime: Some(1700000000),
            mtime: Some(1700000100),
        };
        let (ftype2, meta2) = map_sftp_attributes(&numeric_attrs, None);
        assert_eq!(ftype2, FileType::Regular);
        assert_eq!(meta2.size, Some(4096));
        assert_eq!(meta2.modified_at_secs, Some(1700000100));
        assert_eq!(
            meta2.permissions.map(|p| p.to_symbolic_string()),
            Some("rw-r--r--".to_string())
        );
        let ownership = meta2.ownership.expect("Ownership should be present");
        assert_eq!(ownership.uid, Some(1000));
        assert_eq!(ownership.gid, Some(1001));
        assert_eq!(ownership.user_name, None);
        assert_eq!(ownership.group_name, None);

        // Directory size is omitted (None -> "—")
        let dir_attrs = FileAttributes {
            size: Some(4096),
            permissions: Some(0o040755),
            ..Default::default()
        };
        let (dir_type, dir_meta) = map_sftp_attributes(&dir_attrs, None);
        assert_eq!(dir_type, FileType::Directory);
        assert_eq!(dir_meta.size, None);

        // Symlink preserves target
        let link_attrs = FileAttributes {
            permissions: Some(0o120777),
            ..Default::default()
        };
        let (link_type, _) = map_sftp_attributes(&link_attrs, Some("/var/log".to_string()));
        assert_eq!(
            link_type,
            FileType::Symlink {
                target: Some("/var/log".to_string())
            }
        );
    }

    #[test]
    fn test_classify_sftp_error_maps_failure_states() {
        use russh_sftp::protocol::Status;

        let perm_err = SftpClientError::Status(Status {
            id: 1,
            status_code: StatusCode::PermissionDenied,
            error_message: "Permission denied".to_string(),
            language_tag: "en".to_string(),
        });
        assert!(matches!(
            classify_sftp_error(perm_err, "/root"),
            AppError::PermissionDenied(_)
        ));

        let not_found_err = SftpClientError::Status(Status {
            id: 2,
            status_code: StatusCode::NoSuchFile,
            error_message: "No such file".to_string(),
            language_tag: "en".to_string(),
        });
        assert!(matches!(
            classify_sftp_error(not_found_err, "/missing"),
            AppError::Filesystem(_)
        ));

        let lost_err = SftpClientError::Status(Status {
            id: 3,
            status_code: StatusCode::ConnectionLost,
            error_message: "Connection lost".to_string(),
            language_tag: "en".to_string(),
        });
        assert!(matches!(
            classify_sftp_error(lost_err, "/home"),
            AppError::Network(_)
        ));

        assert!(matches!(
            classify_sftp_error(SftpClientError::Timeout, "/home"),
            AppError::Network(_)
        ));
    }

    #[tokio::test]
    async fn test_live_sftp_browsing_paths_metadata_and_failures() {
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

        let conn = match establish_sftp_connection(&profile, None).await {
            Ok(Ok(c)) => c,
            _ => return,
        };

        let home = conn
            .session_meta
            .home_remote_path
            .as_deref()
            .unwrap_or("/home/ubuntu");
        let timeout = Duration::from_secs(10);

        // 1. List home directory (hidden files excluded vs included)
        let listing_no_hidden = list_sftp_directory(
            &conn.sftp_session,
            Some(home),
            home,
            Some(home),
            false,
            timeout,
        )
        .await
        .expect("Should list home dir");
        assert_eq!(listing_no_hidden.current_path, home);
        assert!(listing_no_hidden.entries.iter().all(|e| !e.is_hidden));

        let listing_with_hidden = list_sftp_directory(
            &conn.sftp_session,
            Some(home),
            home,
            Some(home),
            true,
            timeout,
        )
        .await
        .expect("Should list home dir with hidden files");
        assert!(listing_with_hidden.entries.len() >= listing_no_hidden.entries.len());

        // 2. List root `/`
        let root_listing = list_sftp_directory(
            &conn.sftp_session,
            Some("/"),
            home,
            Some(home),
            false,
            timeout,
        )
        .await
        .expect("Should list root");
        assert_eq!(root_listing.current_path, "/");
        assert_eq!(root_listing.parent_path, None);

        // 3. Relative path resolution (`~` -> home)
        let tilde_listing = list_sftp_directory(
            &conn.sftp_session,
            Some("~"),
            "/",
            Some(home),
            false,
            timeout,
        )
        .await
        .expect("Should resolve tilde");
        assert_eq!(tilde_listing.current_path, home);

        // 4. Permission denied on `/root`
        let perm_res = list_sftp_directory(
            &conn.sftp_session,
            Some("/root"),
            home,
            Some(home),
            false,
            timeout,
        )
        .await;
        assert!(matches!(perm_res, Err(AppError::PermissionDenied(_))));

        // 5. Non-existent path
        let missing_res = list_sftp_directory(
            &conn.sftp_session,
            Some("/definitely_nonexistent_dir_12345"),
            home,
            Some(home),
            false,
            timeout,
        )
        .await;
        assert!(matches!(missing_res, Err(AppError::Filesystem(_))));

        conn.close().await.expect("Should close cleanly");
    }

    #[test]
    fn test_validate_remote_entry_name_and_error_classification() {
        assert!(validate_remote_entry_name("").is_err());
        assert!(validate_remote_entry_name(".").is_err());
        assert!(validate_remote_entry_name("..").is_err());
        assert!(validate_remote_entry_name("a/b").is_err());
        assert_eq!(
            validate_remote_entry_name(" valid.txt ").unwrap(),
            "valid.txt"
        );

        let unsupported = classify_sftp_error(
            SftpClientError::Status(russh_sftp::protocol::Status {
                id: 1,
                status_code: StatusCode::OpUnsupported,
                error_message: "unsupported".to_string(),
                language_tag: "en".to_string(),
            }),
            "/tmp/x",
        );
        assert!(matches!(unsupported, AppError::Protocol(_)));
    }

    #[tokio::test]
    async fn test_live_sftp_remote_file_operations_and_capabilities() {
        let host = "192.168.122.10";
        let port = 22u16;
        if tokio::net::TcpStream::connect((host, port)).await.is_err() {
            eprintln!(
                "Skipping live SFTP remote ops test: {}:{} unreachable",
                host, port
            );
            return;
        }

        let mut profile = ConnectionProfile::new(
            "Live SFTP Ops Test".to_string(),
            ProtocolType::Sftp,
            host.to_string(),
            "ubuntu".to_string(),
            AuthMethod::PublicKey {
                private_key_path: "~/.ssh/id_rsa".to_string(),
                requires_passphrase: false,
            },
        );
        profile.port = port;
        profile.connection_settings.timeout_seconds = 10;

        let conn = match establish_sftp_connection(&profile, None).await {
            Ok(Ok(conn)) => conn,
            _ => {
                eprintln!("Skipping live SFTP remote ops test: key/host not ready");
                return;
            }
        };

        let timeout = Duration::from_secs(10);
        let adapter = SftpFileSystemAdapter::new(
            Arc::clone(&conn.sftp_session),
            conn.session_meta.capabilities.clone(),
            timeout,
        );

        let home = conn
            .session_meta
            .home_remote_path
            .as_deref()
            .unwrap_or("/tmp");
        let test_root = join_posix_path(
            home,
            &format!("linuxscp_ops_test_{}", uuid::Uuid::new_v4().simple()),
        );

        // 1. Create directory & reject duplicate create_directory
        adapter
            .create_directory(&test_root)
            .await
            .expect("create_directory should succeed");
        let dup_dir = adapter.create_directory(&test_root).await;
        assert!(matches!(dup_dir, Err(AppError::Filesystem(_))));

        // 2. Create empty file & reject duplicate create_file
        let file_a = join_posix_path(&test_root, "alpha.txt");
        adapter
            .create_file(&file_a)
            .await
            .expect("create_file should succeed");
        let dup_file = adapter.create_file(&file_a).await;
        assert!(matches!(dup_file, Err(AppError::Filesystem(_))));

        // 3. Rename file
        let file_b = join_posix_path(&test_root, "beta.txt");
        adapter
            .rename(&file_a, &file_b)
            .await
            .expect("rename should succeed");

        // 4. Non-empty delete_directory(false) must fail with clear non-empty error
        let non_empty_err = adapter.delete_directory(&test_root, false).await;
        assert!(
            matches!(non_empty_err, Err(AppError::Filesystem(ref m)) if m.contains("not empty")),
            "Expected non-empty directory error, got: {:?}",
            non_empty_err
        );

        // 5. Permission denied when creating inside `/root`
        let perm_err = adapter.create_directory("/root/linuxscp_nope").await;
        assert!(matches!(perm_err, Err(AppError::PermissionDenied(_))));

        // 6. Missing path delete_file error
        let missing_err = adapter
            .delete_file(&join_posix_path(&test_root, "does_not_exist.txt"))
            .await;
        assert!(matches!(missing_err, Err(AppError::Filesystem(_))));

        // 7. Refuse root `/` deletion
        let root_del_err = adapter.delete_directory("/", true).await;
        assert!(matches!(root_del_err, Err(AppError::Validation(_))));

        // 8. Capability check blocks unsupported operations
        let mut restricted_caps = ProtocolCapabilities::for_sftp();
        restricted_caps.can_create_file = false;
        let restricted_adapter =
            SftpFileSystemAdapter::new(Arc::clone(&conn.sftp_session), restricted_caps, timeout);
        let cap_err = restricted_adapter
            .create_file(&join_posix_path(&test_root, "blocked.txt"))
            .await;
        assert!(matches!(cap_err, Err(AppError::Protocol(_))));

        // 9. Recursive delete_directory(true) cleans up nested folder + file
        let nested_dir = join_posix_path(&test_root, "nested");
        adapter.create_directory(&nested_dir).await.unwrap();
        adapter
            .create_file(&join_posix_path(&nested_dir, "inner.txt"))
            .await
            .unwrap();
        adapter
            .delete_directory(&test_root, true)
            .await
            .expect("Recursive delete_directory should succeed");

        conn.close().await.expect("Should close cleanly");
    }

    #[test]
    fn test_parse_octal_mode_and_special_bits() {
        let p755 = parse_octal_mode("755").expect("755 valid");
        assert_eq!(p755.to_octal_number(), 0o0755);
        assert_eq!(p755.to_octal_string(), "0755");
        assert_eq!(p755.to_symbolic_string(), "rwxr-xr-x");

        let p4755 = parse_octal_mode("4755").expect("4755 valid");
        assert!(p4755.set_uid);
        assert!(!p4755.set_gid);
        assert!(!p4755.sticky);
        assert_eq!(p4755.to_symbolic_string(), "rwsr-xr-x");

        let p1777 = parse_octal_mode("1777").expect("1777 valid");
        assert!(p1777.sticky);
        assert_eq!(p1777.to_symbolic_string(), "rwxrwxrwt");

        assert!(parse_octal_mode("").is_err());
        assert!(parse_octal_mode("789").is_err());
        assert!(parse_octal_mode("07777").is_err());
        assert!(parse_octal_mode("rwxr-xr-x").is_err());
        assert!(validate_posix_identity_name("ubuntu").is_ok());
        assert!(validate_posix_identity_name("ubuntu; rm -rf /").is_err());
    }

    #[tokio::test]
    async fn test_live_sftp_permissions_and_ownership_errors() {
        let host = "192.168.122.10";
        let port = 22u16;
        if tokio::net::TcpStream::connect((host, port)).await.is_err() {
            return;
        }

        let profile = ConnectionProfile::new(
            "Live SFTP Chmod Test".to_string(),
            ProtocolType::Sftp,
            host.to_string(),
            "ubuntu".to_string(),
            AuthMethod::PublicKey {
                private_key_path: "~/.ssh/id_rsa".to_string(),
                requires_passphrase: false,
            },
        );
        let conn = match establish_sftp_connection(&profile, None).await {
            Ok(Ok(conn)) => conn,
            _ => return,
        };

        let timeout = Duration::from_secs(10);
        let adapter = SftpFileSystemAdapter::new(
            Arc::clone(&conn.sftp_session),
            conn.session_meta.capabilities.clone(),
            timeout,
        );

        let home = conn
            .session_meta
            .home_remote_path
            .as_deref()
            .unwrap_or("/home/ubuntu");
        let test_dir = join_posix_path(
            home,
            &format!("linuxscp_perm_test_{}", uuid::Uuid::new_v4().simple()),
        );
        let test_file = join_posix_path(&test_dir, "perm.txt");

        adapter.create_directory(&test_dir).await.unwrap();
        adapter.create_file(&test_file).await.unwrap();

        // 1. Set permissions (0o640) and verify via stat
        adapter
            .set_permissions(&test_file, UnixPermissions::from_mode(0o640))
            .await
            .expect("chmod 0640 should succeed");
        let meta = adapter.stat(&test_file).await.unwrap();
        assert_eq!(meta.permissions.unwrap().to_octal_number(), 0o640);

        // 2. Recursive chmod on directory (0o750)
        apply_remote_permissions_with_adapter(
            &adapter,
            &conn.sftp_session,
            std::slice::from_ref(&test_dir),
            UnixPermissions::from_mode(0o750),
            true,
            timeout,
        )
        .await
        .expect("Recursive chmod should succeed");
        let child_meta = adapter.stat(&test_file).await.unwrap();
        assert_eq!(child_meta.permissions.unwrap().to_octal_number(), 0o750);

        // 3. Optional getent resolution for file's UID/GID
        let resolved = resolve_remote_identity_via_getent(
            &conn.ssh_handle,
            child_meta.ownership.as_ref().and_then(|o| o.uid),
            child_meta.ownership.as_ref().and_then(|o| o.gid),
        )
        .await;
        assert_eq!(resolved.user_name.as_deref(), Some("ubuntu"));

        // 4. Ownership change to uid=0 (root) as normal user must be rejected accurately
        let chown_res = adapter
            .set_owner_group(
                &test_file,
                FileOwnership {
                    uid: Some(0),
                    gid: None,
                    user_name: None,
                    group_name: None,
                },
            )
            .await;
        assert!(
            matches!(chown_res, Err(AppError::PermissionDenied(_))),
            "Expected PermissionDenied for chown to root as non-root user, got: {:?}",
            chown_res
        );

        adapter.delete_directory(&test_dir, true).await.unwrap();
        conn.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_live_sftp_sudo_elevation_unconfigured_fails_safely_with_guidance() {
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

        // Configure a profile with sudo elevation on the real test VM where NOPASSWD is NOT configured
        let mut profile = ConnectionProfile::new(
            "Ubuntu Test VM Sudo".to_string(),
            ProtocolType::Sftp,
            test_host.to_string(),
            "ubuntu".to_string(),
            AuthMethod::PublicKey {
                private_key_path: "~/.ssh/id_rsa".to_string(),
                requires_passphrase: false,
            },
        );
        profile.port = test_port;
        profile.connection_settings.timeout_seconds = 5;
        profile.sudo_sftp.enabled = true;
        profile.sudo_sftp.sftp_server_path = Some("/usr/lib/openssh/sftp-server".to_string());

        let result = establish_sftp_connection(&profile, None).await;
        let outcome = match result {
            Ok(Err(ConnectOutcome::HostKeyUnknown {
                algorithm,
                fingerprint_sha256,
                public_key_openssh,
                ..
            })) => {
                let db = Database::open_default().expect("Should open DB");
                let kh_repo = KnownHostsRepository::new(&db);
                kh_repo
                    .trust_unknown_host_key(
                        test_host,
                        test_port,
                        &algorithm,
                        &fingerprint_sha256,
                        &public_key_openssh,
                    )
                    .expect("Should trust host key");
                establish_sftp_connection(&profile, None).await
            }
            other => other,
        };

        // Connecting with sudo-sftp when sudo requires password must fail safely with actionable guidance
        let err = match outcome {
            Err(e) => e,
            Ok(Ok(_)) => {
                panic!("Expected connection failure when sudoers NOPASSWD is not configured")
            }
            Ok(Err(challenge)) => {
                panic!(
                    "Unexpected challenge after trusting host key: {:?}",
                    challenge
                )
            }
        };
        match err {
            AppError::Protocol(msg) => {
                assert!(
                    msg.contains("NOPASSWD")
                        || msg.contains("sudoers")
                        || msg.contains("handshake"),
                    "Error message should provide clear sudoers NOPASSWD guidance: {}",
                    msg
                );
            }
            other => panic!("Expected AppError::Protocol, got: {:?}", other),
        }

        // Clean up trusted entry after test so test_live_sftp_connection_and_host_key_verification_flow still starts with unknown
        let db = Database::open_default().expect("Should open DB");
        let kh_repo = KnownHostsRepository::new(&db);
        let _ = kh_repo.delete_known_host(test_host, test_port);
    }
}
