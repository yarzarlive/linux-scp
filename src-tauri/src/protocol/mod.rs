use crate::errors::AppError;
use crate::models::{
    FileEntry, FileMetadata, FileOwnership, ProtocolCapabilities, UnixPermissions,
};
use async_trait::async_trait;

/// Unified abstraction contract for all remote filesystem protocols (SFTP, FTP/FTPS, SCP).
/// Domain services interact exclusively with this trait, decoupling application logic from
/// specific underlying protocol crates or C libraries.
#[async_trait]
pub trait RemoteFileSystem: Send + Sync {
    /// Returns the active capability matrix supported by this backend instance.
    fn capabilities(&self) -> &ProtocolCapabilities;

    /// Connects the underlying transport and performs initial handshakes.
    async fn connect(&mut self) -> Result<(), AppError>;

    /// Gracefully closes protocol sessions and transport channels.
    async fn disconnect(&mut self) -> Result<(), AppError>;

    /// Lists entries in a directory.
    async fn list_directory(&self, path: &str) -> Result<Vec<FileEntry>, AppError> {
        self.capabilities()
            .verify_operation("list_directory")
            .map_err(|e| AppError::Protocol(e.to_string()))?;
        self.do_list_directory(path).await
    }

    /// Retrieves file attributes and permissions.
    async fn stat(&self, path: &str) -> Result<FileMetadata, AppError> {
        self.capabilities()
            .verify_operation("stat_metadata")
            .map_err(|e| AppError::Protocol(e.to_string()))?;
        self.do_stat(path).await
    }

    /// Creates an empty remote file.
    async fn create_file(&self, path: &str) -> Result<(), AppError> {
        self.capabilities()
            .verify_operation("create_file")
            .map_err(|e| AppError::Protocol(e.to_string()))?;
        self.do_create_file(path).await
    }

    /// Creates a remote directory.
    async fn create_directory(&self, path: &str) -> Result<(), AppError> {
        self.capabilities()
            .verify_operation("create_directory")
            .map_err(|e| AppError::Protocol(e.to_string()))?;
        self.do_create_directory(path).await
    }

    /// Renames or moves a remote item.
    async fn rename(&self, old_path: &str, new_path: &str) -> Result<(), AppError> {
        self.capabilities()
            .verify_operation("rename")
            .map_err(|e| AppError::Protocol(e.to_string()))?;
        self.do_rename(old_path, new_path).await
    }

    /// Deletes a single remote file.
    async fn delete_file(&self, path: &str) -> Result<(), AppError> {
        self.capabilities()
            .verify_operation("delete_file")
            .map_err(|e| AppError::Protocol(e.to_string()))?;
        self.do_delete_file(path).await
    }

    /// Deletes a remote directory.
    async fn delete_directory(&self, path: &str, recursive: bool) -> Result<(), AppError> {
        self.capabilities()
            .verify_operation("delete_directory")
            .map_err(|e| AppError::Protocol(e.to_string()))?;
        self.do_delete_directory(path, recursive).await
    }

    /// Sets Unix permissions (chmod).
    async fn set_permissions(
        &self,
        path: &str,
        permissions: UnixPermissions,
    ) -> Result<(), AppError> {
        self.capabilities()
            .verify_operation("set_permissions")
            .map_err(|e| AppError::Protocol(e.to_string()))?;
        self.do_set_permissions(path, permissions).await
    }

    /// Sets remote ownership (chown).
    async fn set_owner_group(&self, path: &str, ownership: FileOwnership) -> Result<(), AppError> {
        self.capabilities()
            .verify_operation("set_owner_group")
            .map_err(|e| AppError::Protocol(e.to_string()))?;
        self.do_set_owner_group(path, ownership).await
    }

    // Concrete protocol backends implement these private methods:
    async fn do_list_directory(&self, path: &str) -> Result<Vec<FileEntry>, AppError>;
    async fn do_stat(&self, path: &str) -> Result<FileMetadata, AppError>;
    async fn do_create_file(&self, path: &str) -> Result<(), AppError>;
    async fn do_create_directory(&self, path: &str) -> Result<(), AppError>;
    async fn do_rename(&self, old_path: &str, new_path: &str) -> Result<(), AppError>;
    async fn do_delete_file(&self, path: &str) -> Result<(), AppError>;
    async fn do_delete_directory(&self, path: &str, recursive: bool) -> Result<(), AppError>;
    async fn do_set_permissions(
        &self,
        path: &str,
        permissions: UnixPermissions,
    ) -> Result<(), AppError>;
    async fn do_set_owner_group(
        &self,
        path: &str,
        ownership: FileOwnership,
    ) -> Result<(), AppError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test mock backend for SCP (which does NOT support directory listing or chmod)
    struct MockScpBackend {
        caps: ProtocolCapabilities,
    }

    impl MockScpBackend {
        fn new() -> Self {
            Self {
                caps: ProtocolCapabilities::for_scp(),
            }
        }
    }

    #[async_trait]
    impl RemoteFileSystem for MockScpBackend {
        fn capabilities(&self) -> &ProtocolCapabilities {
            &self.caps
        }

        async fn connect(&mut self) -> Result<(), AppError> {
            Ok(())
        }
        async fn disconnect(&mut self) -> Result<(), AppError> {
            Ok(())
        }

        async fn do_list_directory(&self, _path: &str) -> Result<Vec<FileEntry>, AppError> {
            Ok(vec![])
        }

        async fn do_stat(&self, _path: &str) -> Result<FileMetadata, AppError> {
            Ok(FileMetadata {
                size: None,
                modified_at_secs: None,
                accessed_at_secs: None,
                permissions: None,
                ownership: None,
            })
        }

        async fn do_create_file(&self, _path: &str) -> Result<(), AppError> {
            Ok(())
        }
        async fn do_create_directory(&self, _path: &str) -> Result<(), AppError> {
            Ok(())
        }
        async fn do_rename(&self, _old: &str, _new: &str) -> Result<(), AppError> {
            Ok(())
        }
        async fn do_delete_file(&self, _path: &str) -> Result<(), AppError> {
            Ok(())
        }
        async fn do_delete_directory(&self, _path: &str, _rec: bool) -> Result<(), AppError> {
            Ok(())
        }
        async fn do_set_permissions(
            &self,
            _path: &str,
            _p: UnixPermissions,
        ) -> Result<(), AppError> {
            Ok(())
        }
        async fn do_set_owner_group(&self, _path: &str, _o: FileOwnership) -> Result<(), AppError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_remote_filesystem_enforces_capability_boundaries() {
        let scp_backend = MockScpBackend::new();

        // Listing on SCP must be blocked by capability check with explicit error
        let list_res = scp_backend.list_directory("/var/www").await;
        match list_res {
            Err(AppError::Protocol(msg)) => {
                assert!(msg.contains("unsupported by active protocol Scp"));
                assert!(msg.contains("list_directory"));
            }
            other => panic!("Expected Protocol unsupported error, got {:?}", other),
        }

        // Setting permissions on SCP must also be blocked
        let perm_res = scp_backend
            .set_permissions("/var/www/file.txt", UnixPermissions::from_mode(0o644))
            .await;
        match perm_res {
            Err(AppError::Protocol(msg)) => {
                assert!(msg.contains("set_permissions"));
            }
            other => panic!("Expected Protocol unsupported error, got {:?}", other),
        }
    }
}
