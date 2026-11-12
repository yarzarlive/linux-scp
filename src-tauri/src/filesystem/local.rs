use crate::errors::AppError;
use crate::models::{FileEntry, FileMetadata, FileOwnership, FileType, UnixPermissions};
use serde::{Deserialize, Serialize};
use std::ffi::CStr;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalDirectoryListing {
    pub current_path: String,
    pub parent_path: Option<String>,
    pub entries: Vec<FileEntry>,
}

/// Resolves a numeric UID to a system username via POSIX getpwuid.
fn resolve_uid(uid: u32) -> Option<String> {
    unsafe {
        let pwd = libc::getpwuid(uid);
        if !pwd.is_null() && !(*pwd).pw_name.is_null() {
            CStr::from_ptr((*pwd).pw_name)
                .to_str()
                .ok()
                .map(String::from)
        } else {
            None
        }
    }
}

/// Resolves a numeric GID to a system group name via POSIX getgrgid.
fn resolve_gid(gid: u32) -> Option<String> {
    unsafe {
        let grp = libc::getgrgid(gid);
        if !grp.is_null() && !(*grp).gr_name.is_null() {
            CStr::from_ptr((*grp).gr_name)
                .to_str()
                .ok()
                .map(String::from)
        } else {
            None
        }
    }
}

/// Gets user's home directory path or root fallback.
pub fn get_home_dir() -> String {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "/".to_string())
}

/// Lists files and directories in a local filesystem path.
pub fn list_directory(
    raw_path: &str,
    show_hidden: bool,
) -> Result<LocalDirectoryListing, AppError> {
    let expanded = shellexpand::tilde(raw_path).to_string();
    let path = Path::new(&expanded);

    if !path.exists() {
        return Err(AppError::Filesystem(format!(
            "Path does not exist: {}",
            raw_path
        )));
    }

    let canonical = path
        .canonicalize()
        .map_err(|e| AppError::Filesystem(format!("Failed to resolve canonical path: {}", e)))?;

    if !canonical.is_dir() {
        return Err(AppError::Filesystem(format!(
            "Path is not a directory: {}",
            canonical.display()
        )));
    }

    let current_path = canonical.to_string_lossy().to_string();
    let parent_path = canonical.parent().map(|p| p.to_string_lossy().to_string());

    let mut entries = Vec::new();
    let read_dir = fs::read_dir(&canonical).map_err(|e| {
        AppError::Filesystem(format!("Permission denied accessing directory: {}", e))
    })?;

    for item in read_dir.flatten() {
        let entry_path = item.path();
        let file_name = item.file_name().to_string_lossy().to_string();

        let is_hidden = file_name.starts_with('.') && file_name != "." && file_name != "..";
        if !show_hidden && is_hidden {
            continue;
        }

        // Use symlink_metadata to avoid following broken links blindly
        let metadata = match fs::symlink_metadata(&entry_path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let file_type = if metadata.file_type().is_symlink() {
            let target = fs::read_link(&entry_path)
                .ok()
                .map(|p| p.to_string_lossy().to_string());
            FileType::Symlink { target }
        } else if metadata.is_dir() {
            FileType::Directory
        } else {
            FileType::Regular
        };

        let raw_mode = metadata.mode();
        let permissions = Some(UnixPermissions::from_mode(raw_mode));

        let uid = metadata.uid();
        let gid = metadata.gid();
        let ownership = Some(FileOwnership {
            uid: Some(uid),
            gid: Some(gid),
            user_name: resolve_uid(uid),
            group_name: resolve_gid(gid),
        });

        let size = if file_type.is_dir() {
            None // Directories show "—" per specification
        } else {
            Some(metadata.len())
        };

        let modified_at_secs = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs());

        let accessed_at_secs = metadata
            .accessed()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs());

        let entry = FileEntry::new(
            file_name,
            entry_path.to_string_lossy().to_string(),
            file_type,
            FileMetadata {
                size,
                modified_at_secs,
                accessed_at_secs,
                permissions,
                ownership,
            },
        );

        entries.push(entry);
    }

    Ok(LocalDirectoryListing {
        current_path,
        parent_path,
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_home_dir() {
        let home = get_home_dir();
        assert!(!home.is_empty());
        assert!(Path::new(&home).is_absolute());
    }

    #[test]
    fn test_list_directory_non_existent() {
        let res = list_directory("/non_existent_directory_12345", true);
        assert!(res.is_err());
    }

    #[test]
    fn test_list_directory_valid() {
        let res = list_directory("/usr", true).expect("Should list /usr");
        assert_eq!(res.current_path, "/usr");
        assert_eq!(res.parent_path, Some("/".to_string()));
        assert!(!res.entries.is_empty());

        let bin_entry = res.entries.iter().find(|e| e.name == "bin");
        assert!(bin_entry.is_some(), "Expected 'bin' in /usr");
        let bin = bin_entry.unwrap();
        assert!(bin.metadata.permissions.is_some());
    }

    #[test]
    fn test_hidden_file_filtering() {
        let home = get_home_dir();
        let with_hidden = list_directory(&home, true).unwrap();
        let without_hidden = list_directory(&home, false).unwrap();

        let hidden_count = with_hidden.entries.iter().filter(|e| e.is_hidden).count();
        if hidden_count > 0 {
            assert!(with_hidden.entries.len() > without_hidden.entries.len());
            assert!(!without_hidden.entries.iter().any(|e| e.is_hidden));
        }
    }
}
