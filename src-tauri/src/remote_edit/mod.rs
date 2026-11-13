//! Managed temporary-copy workflow for opening and safely editing remote files.

pub mod manager;
pub mod sftp_backend;
pub mod types;
pub mod workspace;

pub use manager::{
    BackendError, NoopRemoteEditEventSink, RemoteEditBackend, RemoteEditConfig,
    RemoteEditEventSink, RemoteEditManager, DEFAULT_VERIFY_HASH_MAX_BYTES,
};
pub use sftp_backend::{spawn_background_watcher, SftpRemoteEditBackend, TauriRemoteEditEventSink};
pub use types::{
    compare_fingerprints, FingerprintComparison, RemoteEditSession, RemoteEditState,
    RemoteEditUploadMode, RemoteEditUploadOutcome, RemoteEndpointIdentity, RemoteFileFingerprint,
    RemoteFileStat, RemoteObjectKind, UploadFailureKind,
};
pub use workspace::{
    default_workspace_root, ensure_private_dir, local_stat, read_manifest, remove_session_dir,
    sha256_file, validate_local_file_name, validate_session_id, write_manifest, LocalStat,
    SessionPaths,
};
