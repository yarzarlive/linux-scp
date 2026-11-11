use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Unified typed error model for all LinuxSCP IPC commands.
#[derive(Debug, Error, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "message")]
pub enum AppError {
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Filesystem error: {0}")]
    Filesystem(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Authentication error: {0}")]
    Authentication(String),

    #[error("Security error: {0}")]
    Security(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Filesystem(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_error_serialization() {
        let err = AppError::Validation("Invalid path".to_string());
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, r#"{"kind":"Validation","message":"Invalid path"}"#);
    }
}
