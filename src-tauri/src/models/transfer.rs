use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Direction of transfer relative to local client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransferDirection {
    Upload,
    Download,
}

/// Explicit conflict resolution policy when destination file already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConflictPolicy {
    /// Prompt user with conflict dialog displaying source/target metadata.
    #[default]
    Ask,
    /// Overwrite destination unconditionally.
    Overwrite,
    /// Overwrite only if source timestamp is newer than destination.
    OverwriteIfNewer,
    /// Skip transfer and mark completed without error.
    Skip,
    /// Rename new file with numbered suffix (e.g., `file (1).txt`).
    Rename,
}

/// Lifecycle state for an individual file transfer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "details")]
pub enum TransferState {
    /// Queued waiting for available concurrency slot.
    Queued,
    /// Establishing connection or remote file descriptor.
    Connecting,
    /// Active chunked streaming in progress.
    Transferring,
    /// Verifying file integrity / renaming temporary file to final target.
    Verifying,
    /// Successfully finished.
    Completed,
    /// User cancelled mid-transfer.
    Cancelled,
    /// Aborted with actionable failure message.
    Failed { error_message: String },
}

impl TransferState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Cancelled | Self::Failed { .. }
        )
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Connecting | Self::Transferring | Self::Verifying
        )
    }
}

/// Stable unique identifier for a transfer job.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransferId(pub String);

impl TransferId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl Default for TransferId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TransferId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Central transfer record representing a file or directory transfer operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferJob {
    /// Unique identifier.
    pub id: TransferId,
    /// Session identifier that initiated transfer.
    pub session_id: String,
    /// Canonical source path (local path if upload, remote path if download).
    pub source_path: String,
    /// Canonical destination path.
    pub destination_path: String,
    /// Upload or Download.
    pub direction: TransferDirection,
    /// Active lifecycle status.
    pub state: TransferState,
    /// Total expected size in bytes. `None` if unknown (e.g. dynamic stream).
    pub total_bytes: Option<u64>,
    /// Bytes successfully written so far.
    pub transferred_bytes: u64,
    /// Current transfer speed in bytes per second (rolling average).
    pub speed_bps: u64,
    /// Estimated time remaining in seconds.
    pub eta_seconds: Option<u64>,
    /// Conflict resolution rule.
    pub conflict_policy: ConflictPolicy,
    /// Whether this job represents a recursive directory transfer.
    #[serde(default)]
    pub is_directory: bool,
    /// Total number of files in this job (1 for single file, >=0 for directory).
    #[serde(default)]
    pub total_files: u64,
    /// Number of files completed so far.
    #[serde(default)]
    pub completed_files: u64,
    /// Name or relative path of the file currently being transferred.
    #[serde(default)]
    pub current_file: Option<String>,
    /// Creation timestamp (Unix ms).
    pub created_at_ms: u64,
    /// Last update timestamp (Unix ms).
    pub updated_at_ms: u64,
}

/// Throttled progress event payload emitted on `transfer://progress`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferProgressEvent {
    pub job_id: String,
    pub transferred_bytes: u64,
    pub total_bytes: Option<u64>,
    pub speed_bps: u64,
    pub eta_seconds: Option<u64>,
    pub completed_files: u64,
    pub total_files: u64,
    pub current_file: Option<String>,
}

/// Explicit conflict resolution prompt emitted on `transfer://conflict`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferConflictPrompt {
    pub conflict_id: String,
    pub job_id: String,
    pub direction: TransferDirection,
    pub source_path: String,
    pub destination_path: String,
    pub source_size: Option<u64>,
    pub source_modified_at_secs: Option<u64>,
    pub target_size: Option<u64>,
    pub target_modified_at_secs: Option<u64>,
}

/// User decision for a pending name conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictDecision {
    Overwrite,
    OverwriteIfNewer,
    Skip,
    Rename,
    Cancel,
}

impl TransferJob {
    pub fn new(
        session_id: String,
        source_path: String,
        destination_path: String,
        direction: TransferDirection,
        total_bytes: Option<u64>,
        conflict_policy: ConflictPolicy,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Self {
            id: TransferId::new(),
            session_id,
            source_path,
            destination_path,
            direction,
            state: TransferState::Queued,
            total_bytes,
            transferred_bytes: 0,
            speed_bps: 0,
            eta_seconds: None,
            conflict_policy,
            is_directory: false,
            total_files: 1,
            completed_files: 0,
            current_file: None,
            created_at_ms: now,
            updated_at_ms: now,
        }
    }

    /// Calculates progress percentage (0.0 to 100.0) safely without dividing by zero.
    pub fn progress_percentage(&self) -> Option<f32> {
        self.total_bytes.map(|total| {
            if total == 0 {
                100.0
            } else {
                ((self.transferred_bytes as f64 / total as f64) * 100.0) as f32
            }
        })
    }

    /// Updates byte progress and recalculates speed and ETA.
    pub fn update_progress(&mut self, written_bytes: u64, speed_bps: u64) {
        self.transferred_bytes = written_bytes;
        self.speed_bps = speed_bps;
        self.updated_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        if let Some(total) = self.total_bytes {
            if speed_bps > 0 && total >= written_bytes {
                let remaining = total - written_bytes;
                self.eta_seconds = Some(remaining / speed_bps);
            } else {
                self.eta_seconds = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transfer_progress_calculation() {
        let mut job = TransferJob::new(
            "sess-1".to_string(),
            "/home/user/test.bin".to_string(),
            "/remote/test.bin".to_string(),
            TransferDirection::Upload,
            Some(1000),
            ConflictPolicy::Ask,
        );

        assert_eq!(job.progress_percentage(), Some(0.0));
        job.update_progress(500, 100);
        assert_eq!(job.progress_percentage(), Some(50.0));
        assert_eq!(job.eta_seconds, Some(5)); // (1000 - 500) / 100 = 5s
    }

    #[test]
    fn test_unknown_size_handles_none_percentage() {
        let job = TransferJob::new(
            "sess-1".to_string(),
            "/pipe/stream".to_string(),
            "/remote/stream".to_string(),
            TransferDirection::Upload,
            None,
            ConflictPolicy::Ask,
        );
        assert_eq!(job.progress_percentage(), None);
    }
}
