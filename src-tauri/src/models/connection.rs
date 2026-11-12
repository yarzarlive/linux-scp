use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Supported file-transfer protocol variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ProtocolType {
    Sftp,
    Ftp,
    Ftps,
    Scp,
}

impl ProtocolType {
    /// Standard default TCP port for each protocol.
    pub fn default_port(&self) -> u16 {
        match self {
            Self::Sftp => 22,
            Self::Ftp => 21,
            Self::Ftps => 990,
            Self::Scp => 22,
        }
    }

    /// Display string representing protocol name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sftp => "SFTP",
            Self::Ftp => "FTP",
            Self::Ftps => "FTPS",
            Self::Scp => "SCP",
        }
    }
}

/// Authentication method for connection profile.
/// Strictly excludes password/passphrase secret payloads (handled exclusively via Secret Service).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "details")]
pub enum AuthMethod {
    /// Password authentication (password stored in Secret Service keyring).
    Password,
    /// Public key authentication with private key file path.
    PublicKey {
        private_key_path: String,
        /// Indicates if private key is passphrase protected (passphrase stored in Secret Service).
        requires_passphrase: bool,
    },
    /// SSH Agent authentication via SSH_AUTH_SOCK.
    Agent,
    /// Interactive keyboard-interactive authentication.
    Interactive,
}

/// Sudo-enabled external SFTP server configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SudoSftpConfig {
    /// Whether sudo SFTP elevation is enabled for this connection.
    pub enabled: bool,
    /// Custom remote command to execute through sudo (e.g., `sudo -n /usr/lib/openssh/sftp-server`).
    pub custom_command: Option<String>,
}

impl SudoSftpConfig {
    pub const DEFAULT_SFTP_SERVER_CMD: &'static str = "sudo -n /usr/lib/openssh/sftp-server";

    /// Resolves the actual command to execute for sudo SFTP.
    pub fn effective_command(&self) -> &str {
        self.custom_command
            .as_deref()
            .unwrap_or(Self::DEFAULT_SFTP_SERVER_CMD)
    }
}

/// Default transfer policies associated with a connection profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileTransferSettings {
    pub preserve_timestamps: bool,
    pub preserve_permissions: bool,
    pub concurrency_limit: usize,
}

impl Default for ProfileTransferSettings {
    fn default() -> Self {
        Self {
            preserve_timestamps: true,
            preserve_permissions: true,
            concurrency_limit: 2,
        }
    }
}

/// Persistent connection profile metadata.
/// Security invariant: Contains ZERO passwords, passphrases, tokens, or private key bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionProfile {
    /// Stable unique identifier.
    pub id: String,
    /// User-friendly label for connection.
    pub name: String,
    /// Target protocol.
    pub protocol: ProtocolType,
    /// Remote hostname or IPv4/IPv6 address.
    pub host: String,
    /// Remote port.
    pub port: u16,
    /// Login username.
    pub username: String,
    /// Non-secret authentication configuration.
    pub auth_method: AuthMethod,
    /// Initial directory to navigate upon local pane load.
    pub initial_local_directory: Option<String>,
    /// Initial directory to navigate upon remote pane connect.
    pub initial_remote_directory: Option<String>,
    /// Sudo-enabled SFTP session settings.
    pub sudo_sftp: SudoSftpConfig,
    /// Transfer behavior defaults.
    pub transfer_settings: ProfileTransferSettings,
    /// Unix timestamp (milliseconds) of creation.
    pub created_at_ms: u64,
    /// Unix timestamp (milliseconds) of last update.
    pub updated_at_ms: u64,
}

impl ConnectionProfile {
    pub fn new(
        name: String,
        protocol: ProtocolType,
        host: String,
        username: String,
        auth_method: AuthMethod,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let port = protocol.default_port();

        Self {
            id: Uuid::new_v4().to_string(),
            name,
            protocol,
            host,
            port,
            username,
            auth_method,
            initial_local_directory: None,
            initial_remote_directory: None,
            sudo_sftp: SudoSftpConfig::default(),
            transfer_settings: ProfileTransferSettings::default(),
            created_at_ms: now,
            updated_at_ms: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_ports() {
        assert_eq!(ProtocolType::Sftp.default_port(), 22);
        assert_eq!(ProtocolType::Ftp.default_port(), 21);
        assert_eq!(ProtocolType::Ftps.default_port(), 990);
        assert_eq!(ProtocolType::Scp.default_port(), 22);
    }

    #[test]
    fn test_profile_serialization_contains_no_secrets() {
        let profile = ConnectionProfile::new(
            "Test Server".to_string(),
            ProtocolType::Sftp,
            "192.168.122.10".to_string(),
            "deploy".to_string(),
            AuthMethod::Password,
        );

        let json = serde_json::to_string(&profile).expect("Should serialize profile");
        assert!(json.contains("Test Server"));
        assert!(json.contains("deploy"));
        assert!(json.contains("192.168.122.10"));
        // Confirm no raw credential fields
        assert!(!json.contains("mysecret"));
        assert!(!json.contains("passphrase_secret"));
    }

    #[test]
    fn test_sudo_sftp_command_resolution() {
        let mut cfg = SudoSftpConfig::default();
        assert_eq!(
            cfg.effective_command(),
            "sudo -n /usr/lib/openssh/sftp-server"
        );

        cfg.custom_command = Some("sudo -n /usr/libexec/openssh/sftp-server".to_string());
        assert_eq!(
            cfg.effective_command(),
            "sudo -n /usr/libexec/openssh/sftp-server"
        );
    }
}
