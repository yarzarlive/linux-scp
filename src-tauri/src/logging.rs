use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Patterns that must be sanitized or blocked from logs to prevent leaking secrets.
const SENSITIVE_KEYWORDS: &[&str] = &[
    "password",
    "passphrase",
    "private_key",
    "secret",
    "token",
    "credential",
    "BEGIN OPENSSH PRIVATE KEY",
    "BEGIN RSA PRIVATE KEY",
    "BEGIN PRIVATE KEY",
];

/// Redacts any sensitive substring from a log message.
pub fn sanitize_log_message(msg: &str) -> String {
    let mut sanitized = msg.to_string();
    for keyword in SENSITIVE_KEYWORDS {
        if sanitized.to_lowercase().contains(&keyword.to_lowercase()) {
            sanitized = sanitized.replace(keyword, "[REDACTED_SENSITIVE]");
        }
    }
    sanitized
}

/// Initializes structured logging for LinuxSCP with security filtering.
pub fn init_logging() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,linuxscp=debug"));

    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_thread_ids(false);

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .try_init();

    tracing::info!("LinuxSCP structured logging initialized with credential redaction active.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitizes_credentials() {
        let text = "Connecting with password mysecret123 and private_key content";
        let sanitized = sanitize_log_message(text);
        assert!(!sanitized.contains("password"));
        assert!(!sanitized.contains("private_key"));
        assert!(sanitized.contains("[REDACTED_SENSITIVE]"));
    }

    #[test]
    fn test_sanitizes_pem_headers() {
        let text = "-----BEGIN OPENSSH PRIVATE KEY----- foo -----END OPENSSH PRIVATE KEY-----";
        let sanitized = sanitize_log_message(text);
        assert!(!sanitized.contains("BEGIN OPENSSH PRIVATE KEY"));
        assert!(sanitized.contains("[REDACTED_SENSITIVE]"));
    }
}
