use crate::errors::AppError;
use crate::models::{
    AuthMethod, ConnectionProfile, ConnectionSession, ElevationState, ProtocolType, SessionStatus,
};
use crate::persistence::{
    Database, HostKeyDecision, KeyringService, KnownHostEntry, KnownHostsRepository,
};
use russh::client::{self, Handle, Handler};
use russh::keys::{HashAlg, PrivateKeyWithHashAlg, PublicKeyOrCertificate};
use russh_sftp::client::SftpSession;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

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
    pub sftp_session: SftpSession,
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

    // 3. Open SFTP Subsystem Channel (or elevated sudo-SFTP channel)
    let channel = tokio::time::timeout(timeout_duration, ssh_handle.channel_open_session())
        .await
        .map_err(|_| AppError::Protocol("Timed out opening SSH session channel".to_string()))?
        .map_err(|e| AppError::Protocol(format!("Failed to open SSH session channel: {}", e)))?;

    let elevation_state = if profile.sudo_sftp.enabled {
        let cmd = profile.sudo_sftp.effective_command().to_string();
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
            AppError::Protocol(
                "Timed out initializing SFTP protocol stream (verify remote sftp-server or sudoers NOPASSWD)".to_string(),
            )
        })?
        .map_err(|e| {
            AppError::Protocol(format!(
                "SFTP subsystem initialization failed (if sudo-SFTP is enabled, ensure NOPASSWD is configured for sftp-server): {}",
                e
            ))
        })?;

    // 4. Verify SFTP subsystem readiness by resolving the initial working directory
    let desired_dir = profile
        .initial_remote_directory
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(".");

    let resolved_path = match tokio::time::timeout(
        timeout_duration,
        sftp_session.canonicalize(desired_dir),
    )
    .await
    {
        Ok(Ok(path)) => path,
        _ => tokio::time::timeout(timeout_duration, sftp_session.canonicalize("."))
            .await
            .map_err(|_| {
                AppError::Protocol("Timed out verifying SFTP working directory".to_string())
            })?
            .map_err(|e| {
                AppError::Protocol(format!(
                    "SFTP subsystem failed working directory check: {}",
                    e
                ))
            })?,
    };

    let mut session_meta = ConnectionSession::new(
        Some(profile.id.clone()),
        profile.protocol,
        profile.host.clone(),
        profile.port,
        profile.username.clone(),
        Some(resolved_path),
    );
    session_meta.status = SessionStatus::Connected;
    session_meta.elevation = elevation_state;

    Ok(Ok(ActiveSftpConnection {
        session_meta,
        profile: profile.clone(),
        trusted_host_key,
        ssh_handle,
        sftp_session,
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
}
