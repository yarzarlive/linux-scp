use serde::{Deserialize, Serialize};

/// Type classification for a file entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "target")]
pub enum FileType {
    Regular,
    Directory,
    Symlink { target: Option<String> },
    Special,
}

impl FileType {
    pub fn is_dir(&self) -> bool {
        matches!(self, Self::Directory)
    }

    pub fn is_symlink(&self) -> bool {
        matches!(self, Self::Symlink { .. })
    }
}

/// Structured Unix permission mode bits.
/// Models standard rwx triplets for User, Group, Others, plus special SUID/SGID/Sticky bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnixPermissions {
    /// Full raw mode bits (e.g. `0o100755` or `0o755`).
    pub raw_mode: u32,
    pub user_read: bool,
    pub user_write: bool,
    pub user_execute: bool,
    pub group_read: bool,
    pub group_write: bool,
    pub group_execute: bool,
    pub other_read: bool,
    pub other_write: bool,
    pub other_execute: bool,
    pub set_uid: bool,
    pub set_gid: bool,
    pub sticky: bool,
}

impl UnixPermissions {
    /// Constructs structured permissions from standard Unix mode integer.
    pub fn from_mode(mode: u32) -> Self {
        Self {
            raw_mode: mode,
            user_read: (mode & 0o400) != 0,
            user_write: (mode & 0o200) != 0,
            user_execute: (mode & 0o100) != 0,
            group_read: (mode & 0o040) != 0,
            group_write: (mode & 0o020) != 0,
            group_execute: (mode & 0o010) != 0,
            other_read: (mode & 0o004) != 0,
            other_write: (mode & 0o002) != 0,
            other_execute: (mode & 0o001) != 0,
            set_uid: (mode & 0o4000) != 0,
            set_gid: (mode & 0o2000) != 0,
            sticky: (mode & 0o1000) != 0,
        }
    }

    /// Converts booleans back to 9-bit/12-bit octal mode number.
    pub fn to_octal_number(&self) -> u32 {
        let mut mode = 0;
        if self.set_uid {
            mode |= 0o4000;
        }
        if self.set_gid {
            mode |= 0o2000;
        }
        if self.sticky {
            mode |= 0o1000;
        }

        if self.user_read {
            mode |= 0o400;
        }
        if self.user_write {
            mode |= 0o200;
        }
        if self.user_execute {
            mode |= 0o100;
        }

        if self.group_read {
            mode |= 0o040;
        }
        if self.group_write {
            mode |= 0o020;
        }
        if self.group_execute {
            mode |= 0o010;
        }

        if self.other_read {
            mode |= 0o004;
        }
        if self.other_write {
            mode |= 0o002;
        }
        if self.other_execute {
            mode |= 0o001;
        }

        mode
    }

    /// Formats as 3 or 4-digit octal string (e.g. "0755" or "0644").
    pub fn to_octal_string(&self) -> String {
        format!("{:04o}", self.to_octal_number())
    }

    /// Formats as standard Unix 9-character symbolic string (e.g., `rwxr-xr-x`).
    pub fn to_symbolic_string(&self) -> String {
        let mut s = String::with_capacity(9);

        // User
        s.push(if self.user_read { 'r' } else { '-' });
        s.push(if self.user_write { 'w' } else { '-' });
        s.push(match (self.user_execute, self.set_uid) {
            (true, true) => 's',
            (false, true) => 'S',
            (true, false) => 'x',
            (false, false) => '-',
        });

        // Group
        s.push(if self.group_read { 'r' } else { '-' });
        s.push(if self.group_write { 'w' } else { '-' });
        s.push(match (self.group_execute, self.set_gid) {
            (true, true) => 's',
            (false, true) => 'S',
            (true, false) => 'x',
            (false, false) => '-',
        });

        // Others
        s.push(if self.other_read { 'r' } else { '-' });
        s.push(if self.other_write { 'w' } else { '-' });
        s.push(match (self.other_execute, self.sticky) {
            (true, true) => 't',
            (false, true) => 'T',
            (true, false) => 'x',
            (false, false) => '-',
        });

        s
    }
}

/// File ownership information.
/// Distinguishes between resolved names and raw numeric IDs without fabricating data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileOwnership {
    /// Numeric User ID.
    pub uid: Option<u32>,
    /// Numeric Group ID.
    pub gid: Option<u32>,
    /// Human-readable username if resolvable.
    pub user_name: Option<String>,
    /// Human-readable group name if resolvable.
    pub group_name: Option<String>,
}

impl FileOwnership {
    /// Display label for owner column (prefers resolved name, falls back to numeric UID).
    pub fn owner_display(&self) -> Option<String> {
        self.user_name
            .clone()
            .or_else(|| self.uid.map(|id| id.to_string()))
    }

    /// Display label for group (prefers resolved name, falls back to numeric GID).
    pub fn group_display(&self) -> Option<String> {
        self.group_name
            .clone()
            .or_else(|| self.gid.map(|id| id.to_string()))
    }
}

/// Metadata attributes for a filesystem entry.
/// Explicitly distinguishes between unknown metadata and 0 values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMetadata {
    /// File size in bytes. `None` if unknown (e.g. certain virtual directories or streaming pipes).
    pub size: Option<u64>,
    /// Last modification timestamp in seconds since Unix epoch. `None` if unknown.
    pub modified_at_secs: Option<u64>,
    /// Last access timestamp in seconds since Unix epoch.
    pub accessed_at_secs: Option<u64>,
    /// Unix file permissions. `None` if unavailable on target filesystem/protocol.
    pub permissions: Option<UnixPermissions>,
    /// Ownership details. `None` if unavailable.
    pub ownership: Option<FileOwnership>,
}

/// Unified entry model representing a file, folder, or link in local or remote panes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Base name of file or directory (e.g. `index.html`).
    pub name: String,
    /// Absolute path to entry (e.g. `/var/www/html/index.html`).
    pub path: String,
    /// Classification (regular file, directory, symlink).
    pub file_type: FileType,
    /// File attributes and permissions.
    pub metadata: FileMetadata,
    /// Indicates whether entry starts with `.` or is flagged hidden.
    pub is_hidden: bool,
}

impl FileEntry {
    pub fn new(name: String, path: String, file_type: FileType, metadata: FileMetadata) -> Self {
        let is_hidden = name.starts_with('.') && name != "." && name != "..";
        Self {
            name,
            path,
            file_type,
            metadata,
            is_hidden,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unix_permissions_standard_755() {
        let p = UnixPermissions::from_mode(0o755);
        assert!(p.user_read && p.user_write && p.user_execute);
        assert!(p.group_read && !p.group_write && p.group_execute);
        assert!(p.other_read && !p.other_write && p.other_execute);
        assert!(!p.set_uid && !p.set_gid && !p.sticky);
        assert_eq!(p.to_symbolic_string(), "rwxr-xr-x");
        assert_eq!(p.to_octal_string(), "0755");
    }

    #[test]
    fn test_unix_permissions_special_bits() {
        // SUID 4755 -> rwsr-xr-x
        let p1 = UnixPermissions::from_mode(0o4755);
        assert!(p1.set_uid);
        assert_eq!(p1.to_symbolic_string(), "rwsr-xr-x");

        // SGID 2755 -> rwxr-sr-x
        let p2 = UnixPermissions::from_mode(0o2755);
        assert!(p2.set_gid);
        assert_eq!(p2.to_symbolic_string(), "rwxr-sr-x");

        // Sticky 1777 -> rwxrwxrwt
        let p3 = UnixPermissions::from_mode(0o1777);
        assert!(p3.sticky);
        assert_eq!(p3.to_symbolic_string(), "rwxrwxrwt");
    }

    #[test]
    fn test_distinguishes_unknown_metadata_from_zero() {
        let meta = FileMetadata {
            size: None, // Unknown size is NOT 0 bytes
            modified_at_secs: None,
            accessed_at_secs: None,
            permissions: None,
            ownership: None,
        };
        assert_eq!(meta.size, None);
        assert_ne!(meta.size, Some(0));
    }

    #[test]
    fn test_ownership_fallbacks() {
        let o1 = FileOwnership {
            uid: Some(1000),
            gid: Some(1000),
            user_name: Some("ubuntu".to_string()),
            group_name: Some("ubuntu".to_string()),
        };
        assert_eq!(o1.owner_display(), Some("ubuntu".to_string()));

        let o2 = FileOwnership {
            uid: Some(1001),
            gid: Some(1001),
            user_name: None, // Unresolved name
            group_name: None,
        };
        assert_eq!(o2.owner_display(), Some("1001".to_string()));
    }
}
