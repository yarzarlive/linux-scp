use crate::errors::AppError;
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

/// Connection timeouts and keepalive options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileConnectionSettings {
    pub timeout_seconds: u32,
    pub keepalive_interval_seconds: Option<u32>,
}

impl Default for ProfileConnectionSettings {
    fn default() -> Self {
        Self {
            timeout_seconds: 30,
            keepalive_interval_seconds: Some(15),
        }
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
    /// Connection timeouts and keepalive interval.
    pub connection_settings: ProfileConnectionSettings,
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
            connection_settings: ProfileConnectionSettings::default(),
            created_at_ms: now,
            updated_at_ms: now,
        }
    }

    /// Validates profile constraints according to protocol capabilities and specification rules.
    pub fn validate(&self) -> Result<(), AppError> {
        let trimmed_name = self.name.trim();
        if trimmed_name.is_empty() {
            return Err(AppError::Validation(
                "Profile name cannot be empty".to_string(),
            ));
        }
        if trimmed_name.len() > 100 {
            return Err(AppError::Validation(
                "Profile name cannot exceed 100 characters".to_string(),
            ));
        }

        let trimmed_host = self.host.trim();
        if trimmed_host.is_empty() {
            return Err(AppError::Validation(
                "Hostname or IP address cannot be empty".to_string(),
            ));
        }
        if trimmed_host.contains("://") || trimmed_host.contains(' ') {
            return Err(AppError::Validation(
                "Hostname must be a plain host or IP without URI scheme or spaces".to_string(),
            ));
        }

        if self.port == 0 {
            return Err(AppError::Validation(
                "Port must be between 1 and 65535".to_string(),
            ));
        }

        let trimmed_user = self.username.trim();
        if trimmed_user.is_empty() {
            return Err(AppError::Validation("Username cannot be empty".to_string()));
        }

        // Protocol-specific constraints
        if (self.protocol == ProtocolType::Scp || self.protocol == ProtocolType::Ftp)
            && self.sudo_sftp.enabled
        {
            return Err(AppError::Validation(format!(
                "{} protocol does not support sudo-SFTP elevation",
                self.protocol.as_str()
            )));
        }

        if let AuthMethod::PublicKey {
            ref private_key_path,
            ..
        } = self.auth_method
        {
            if private_key_path.trim().is_empty() {
                return Err(AppError::Validation(
                    "Private key file path cannot be empty for public key authentication"
                        .to_string(),
                ));
            }
        }

        Ok(())
    }
}

/// Lightweight summary for profile listing in Site Manager sidebar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionProfileSummary {
    pub id: String,
    pub name: String,
    pub protocol: ProtocolType,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub has_saved_secret: bool,
    pub updated_at_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_ports() {
        assert_eq!(ProtocolType::Sftp.default_port(), 22);
        assert_eq!(ProtocolType::Scp.default_port(), 22);
        assert_eq!(ProtocolType::Ftp.default_port(), 21);
        assert_eq!(ProtocolType::Ftps.default_port(), 990);
    }

    #[test]
    fn test_sudo_sftp_command_resolution() {
        let default_config = SudoSftpConfig {
            enabled: true,
            custom_command: None,
        };
        assert_eq!(
            default_config.effective_command(),
            SudoSftpConfig::DEFAULT_SFTP_SERVER_CMD
        );

        let custom_config = SudoSftpConfig {
            enabled: true,
            custom_command: Some("sudo -n /opt/custom/sftp-server".to_string()),
        };
        assert_eq!(
            custom_config.effective_command(),
            "sudo -n /opt/custom/sftp-server"
        );
    }

    #[test]
    fn test_profile_serialization_contains_no_secrets() {
        let profile = ConnectionProfile::new(
            "Production SFTP".to_string(),
            ProtocolType::Sftp,
            "192.168.1.100".to_string(),
            "deploy".to_string(),
            AuthMethod::Password,
        );

        let json = serde_json::to_string(&profile).expect("Failed to serialize profile");
        assert!(!json.contains("password"));
        assert!(!json.contains("secret"));
    }

    #[test]
    fn test_profile_validation_success() {
        let profile = ConnectionProfile::new(
            "Valid Server".to_string(),
            ProtocolType::Sftp,
            "192.168.1.50".to_string(),
            "alice".to_string(),
            AuthMethod::Password,
        );
        assert!(profile.validate().is_ok());
    }

    #[test]
    fn test_profile_validation_rejects_empty_fields() {
        let mut profile = ConnectionProfile::new(
            "".to_string(),
            ProtocolType::Sftp,
            "192.168.1.50".to_string(),
            "alice".to_string(),
            AuthMethod::Password,
        );
        assert!(profile.validate().is_err());

        profile.name = "Server".to_string();
        profile.host = "".to_string();
        assert!(profile.validate().is_err());

        profile.host = "192.168.1.50".to_string();
        profile.username = "".to_string();
        assert!(profile.validate().is_err());
    }

    #[test]
    fn test_profile_validation_rejects_scheme_in_host() {
        let profile = ConnectionProfile::new(
            "Server".to_string(),
            ProtocolType::Sftp,
            "sftp://192.168.1.50".to_string(),
            "alice".to_string(),
            AuthMethod::Password,
        );
        assert!(profile.validate().is_err());
    }

    #[test]
    fn test_profile_validation_rejects_sudo_on_unsupported_protocols() {
        let mut profile = ConnectionProfile::new(
            "SCP Server".to_string(),
            ProtocolType::Scp,
            "192.168.1.50".to_string(),
            "alice".to_string(),
            AuthMethod::Password,
        );
        profile.sudo_sftp.enabled = true;
        assert!(profile.validate().is_err());

        profile.protocol = ProtocolType::Ftp;
        assert!(profile.validate().is_err());

        profile.protocol = ProtocolType::Sftp;
        assert!(profile.validate().is_ok());
    }

    #[test]
    fn test_profile_validation_rejects_empty_private_key_path() {
        let profile = ConnectionProfile::new(
            "SSH Key Server".to_string(),
            ProtocolType::Sftp,
            "192.168.1.50".to_string(),
            "alice".to_string(),
            AuthMethod::PublicKey {
                private_key_path: "".to_string(),
                requires_passphrase: false,
            },
        );
        assert!(profile.validate().is_err());
    }
}
