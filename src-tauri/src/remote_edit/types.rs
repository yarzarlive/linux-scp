use serde::{Deserialize, Serialize};

/// Identity of the remote endpoint a working copy was downloaded from.
///
/// An edit may only be uploaded back through a session that authenticates to the
/// same host, port, user and elevation mode. This prevents a working copy from being
/// written to an identically-named path on a different server after a reconnect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteEndpointIdentity {
    pub host: String,
    pub port: u16,
    pub username: String,
    /// Whether the session ran through the elevated (sudo) SFTP channel.
    pub elevated: bool,
}

impl RemoteEndpointIdentity {
    /// Hostnames are case-insensitive; everything else must match exactly.
    pub fn matches(&self, other: &RemoteEndpointIdentity) -> bool {
        self.host.eq_ignore_ascii_case(&other.host)
            && self.port == other.port
            && self.username == other.username
            && self.elevated == other.elevated
    }

    pub fn display(&self) -> String {
        let base = format!("{}@{}:{}", self.username, self.host, self.port);
        if self.elevated {
            format!("{} (elevated)", base)
        } else {
            base
        }
    }
}

/// Remote metadata recorded as the version fingerprint of a downloaded file.
///
/// Size and modification time are the primary change indicators. Mode and
/// ownership are recorded so permissions can be restored on upload and for display,
/// but they are not treated as content changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteFileFingerprint {
    pub size: Option<u64>,
    pub modified_at_secs: Option<u64>,
    pub permissions_mode: Option<u32>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
}

/// Kind of remote filesystem object found at a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteObjectKind {
    Regular,
    Directory,
    Other,
}

/// Result of a remote `stat` (following symlinks).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFileStat {
    pub kind: RemoteObjectKind,
    pub fingerprint: RemoteFileFingerprint,
}

/// Outcome of comparing a recorded baseline fingerprint with the current remote one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FingerprintComparison {
    /// Size and modification time are both known and unchanged.
    Same,
    /// Size or modification time is known to differ.
    Changed { reason: String },
    /// The protocol did not report enough metadata to decide.
    Unverifiable { reason: String },
}

/// Compares two fingerprints. Never reports `Same` unless both size and mtime are
/// available on both sides and equal.
pub fn compare_fingerprints(
    baseline: &RemoteFileFingerprint,
    current: &RemoteFileFingerprint,
) -> FingerprintComparison {
    if let (Some(before), Some(now)) = (baseline.size, current.size) {
        if before != now {
            return FingerprintComparison::Changed {
                reason: format!(
                    "Remote file size changed from {} to {} bytes since it was downloaded.",
                    before, now
                ),
            };
        }
    }
    if let (Some(before), Some(now)) = (baseline.modified_at_secs, current.modified_at_secs) {
        if before != now {
            return FingerprintComparison::Changed {
                reason: format!(
                    "Remote modification time changed (was {}, now {}) since it was downloaded.",
                    before, now
                ),
            };
        }
    }
    if baseline.size.is_none()
        || current.size.is_none()
        || baseline.modified_at_secs.is_none()
        || current.modified_at_secs.is_none()
    {
        return FingerprintComparison::Unverifiable {
            reason: "The server did not report size and modification time, so remote changes cannot be ruled out.".to_string(),
        };
    }
    FingerprintComparison::Same
}

/// Why an upload attempt failed. The local working copy is always preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UploadFailureKind {
    /// No active connection, or the connection dropped during the operation.
    Disconnected,
    /// The active connection belongs to a different host/user than the working copy.
    WrongServer,
    /// The server rejected the operation (permissions, disk full, ...).
    RemoteError,
    /// The local working copy could not be read.
    LocalError,
}

/// Lifecycle state of a remote editing session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "details")]
pub enum RemoteEditState {
    /// Local working copy content equals what was last downloaded or uploaded.
    Clean,
    /// Local working copy content differs from the remote baseline; awaiting a user decision.
    Modified,
    /// An upload is in progress.
    Uploading,
    /// The remote file changed (or could not be verified) since the baseline. Upload is
    /// blocked until the user explicitly chooses to overwrite.
    Conflict {
        reason: String,
        remote_current: Option<RemoteFileFingerprint>,
    },
    /// The last upload attempt failed; the local copy is kept.
    UploadFailed {
        message: String,
        kind: UploadFailureKind,
    },
    /// The working copy is missing (deleted, or an application moved it away).
    LocalMissing,
}

/// A tracked remote file opened through a managed local working copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteEditSession {
    pub id: String,
    pub endpoint: RemoteEndpointIdentity,
    pub profile_id: Option<String>,
    /// Canonical (symlink-resolved) remote path of the edited file.
    pub remote_path: String,
    pub file_name: String,
    /// Absolute path of the local working copy handed to the desktop application.
    pub local_path: String,
    pub app_id: Option<String>,
    pub app_name: Option<String>,
    /// Remote fingerprint at download or after the last successful upload.
    pub baseline: RemoteFileFingerprint,
    /// SHA-256 of the content at download or after the last successful upload.
    pub baseline_sha256: String,
    /// SHA-256 of the local working copy when it was last inspected.
    pub local_sha256: Option<String>,
    pub state: RemoteEditState,
    /// User chose "keep local changes" for the current modification; suppresses the
    /// upload prompt until the file changes again.
    pub prompt_dismissed: bool,
    /// True when the local working copy content differs from the remote baseline.
    pub has_unsynced_changes: bool,
    pub created_at_ms: u64,
    pub last_synced_at_ms: u64,
    pub upload_count: u32,
    /// Restored from disk after an application restart.
    #[serde(default)]
    pub recovered: bool,
}

impl RemoteEditSession {
    /// Recomputes derived flags after a hash change.
    pub fn refresh_derived(&mut self) {
        self.has_unsynced_changes = match self.local_sha256.as_deref() {
            Some(local) => local != self.baseline_sha256,
            None => false,
        };
    }

    /// True if discarding this session could lose work that is not on the server.
    pub fn may_lose_local_work(&self) -> bool {
        self.has_unsynced_changes
            || matches!(
                self.state,
                RemoteEditState::Modified
                    | RemoteEditState::Uploading
                    | RemoteEditState::Conflict { .. }
                    | RemoteEditState::UploadFailed { .. }
                    | RemoteEditState::LocalMissing
            )
    }
}

/// Result of an explicit upload request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome")]
pub enum RemoteEditUploadOutcome {
    /// The working copy was uploaded and is the new baseline.
    Uploaded { session: RemoteEditSession },
    /// The working copy is identical to the baseline; nothing was uploaded.
    Unchanged { session: RemoteEditSession },
    /// The remote file changed since download; nothing was uploaded.
    Conflict { session: RemoteEditSession },
    /// The upload failed; the local copy is preserved.
    Failed { session: RemoteEditSession },
}

impl RemoteEditUploadOutcome {
    pub fn session(&self) -> &RemoteEditSession {
        match self {
            Self::Uploaded { session }
            | Self::Unchanged { session }
            | Self::Conflict { session }
            | Self::Failed { session } => session,
        }
    }
}

/// Whether modified remote editing working copies are uploaded automatically once a
/// save settles (provided the remote file has not changed), or held in `Modified` state
/// until the user clicks **Upload Changes**.
///
/// Even in `Auto` mode, `RemoteEditManager::upload(..., false)` always verifies the
/// remote endpoint identity and remote version fingerprint/hash first, so remote changes
/// are never overwritten automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RemoteEditUploadMode {
    #[default]
    Auto,
    Prompt,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fp(size: Option<u64>, mtime: Option<u64>) -> RemoteFileFingerprint {
        RemoteFileFingerprint {
            size,
            modified_at_secs: mtime,
            permissions_mode: Some(0o644),
            uid: Some(1000),
            gid: Some(1000),
        }
    }

    #[test]
    fn fingerprint_same_requires_complete_metadata() {
        assert_eq!(
            compare_fingerprints(&fp(Some(10), Some(5)), &fp(Some(10), Some(5))),
            FingerprintComparison::Same
        );
        assert!(matches!(
            compare_fingerprints(&fp(Some(10), None), &fp(Some(10), None)),
            FingerprintComparison::Unverifiable { .. }
        ));
        assert!(matches!(
            compare_fingerprints(&fp(None, Some(5)), &fp(Some(10), Some(5))),
            FingerprintComparison::Unverifiable { .. }
        ));
    }

    #[test]
    fn fingerprint_detects_size_and_mtime_changes() {
        assert!(matches!(
            compare_fingerprints(&fp(Some(10), Some(5)), &fp(Some(11), Some(5))),
            FingerprintComparison::Changed { .. }
        ));
        assert!(matches!(
            compare_fingerprints(&fp(Some(10), Some(5)), &fp(Some(10), Some(6))),
            FingerprintComparison::Changed { .. }
        ));
        // A known size change is reported even if mtime is unavailable.
        assert!(matches!(
            compare_fingerprints(&fp(Some(10), None), &fp(Some(12), None)),
            FingerprintComparison::Changed { .. }
        ));
    }

    #[test]
    fn mode_and_owner_changes_are_not_content_conflicts() {
        let mut current = fp(Some(10), Some(5));
        current.permissions_mode = Some(0o600);
        current.uid = Some(0);
        assert_eq!(
            compare_fingerprints(&fp(Some(10), Some(5)), &current),
            FingerprintComparison::Same
        );
    }

    #[test]
    fn endpoint_identity_matching() {
        let a = RemoteEndpointIdentity {
            host: "Example.COM".into(),
            port: 22,
            username: "deploy".into(),
            elevated: false,
        };
        let mut b = a.clone();
        b.host = "example.com".into();
        assert!(a.matches(&b));
        b.username = "root".into();
        assert!(!a.matches(&b));
        let mut c = a.clone();
        c.elevated = true;
        assert!(!a.matches(&c));
        let mut d = a.clone();
        d.port = 2222;
        assert!(!a.matches(&d));
    }

    #[test]
    fn upload_outcome_serializes_with_tag() {
        let json = serde_json::to_value(RemoteEditState::UploadFailed {
            message: "x".into(),
            kind: UploadFailureKind::Disconnected,
        })
        .unwrap();
        assert_eq!(json["state"], "UploadFailed");
        assert_eq!(json["details"]["kind"], "Disconnected");
    }
}
