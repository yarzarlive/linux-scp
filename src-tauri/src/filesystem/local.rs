use crate::errors::AppError;
use crate::models::{FileEntry, FileMetadata, FileOwnership, FileType, UnixPermissions};
use serde::{Deserialize, Serialize};
use std::ffi::CStr;
use std::fs::{self, OpenOptions};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

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

/// Validates that a file or directory name is safe (no slashes, nulls, traversal dots).
pub fn validate_entry_name(name: &str) -> Result<&str, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "File or directory name cannot be empty".to_string(),
        ));
    }
    if trimmed == "." || trimmed == ".." {
        return Err(AppError::Validation(
            "Name cannot be '.' or '..'".to_string(),
        ));
    }
    if trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains('\0') {
        return Err(AppError::Validation(
            "Name cannot contain path separators or null bytes".to_string(),
        ));
    }
    if trimmed.len() > 255 {
        return Err(AppError::Validation(
            "Name exceeds maximum filesystem length of 255 bytes".to_string(),
        ));
    }
    Ok(trimmed)
}

/// Checks if a path targets a critical system directory that must not be deleted.
fn is_protected_system_path(path: &Path) -> bool {
    let protected = [
        "/", "/bin", "/boot", "/dev", "/etc", "/lib", "/lib64", "/proc", "/sys", "/usr", "/sbin",
        "/root", "/home",
    ];
    let path_str = path.to_string_lossy();
    protected.iter().any(|&p| path_str == p)
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

/// Creates a new directory inside the given parent path.
pub fn create_directory(raw_parent_path: &str, dir_name: &str) -> Result<String, AppError> {
    let safe_name = validate_entry_name(dir_name)?;
    let expanded_parent = shellexpand::tilde(raw_parent_path).to_string();
    let parent = Path::new(&expanded_parent);

    if !parent.exists() {
        return Err(AppError::Filesystem(format!(
            "Parent directory does not exist: {}",
            raw_parent_path
        )));
    }

    let canonical_parent = parent.canonicalize().map_err(|e| {
        AppError::Filesystem(format!("Failed to resolve parent directory path: {}", e))
    })?;

    if !canonical_parent.is_dir() {
        return Err(AppError::Filesystem(format!(
            "Parent path is not a directory: {}",
            canonical_parent.display()
        )));
    }

    let target_path = canonical_parent.join(safe_name);
    if target_path.exists() {
        return Err(AppError::Filesystem(format!(
            "A file or directory named '{}' already exists",
            safe_name
        )));
    }

    fs::create_dir(&target_path).map_err(|e| {
        AppError::Filesystem(format!(
            "Failed to create directory '{}': {}",
            target_path.display(),
            e
        ))
    })?;

    Ok(target_path.to_string_lossy().to_string())
}

/// Creates a new empty file inside the given parent path.
pub fn create_file(raw_parent_path: &str, file_name: &str) -> Result<String, AppError> {
    let safe_name = validate_entry_name(file_name)?;
    let expanded_parent = shellexpand::tilde(raw_parent_path).to_string();
    let parent = Path::new(&expanded_parent);

    if !parent.exists() {
        return Err(AppError::Filesystem(format!(
            "Parent directory does not exist: {}",
            raw_parent_path
        )));
    }

    let canonical_parent = parent.canonicalize().map_err(|e| {
        AppError::Filesystem(format!("Failed to resolve parent directory path: {}", e))
    })?;

    if !canonical_parent.is_dir() {
        return Err(AppError::Filesystem(format!(
            "Parent path is not a directory: {}",
            canonical_parent.display()
        )));
    }

    let target_path = canonical_parent.join(safe_name);
    if target_path.exists() {
        return Err(AppError::Filesystem(format!(
            "A file or directory named '{}' already exists",
            safe_name
        )));
    }

    // Atomic create without overwriting
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target_path)
        .map_err(|e| {
            AppError::Filesystem(format!(
                "Failed to create file '{}': {}",
                target_path.display(),
                e
            ))
        })?;

    Ok(target_path.to_string_lossy().to_string())
}

/// Renames a file or directory to a new name in the same parent directory.
pub fn rename_entry(raw_source_path: &str, new_name: &str) -> Result<String, AppError> {
    let safe_name = validate_entry_name(new_name)?;
    let expanded = shellexpand::tilde(raw_source_path).to_string();
    let source = Path::new(&expanded);

    // Use symlink_metadata to support renaming symlinks
    let metadata = fs::symlink_metadata(source).map_err(|_| {
        AppError::Filesystem(format!("Source entry does not exist: {}", raw_source_path))
    })?;

    let parent = source.parent().ok_or_else(|| {
        AppError::Validation("Cannot rename root filesystem directory".to_string())
    })?;

    let target_path = parent.join(safe_name);

    if target_path == source {
        return Ok(target_path.to_string_lossy().to_string());
    }

    // Check if target already exists
    if fs::symlink_metadata(&target_path).is_ok() {
        return Err(AppError::Filesystem(format!(
            "An entry named '{}' already exists in this directory",
            safe_name
        )));
    }

    // Protect root directories
    if is_protected_system_path(source) {
        return Err(AppError::Security(
            "Cannot rename protected system directory".to_string(),
        ));
    }

    let _ = metadata; // used to confirm existence via symlink_metadata
    fs::rename(source, &target_path).map_err(|e| {
        AppError::Filesystem(format!(
            "Failed to rename '{}' to '{}': {}",
            source.display(),
            safe_name,
            e
        ))
    })?;

    Ok(target_path.to_string_lossy().to_string())
}

/// Deletes one or more files, symlinks, or directories.
/// Symlinks are strictly unlinked and never traversed recursively.
pub fn delete_entries(raw_paths: Vec<String>) -> Result<Vec<String>, AppError> {
    let mut deleted = Vec::new();

    for raw_path in raw_paths {
        let expanded = shellexpand::tilde(&raw_path).to_string();
        let path = PathBuf::from(&expanded);

        // Symlink metadata must be used so symlinks are not followed
        let metadata = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                return Err(AppError::Filesystem(format!(
                    "Path does not exist or is inaccessible '{}': {}",
                    raw_path, e
                )));
            }
        };

        if is_protected_system_path(&path) {
            return Err(AppError::Security(format!(
                "Cannot delete protected system directory: {}",
                raw_path
            )));
        }

        if metadata.file_type().is_symlink() {
            // Unlink symlink safely without touching the target
            fs::remove_file(&path).map_err(|e| {
                AppError::Filesystem(format!("Failed to remove symlink '{}': {}", raw_path, e))
            })?;
        } else if metadata.is_dir() {
            // Recursive directory removal
            fs::remove_dir_all(&path).map_err(|e| {
                AppError::Filesystem(format!("Failed to remove directory '{}': {}", raw_path, e))
            })?;
        } else {
            // Regular or special file removal
            fs::remove_file(&path).map_err(|e| {
                AppError::Filesystem(format!("Failed to remove file '{}': {}", raw_path, e))
            })?;
        }

        deleted.push(raw_path);
    }

    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn test_get_home_dir() {
        let home = get_home_dir();
        assert!(!home.is_empty());
        assert!(Path::new(&home).is_absolute());
    }

    #[test]
    fn test_validate_entry_name() {
        assert!(validate_entry_name("valid_name.txt").is_ok());
        assert!(validate_entry_name("valid-dir").is_ok());

        assert!(validate_entry_name("").is_err());
        assert!(validate_entry_name("   ").is_err());
        assert!(validate_entry_name(".").is_err());
        assert!(validate_entry_name("..").is_err());
        assert!(validate_entry_name("folder/subfolder").is_err());
        assert!(validate_entry_name("folder\\subfolder").is_err());
        assert!(validate_entry_name("bad\0null").is_err());

        let long_name = "a".repeat(256);
        assert!(validate_entry_name(&long_name).is_err());
    }

    #[test]
    fn test_create_and_delete_directory() {
        let temp_dir =
            std::env::temp_dir().join(format!("linuxscp_test_dir_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        let parent = temp_dir.to_str().unwrap();
        let new_dir_path =
            create_directory(parent, "test_subfolder").expect("Should create directory");
        assert!(Path::new(&new_dir_path).is_dir());

        // Duplicate create should fail
        let duplicate = create_directory(parent, "test_subfolder");
        assert!(duplicate.is_err());

        // Delete directory
        let deleted = delete_entries(vec![new_dir_path.clone()]).expect("Should delete directory");
        assert_eq!(deleted, vec![new_dir_path.clone()]);
        assert!(!Path::new(&new_dir_path).exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_create_and_delete_file() {
        let temp_dir =
            std::env::temp_dir().join(format!("linuxscp_test_file_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        let parent = temp_dir.to_str().unwrap();
        let new_file_path = create_file(parent, "hello.txt").expect("Should create file");
        assert!(Path::new(&new_file_path).is_file());

        // Duplicate create should fail
        let duplicate = create_file(parent, "hello.txt");
        assert!(duplicate.is_err());

        // Delete file
        let deleted = delete_entries(vec![new_file_path.clone()]).expect("Should delete file");
        assert_eq!(deleted, vec![new_file_path.clone()]);
        assert!(!Path::new(&new_file_path).exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_rename_entry() {
        let temp_dir =
            std::env::temp_dir().join(format!("linuxscp_test_rename_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        let parent = temp_dir.to_str().unwrap();
        let old_file = create_file(parent, "initial.txt").expect("Should create file");

        let new_file = rename_entry(&old_file, "renamed.txt").expect("Should rename file");
        assert!(!Path::new(&old_file).exists());
        assert!(Path::new(&new_file).is_file());

        // Renaming to existing file should fail
        let other_file = create_file(parent, "other.txt").expect("Should create other file");
        let duplicate_rename = rename_entry(&other_file, "renamed.txt");
        assert!(duplicate_rename.is_err());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_symlink_deletion_does_not_delete_target() {
        let temp_dir =
            std::env::temp_dir().join(format!("linuxscp_test_symlink_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        // Create target directory with a file inside
        let target_dir = temp_dir.join("target_dir");
        fs::create_dir_all(&target_dir).unwrap();
        let target_file = target_dir.join("vital_data.txt");
        fs::write(&target_file, "vital contents").unwrap();

        // Create symlink pointing to target directory
        let link_path = temp_dir.join("link_to_target");
        symlink(&target_dir, &link_path).expect("Should create symlink");

        // Delete the symlink
        let deleted = delete_entries(vec![link_path.to_str().unwrap().to_string()])
            .expect("Should delete symlink");
        assert_eq!(deleted.len(), 1);

        // Verify symlink is gone, but the target directory and its contents are safe!
        assert!(!link_path.exists());
        assert!(target_dir.exists(), "Target directory must NOT be deleted!");
        assert!(target_file.exists(), "Target file must NOT be deleted!");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_protected_root_deletion_rejected() {
        let res = delete_entries(vec!["/".to_string()]);
        assert!(res.is_err());
        assert!(matches!(res.unwrap_err(), AppError::Security(_)));

        let res_etc = delete_entries(vec!["/etc".to_string()]);
        assert!(res_etc.is_err());
        assert!(matches!(res_etc.unwrap_err(), AppError::Security(_)));
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
