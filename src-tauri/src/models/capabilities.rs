use super::connection::ProtocolType;
use serde::{Deserialize, Serialize};

/// Granular capability matrix for an active protocol backend or connection session.
/// The UI queries capabilities dynamically to enable/disable UI actions without guessing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolCapabilities {
    pub protocol: ProtocolType,
    pub can_list_directory: bool,
    pub can_stat_metadata: bool,
    pub can_upload: bool,
    pub can_download: bool,
    pub can_create_file: bool,
    pub can_create_directory: bool,
    pub can_rename: bool,
    pub can_delete_file: bool,
    pub can_delete_directory: bool,
    pub can_set_permissions: bool,
    pub can_set_owner_group: bool,
    pub can_elevate_sudo: bool,
    pub can_preserve_timestamps: bool,
    pub can_resume_transfer: bool,
    pub can_symlink: bool,
}

impl ProtocolCapabilities {
    /// Canonical capability profile for SFTP protocol.
    /// SFTP provides rich remote filesystem primitives including chmod, numeric chown, and sudo channel exec.
    pub fn for_sftp() -> Self {
        Self {
            protocol: ProtocolType::Sftp,
            can_list_directory: true,
            can_stat_metadata: true,
            can_upload: true,
            can_download: true,
            can_create_file: true,
            can_create_directory: true,
            can_rename: true,
            can_delete_file: true,
            can_delete_directory: true,
            can_set_permissions: true,
            can_set_owner_group: true,
            can_elevate_sudo: true,
            can_preserve_timestamps: true,
            can_resume_transfer: true,
            can_symlink: true,
        }
    }

    /// Canonical capability profile for FTP / FTPS protocol.
    /// FTP supports transfer and listing, but lacks Unix ownership management and sudo elevation.
    pub fn for_ftp(is_secure: bool) -> Self {
        Self {
            protocol: if is_secure {
                ProtocolType::Ftps
            } else {
                ProtocolType::Ftp
            },
            can_list_directory: true,
            can_stat_metadata: true,
            can_upload: true,
            can_download: true,
            can_create_file: true,
            can_create_directory: true,
            can_rename: true,
            can_delete_file: true,
            can_delete_directory: true,
            // FTP SITE CHMOD may be supported on some servers, but not universally guaranteed
            can_set_permissions: false,
            can_set_owner_group: false,
            // Sudo elevation is strictly an SSH channel concept, impossible over FTP
            can_elevate_sudo: false,
            can_preserve_timestamps: true,
            can_resume_transfer: true,
            can_symlink: false,
        }
    }

    /// Canonical capability profile for SCP protocol.
    /// SCP is transfer-centric without interactive filesystem navigation or permission management.
    pub fn for_scp() -> Self {
        Self {
            protocol: ProtocolType::Scp,
            can_list_directory: false,
            can_stat_metadata: false,
            can_upload: true,
            can_download: true,
            can_create_file: false,
            can_create_directory: false,
            can_rename: false,
            can_delete_file: false,
            can_delete_directory: false,
            can_set_permissions: false,
            can_set_owner_group: false,
            can_elevate_sudo: false,
            can_preserve_timestamps: true,
            can_resume_transfer: false,
            can_symlink: false,
        }
    }

    /// Returns canonical capabilities for given protocol variant.
    pub fn from_protocol(protocol: ProtocolType) -> Self {
        match protocol {
            ProtocolType::Sftp => Self::for_sftp(),
            ProtocolType::Ftp => Self::for_ftp(false),
            ProtocolType::Ftps => Self::for_ftp(true),
            ProtocolType::Scp => Self::for_scp(),
        }
    }

    /// Checks if a named operation is supported, returning an explicit error message if not.
    pub fn verify_operation(&self, operation: &'static str) -> Result<(), CapabilityError> {
        let is_supported = match operation {
            "list_directory" => self.can_list_directory,
            "stat_metadata" => self.can_stat_metadata,
            "upload" => self.can_upload,
            "download" => self.can_download,
            "create_file" => self.can_create_file,
            "create_directory" => self.can_create_directory,
            "rename" => self.can_rename,
            "delete_file" => self.can_delete_file,
            "delete_directory" => self.can_delete_directory,
            "set_permissions" => self.can_set_permissions,
            "set_owner_group" => self.can_set_owner_group,
            "elevate_sudo" => self.can_elevate_sudo,
            "resume_transfer" => self.can_resume_transfer,
            "symlink" => self.can_symlink,
            _ => false,
        };

        if is_supported {
            Ok(())
        } else {
            Err(CapabilityError::UnsupportedOperation {
                protocol: self.protocol,
                operation,
            })
        }
    }
}

/// Structured error raised when an operation is disallowed by protocol capabilities.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[error("Operation '{operation}' is unsupported by active protocol {protocol:?}")]
pub enum CapabilityError {
    UnsupportedOperation {
        protocol: ProtocolType,
        operation: &'static str,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sftp_full_capabilities() {
        let caps = ProtocolCapabilities::for_sftp();
        assert!(caps.can_list_directory);
        assert!(caps.can_set_permissions);
        assert!(caps.can_set_owner_group);
        assert!(caps.can_elevate_sudo);
        assert!(caps.verify_operation("set_permissions").is_ok());
    }

    #[test]
    fn test_ftp_lacks_sudo_elevation() {
        let caps = ProtocolCapabilities::for_ftp(false);
        assert!(!caps.can_elevate_sudo);
        assert!(!caps.can_set_owner_group);
        let err = caps.verify_operation("elevate_sudo").unwrap_err();
        assert_eq!(
            err,
            CapabilityError::UnsupportedOperation {
                protocol: ProtocolType::Ftp,
                operation: "elevate_sudo",
            }
        );
    }

    #[test]
    fn test_scp_transfer_only() {
        let caps = ProtocolCapabilities::for_scp();
        assert!(caps.can_upload);
        assert!(caps.can_download);
        assert!(!caps.can_list_directory);
        assert!(!caps.can_set_permissions);
        let err = caps.verify_operation("list_directory").unwrap_err();
        assert_eq!(
            err,
            CapabilityError::UnsupportedOperation {
                protocol: ProtocolType::Scp,
                operation: "list_directory",
            }
        );
    }
}
