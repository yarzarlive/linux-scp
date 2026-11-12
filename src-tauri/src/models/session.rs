use super::capabilities::ProtocolCapabilities;
use super::connection::ProtocolType;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Stable identifier for an active connection session.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub String);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Lifecycle status of a connection session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "details")]
pub enum SessionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed { error_message: String },
}

/// Elevation status for the remote session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ElevationState {
    /// Normal unprivileged user session.
    None,
    /// Elevated SFTP session executed via sudo on separate channel.
    ElevatedSudo {
        effective_user: String,
        executed_command: String,
    },
}

impl ElevationState {
    pub fn is_elevated(&self) -> bool {
        matches!(self, Self::ElevatedSudo { .. })
    }
}

/// Snapshot of an active or saved connection session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionSession {
    /// Unique session handle.
    pub session_id: SessionId,
    /// Associated profile ID, if connected from a saved profile.
    pub profile_id: Option<String>,
    /// Protocol in use.
    pub protocol: ProtocolType,
    /// Hostname or IP.
    pub host: String,
    /// Port.
    pub port: u16,
    /// Authenticated SSH username.
    pub username: String,
    /// Active connection status.
    pub status: SessionStatus,
    /// Privilege elevation status.
    pub elevation: ElevationState,
    /// Current remote working directory.
    pub current_remote_path: String,
    /// Protocol capabilities for this session.
    pub capabilities: ProtocolCapabilities,
    /// Last activity timestamp (Unix ms).
    pub last_active_ms: u64,
}

impl ConnectionSession {
    pub fn new(
        profile_id: Option<String>,
        protocol: ProtocolType,
        host: String,
        port: u16,
        username: String,
        initial_remote_path: Option<String>,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Self {
            session_id: SessionId::new(),
            profile_id,
            protocol,
            host,
            port,
            username,
            status: SessionStatus::Connecting,
            elevation: ElevationState::None,
            current_remote_path: initial_remote_path.unwrap_or_else(|| "/".to_string()),
            capabilities: ProtocolCapabilities::from_protocol(protocol),
            last_active_ms: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let session = ConnectionSession::new(
            Some("prof-123".to_string()),
            ProtocolType::Sftp,
            "192.168.122.10".to_string(),
            22,
            "ubuntu".to_string(),
            Some("/var/www".to_string()),
        );

        assert_eq!(session.current_remote_path, "/var/www");
        assert_eq!(session.protocol, ProtocolType::Sftp);
        assert!(!session.elevation.is_elevated());
        assert!(session.capabilities.can_elevate_sudo);
    }
}
