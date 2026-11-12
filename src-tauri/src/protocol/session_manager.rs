use crate::errors::AppError;
use crate::models::{ConnectionProfile, ConnectionSession, SessionStatus};
use crate::persistence::KnownHostEntry;
use crate::protocol::sftp::{establish_sftp_connection, ActiveSftpConnection, ConnectOutcome};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;

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
pub struct SessionManager {
    inner: RwLock<InnerSessionState>,
    generation: AtomicU64,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(InnerSessionState {
                status: SessionStatus::Disconnected,
                active: None,
                last_profile: None,
            }),
            generation: AtomicU64::new(0),
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
