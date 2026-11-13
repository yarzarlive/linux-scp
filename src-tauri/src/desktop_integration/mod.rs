use crate::errors::AppError;
use crate::terminal::{
    resolve_executable_in_paths, resolve_terminal_for_preference, system_path_dirs,
    TerminalPreference,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Dedicated extension key used for files without an extension (e.g. `Makefile`, `README`, `.bashrc`).
pub const NO_EXTENSION_KEY: &str = "__no_extension__";

/// Known compound extensions that should be preserved as a unit rather than split on the last dot.
pub const COMPOUND_EXTENSIONS: &[&str] = &[
    ".tar.gz", ".tar.bz2", ".tar.xz", ".tar.zst", ".tar.lz4", ".d.ts",
];

/// Serializable descriptor of a discovered Linux XDG desktop application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopAppInfo {
    /// Stable desktop entry identifier (e.g. `org.gnome.TextEditor.desktop` or `code.desktop`).
    pub id: String,
    /// User-visible application name from `Name=`.
    pub name: String,
    /// Optional description from `Comment=` or `GenericName=`.
    pub comment: Option<String>,
    /// Optional icon identifier or path from `Icon=`.
    pub icon: Option<String>,
    /// Raw `Exec=` line from the `.desktop` entry.
    pub exec_raw: String,
    /// Resolved executable token (first argument of `Exec=`).
    pub executable: String,
    /// Tokenized argument template after the executable.
    pub args_template: Vec<String>,
    /// Declared MIME types from `MimeType=`.
    pub mime_types: Vec<String>,
    /// Declared categories from `Categories=`.
    pub categories: Vec<String>,
    /// Whether the application requires running inside a terminal (`Terminal=true`).
    pub terminal: bool,
    /// Source `.desktop` file path.
    pub desktop_file_path: String,
}

/// Persistent LinuxSCP-scoped file extension association stored in SQLite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileAppAssociation {
    pub extension: String,
    pub desktop_app_id: String,
    pub created_at: String,
    pub updated_at_ms: u64,
}

/// Context returned to the frontend when opening the `Open With` application picker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenWithContext {
    pub extension_key: String,
    pub extension_display: String,
    pub remembered_app_id: Option<String>,
    pub remembered_app_installed: bool,
    pub recommended_app_ids: Vec<String>,
    pub applications: Vec<DesktopAppInfo>,
}

/// Outcome of triggering the default `Open` action on a local file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome")]
#[allow(clippy::large_enum_variant)]
pub enum LocalFileOpenOutcome {
    /// Remembered default application was found and spawned directly.
    Launched {
        app: DesktopAppInfo,
        extension_key: String,
        pid: Option<u32>,
    },
    /// No remembered default exists (or the remembered application was uninstalled); show the picker.
    NeedsAppPicker {
        extension_key: String,
        extension_display: String,
        missing_remembered_app_id: Option<String>,
    },
}

/// Result of launching a local file with an explicitly selected desktop application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalFileLaunchResult {
    pub app: DesktopAppInfo,
    pub extension_key: String,
    pub saved_as_default: bool,
    pub pid: Option<u32>,
}

/// Outcome of triggering the default `Open` action on a remote file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome")]
#[allow(clippy::large_enum_variant)]
pub enum RemoteFileOpenOutcome {
    /// Remembered default application was found, remote file was downloaded into a managed
    /// temporary working copy, and the desktop application was spawned.
    Launched {
        app: DesktopAppInfo,
        extension_key: String,
        pid: Option<u32>,
        session: crate::remote_edit::RemoteEditSession,
    },
    /// No remembered default exists (or the remembered application was uninstalled); show the picker.
    NeedsAppPicker {
        extension_key: String,
        extension_display: String,
        missing_remembered_app_id: Option<String>,
    },
}

/// Result of launching a remote file with an explicitly selected desktop application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteFileLaunchResult {
    pub app: DesktopAppInfo,
    pub extension_key: String,
    pub saved_as_default: bool,
    pub pid: Option<u32>,
    pub session: crate::remote_edit::RemoteEditSession,
}

/// Verified, shell-free process launch plan for a desktop application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppLaunchPlan {
    pub executable: PathBuf,
    pub args: Vec<String>,
}

/// Extracts and normalizes the extension key from a filename or path.
///
/// Rules:
/// - Strips directory path components.
/// - Converts to lowercase for case-insensitive matching.
/// - Preserves compound extensions (`.tar.gz`, `.tar.xz`, `.tar.bz2`, etc.) when preceded by a stem.
/// - Treats pure dotfiles with no secondary dot (e.g. `.bashrc`, `.profile`) and extensionless files
///   (e.g. `Makefile`, `LICENSE`) as `NO_EXTENSION_KEY` (`"__no_extension__"`).
/// - Normalizes standard extensions with a leading dot (e.g. `".txt"`, `".json"`).
pub fn normalize_file_extension(filename_or_path: &str) -> String {
    let trimmed = filename_or_path.trim().trim_end_matches('/');
    let file_name = trimmed.rsplit('/').next().unwrap_or(trimmed).trim();

    if file_name.is_empty() || file_name == "." || file_name == ".." {
        return NO_EXTENSION_KEY.to_string();
    }

    let lower = file_name.to_lowercase();

    for compound in COMPOUND_EXTENSIONS {
        if lower.len() > compound.len() && lower.ends_with(compound) {
            let prefix = &lower[..lower.len() - compound.len()];
            if !prefix.is_empty() && prefix != "." {
                return (*compound).to_string();
            }
        }
    }

    // If the file starts with '.', strip the leading dot(s) when checking for an extension
    let trimmed_leading_dots = lower.trim_start_matches('.');
    if trimmed_leading_dots.is_empty() {
        return NO_EXTENSION_KEY.to_string();
    }

    match trimmed_leading_dots.rfind('.') {
        Some(dot_idx) if dot_idx > 0 && dot_idx + 1 < trimmed_leading_dots.len() => {
            format!(".{}", &trimmed_leading_dots[dot_idx + 1..])
        }
        _ => NO_EXTENSION_KEY.to_string(),
    }
}

/// Normalizes a raw extension key input (such as `".TXT"`, `"txt"`, `"__no_extension__"`, or `""`).
pub fn normalize_extension_input(raw_ext: &str) -> String {
    let trimmed = raw_ext.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case(NO_EXTENSION_KEY) {
        return NO_EXTENSION_KEY.to_string();
    }
    let lower = trimmed.to_lowercase();
    if lower.starts_with('.') {
        lower
    } else {
        format!(".{}", lower)
    }
}

/// Formats a normalized extension key for human-readable UI display.
pub fn format_extension_display(extension_key: &str) -> String {
    if extension_key == NO_EXTENSION_KEY || extension_key.trim().is_empty() {
        "Files without extension".to_string()
    } else {
        extension_key.to_string()
    }
}

/// Infers candidate MIME types for a normalized extension key to highlight recommended applications.
/// Unknown extensions return an empty list; the application picker always allows choosing any installed app.
pub fn infer_mime_types_for_extension(extension_key: &str) -> Vec<&'static str> {
    match extension_key {
        ".txt" | ".log" | ".conf" | ".ini" | ".cfg" | ".env" => vec!["text/plain"],
        ".md" | ".markdown" => vec!["text/markdown", "text/x-markdown", "text/plain"],
        ".json" => vec!["application/json", "text/json", "text/plain"],
        ".yaml" | ".yml" => vec!["application/x-yaml", "text/yaml", "text/plain"],
        ".toml" => vec!["application/toml", "text/x-toml", "text/plain"],
        ".xml" => vec!["application/xml", "text/xml", "text/plain"],
        ".html" | ".htm" => vec!["text/html", "text/plain"],
        ".css" => vec!["text/css", "text/plain"],
        ".js" | ".mjs" | ".cjs" => {
            vec!["application/javascript", "text/javascript", "text/plain"]
        }
        ".ts" | ".tsx" | ".jsx" | ".d.ts" => vec!["text/typescript", "text/plain"],
        ".rs" => vec!["text/rust", "text/x-rust", "text/plain"],
        ".py" => vec!["text/x-python", "application/x-python", "text/plain"],
        ".sh" | ".bash" | ".zsh" => {
            vec![
                "application/x-shellscript",
                "text/x-shellscript",
                "text/plain",
            ]
        }
        ".c" | ".h" => vec!["text/x-csrc", "text/x-chdr", "text/plain"],
        ".cpp" | ".cc" | ".cxx" | ".hpp" => vec!["text/x-c++src", "text/x-c++hdr", "text/plain"],
        ".sql" => vec!["application/sql", "text/x-sql", "text/plain"],
        ".csv" => vec!["text/csv", "text/plain"],
        ".pdf" => vec!["application/pdf"],
        ".png" => vec!["image/png"],
        ".jpg" | ".jpeg" => vec!["image/jpeg"],
        ".gif" => vec!["image/gif"],
        ".svg" => vec!["image/svg+xml"],
        ".webp" => vec!["image/webp"],
        ".mp3" => vec!["audio/mpeg"],
        ".wav" => vec!["audio/wav", "audio/x-wav"],
        ".mp4" => vec!["video/mp4"],
        ".mkv" => vec!["video/x-matroska"],
        ".zip" => vec!["application/zip"],
        ".tar" => vec!["application/x-tar"],
        ".gz" | ".tar.gz" => vec!["application/gzip", "application/x-compressed-tar"],
        ".bz2" | ".tar.bz2" => vec!["application/x-bzip", "application/x-bzip-compressed-tar"],
        ".xz" | ".tar.xz" => vec!["application/x-xz", "application/x-xz-compressed-tar"],
        NO_EXTENSION_KEY => vec!["text/plain"],
        _ => Vec::new(),
    }
}

/// Checks if a desktop application declares support for any of the candidate MIME types.
pub fn app_matches_mime_types(app: &DesktopAppInfo, candidate_mimes: &[&str]) -> bool {
    if candidate_mimes.is_empty() || app.mime_types.is_empty() {
        return false;
    }

    for candidate in candidate_mimes {
        let cand_lower = candidate.to_lowercase();
        let cand_prefix = cand_lower.split('/').next().unwrap_or("");
        for app_mime in &app.mime_types {
            let m = app_mime.to_lowercase();
            if m == cand_lower || (!cand_prefix.is_empty() && m == format!("{}/*", cand_prefix)) {
                return true;
            }
        }
    }

    false
}

/// Resolves standard XDG application directories in precedence order:
/// 1. `$XDG_DATA_HOME/applications` (or `~/.local/share/applications`)
/// 2. `~/.local/share/applications`
/// 3. `$XDG_DATA_DIRS/applications`
/// 4. `/usr/local/share/applications`, `/usr/share/applications`, Flatpak, and Snap export directories.
pub fn xdg_application_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut push_unique = |p: PathBuf| {
        if !dirs.contains(&p) {
            dirs.push(p);
        }
    };

    if let Some(xdg_home) = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
    {
        push_unique(xdg_home.join("applications"));
    }

    let user_local = PathBuf::from(shellexpand::tilde("~/.local/share/applications").into_owned());
    push_unique(user_local);

    let user_flatpak = PathBuf::from(
        shellexpand::tilde("~/.local/share/flatpak/exports/share/applications").into_owned(),
    );
    push_unique(user_flatpak);

    if let Some(xdg_dirs) = env::var_os("XDG_DATA_DIRS").filter(|s| !s.is_empty()) {
        for base in env::split_paths(&xdg_dirs) {
            if !base.as_os_str().is_empty() {
                push_unique(base.join("applications"));
            }
        }
    }

    for sys in [
        "/usr/local/share/applications",
        "/usr/share/applications",
        "/var/lib/flatpak/exports/share/applications",
        "/var/lib/snapd/desktop/applications",
    ] {
        push_unique(PathBuf::from(sys));
    }

    dirs
}

/// Tokenizes a desktop entry `Exec=` string into `(executable, args_template)` without invoking a shell.
/// Also handles leading `env VAR=VALUE` wrappers if present by keeping `env` as the binary or unwrapping when safe.
pub fn parse_desktop_exec(raw_exec: &str) -> Option<(String, Vec<String>)> {
    let unescaped = raw_exec.replace("\\\\", "\\");
    let words = shlex::split(&unescaped)?;
    if words.is_empty() {
        return None;
    }

    let executable = words[0].trim().to_string();
    if executable.is_empty() {
        return None;
    }

    let args = words[1..].to_vec();
    Some((executable, args))
}

/// Internal parse outcome distinguishing valid visible applications from masked (`Hidden`/`NoDisplay`) overrides.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
enum ParsedDesktopEntry {
    Visible(DesktopAppInfo),
    Masked,
    Invalid,
}

/// Parses the contents of a `.desktop` file according to the Freedesktop Desktop Entry Specification.
/// Only reads keys from the primary `[Desktop Entry]` group so `[Desktop Action ...]` sub-sections never clobber it.
fn parse_desktop_entry_raw(
    id: &str,
    desktop_file_path: &Path,
    content: &str,
    path_dirs: &[PathBuf],
) -> ParsedDesktopEntry {
    let mut in_desktop_entry_group = false;
    let mut seen_desktop_entry_group = false;

    let mut entry_type: Option<String> = None;
    let mut name: Option<String> = None;
    let mut generic_name: Option<String> = None;
    let mut comment: Option<String> = None;
    let mut icon: Option<String> = None;
    let mut exec_raw: Option<String> = None;
    let mut try_exec: Option<String> = None;
    let mut mime_types: Vec<String> = Vec::new();
    let mut categories: Vec<String> = Vec::new();
    let mut no_display = false;
    let mut hidden = false;
    let mut terminal = false;

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            if line == "[Desktop Entry]" {
                in_desktop_entry_group = true;
                seen_desktop_entry_group = true;
            } else {
                in_desktop_entry_group = false;
                if seen_desktop_entry_group {
                    // Stop once we leave the primary [Desktop Entry] group
                    break;
                }
            }
            continue;
        }

        if !in_desktop_entry_group {
            continue;
        }

        let Some((raw_key, raw_val)) = line.split_once('=') else {
            continue;
        };
        let key = raw_key.trim();
        let val = raw_val.trim();

        match key {
            "Type" if entry_type.is_none() => {
                entry_type = Some(val.to_string());
            }
            "Name" if name.is_none() && !val.is_empty() => {
                name = Some(val.to_string());
            }
            "GenericName" if generic_name.is_none() && !val.is_empty() => {
                generic_name = Some(val.to_string());
            }
            "Comment" if comment.is_none() && !val.is_empty() => {
                comment = Some(val.to_string());
            }
            "Icon" if icon.is_none() && !val.is_empty() => {
                icon = Some(val.to_string());
            }
            "Exec" if exec_raw.is_none() && !val.is_empty() => {
                exec_raw = Some(val.to_string());
            }
            "TryExec" if try_exec.is_none() && !val.is_empty() => {
                try_exec = Some(val.to_string());
            }
            "MimeType" => {
                mime_types = val
                    .split(';')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ToString::to_string)
                    .collect();
            }
            "Categories" => {
                categories = val
                    .split(';')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ToString::to_string)
                    .collect();
            }
            "NoDisplay" if val.eq_ignore_ascii_case("true") => {
                no_display = true;
            }
            "Hidden" if val.eq_ignore_ascii_case("true") => {
                hidden = true;
            }
            "Terminal" if val.eq_ignore_ascii_case("true") => {
                terminal = true;
            }
            _ => {}
        }
    }

    if !seen_desktop_entry_group {
        return ParsedDesktopEntry::Invalid;
    }

    if hidden || no_display {
        return ParsedDesktopEntry::Masked;
    }

    if let Some(ref t) = entry_type {
        if t != "Application" {
            return ParsedDesktopEntry::Masked;
        }
    }

    let (Some(app_name), Some(raw_exec_str)) = (name, exec_raw) else {
        return ParsedDesktopEntry::Invalid;
    };

    if let Some(ref try_bin) = try_exec {
        if resolve_executable_in_paths(try_bin, path_dirs).is_none() {
            return ParsedDesktopEntry::Masked;
        }
    }

    let Some((executable, args_template)) = parse_desktop_exec(&raw_exec_str) else {
        return ParsedDesktopEntry::Invalid;
    };

    // Verify that the primary executable is installed and executable
    if resolve_executable_in_paths(&executable, path_dirs).is_none() {
        return ParsedDesktopEntry::Masked;
    }

    ParsedDesktopEntry::Visible(DesktopAppInfo {
        id: id.to_string(),
        name: app_name,
        comment: comment.or(generic_name),
        icon,
        exec_raw: raw_exec_str,
        executable,
        args_template,
        mime_types,
        categories,
        terminal,
        desktop_file_path: desktop_file_path.to_string_lossy().into_owned(),
    })
}

/// Public helper to parse a `.desktop` file string into `DesktopAppInfo` if it is a valid visible application.
pub fn parse_desktop_entry_content(
    id: &str,
    desktop_file_path: &Path,
    content: &str,
    path_dirs: &[PathBuf],
) -> Option<DesktopAppInfo> {
    match parse_desktop_entry_raw(id, desktop_file_path, content, path_dirs) {
        ParsedDesktopEntry::Visible(info) => Some(info),
        ParsedDesktopEntry::Masked | ParsedDesktopEntry::Invalid => None,
    }
}

/// Discovers installed XDG desktop applications across the given application directories and `PATH` directories.
/// Respects XDG directory override precedence so user `.desktop` files override or mask system entries with the same ID.
pub fn discover_applications_in_dirs(
    app_dirs: &[PathBuf],
    path_dirs: &[PathBuf],
) -> Vec<DesktopAppInfo> {
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut apps_by_id: HashMap<String, DesktopAppInfo> = HashMap::new();

    for dir in app_dirs {
        if !dir.exists() {
            continue;
        }

        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };

        let mut dir_paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        dir_paths.sort();

        for path in dir_paths {
            if path.is_dir() {
                if let Ok(sub_entries) = fs::read_dir(&path) {
                    let mut sub_paths: Vec<PathBuf> =
                        sub_entries.flatten().map(|e| e.path()).collect();
                    sub_paths.sort();
                    for sub_path in sub_paths {
                        process_desktop_file_candidate(
                            &sub_path,
                            path_dirs,
                            &mut seen_ids,
                            &mut apps_by_id,
                        );
                    }
                }
            } else {
                process_desktop_file_candidate(&path, path_dirs, &mut seen_ids, &mut apps_by_id);
            }
        }
    }

    let mut list: Vec<DesktopAppInfo> = apps_by_id.into_values().collect();
    list.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    list
}

fn process_desktop_file_candidate(
    path: &Path,
    path_dirs: &[PathBuf],
    seen_ids: &mut HashSet<String>,
    apps_by_id: &mut HashMap<String, DesktopAppInfo>,
) {
    if path.extension().and_then(|s| s.to_str()) != Some("desktop") {
        return;
    }

    let Some(file_name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
        return;
    };

    if seen_ids.contains(&file_name) {
        return;
    }

    let Ok(content) = fs::read_to_string(path) else {
        return;
    };

    match parse_desktop_entry_raw(&file_name, path, &content, path_dirs) {
        ParsedDesktopEntry::Visible(app) => {
            seen_ids.insert(file_name.clone());
            apps_by_id.insert(file_name, app);
        }
        ParsedDesktopEntry::Masked => {
            // Higher-priority user override with Hidden=true or NoDisplay=true masks lower-priority entries
            seen_ids.insert(file_name);
        }
        ParsedDesktopEntry::Invalid => {}
    }
}

/// Expands desktop entry `Exec` field codes (`%f`, `%F`, `%u`, `%U`, `%i`, `%c`, `%k`, `%%`) safely into distinct arguments.
/// Never concatenates arguments into a shell string.
pub fn expand_exec_args(app: &DesktopAppInfo, target_files: &[String]) -> Vec<String> {
    let mut expanded: Vec<String> = Vec::new();
    let mut file_placeholder_consumed = false;

    let first_file = target_files.first().cloned().unwrap_or_default();

    for token in &app.args_template {
        match token.as_str() {
            "%f" | "%u" => {
                if !first_file.is_empty() {
                    expanded.push(first_file.clone());
                }
                file_placeholder_consumed = true;
            }
            "%F" | "%U" => {
                for file in target_files {
                    if !file.is_empty() {
                        expanded.push(file.clone());
                    }
                }
                file_placeholder_consumed = true;
            }
            "%i" => {
                if let Some(ref icon) = app.icon {
                    if !icon.trim().is_empty() {
                        expanded.push("--icon".to_string());
                        expanded.push(icon.clone());
                    }
                }
            }
            "%c" => {
                expanded.push(app.name.clone());
            }
            "%k" => {
                if !app.desktop_file_path.is_empty() {
                    expanded.push(app.desktop_file_path.clone());
                }
            }
            "%%" => {
                expanded.push("%".to_string());
            }
            other if other.starts_with('%') && other.len() == 2 => {
                // Strip deprecated/unsupported field codes (%d, %D, %n, %N, %v, %m, etc.)
            }
            other => {
                if other.contains("%f") || other.contains("%u") {
                    expanded.push(
                        other
                            .replace("%%", "\x00")
                            .replace("%f", &first_file)
                            .replace("%u", &first_file)
                            .replace('\x00', "%"),
                    );
                    file_placeholder_consumed = true;
                } else if other.contains("%F") || other.contains("%U") {
                    expanded.push(
                        other
                            .replace("%%", "\x00")
                            .replace("%F", &first_file)
                            .replace("%U", &first_file)
                            .replace('\x00', "%"),
                    );
                    file_placeholder_consumed = true;
                } else if other.contains("%%") {
                    expanded.push(other.replace("%%", "%"));
                } else {
                    expanded.push(other.to_string());
                }
            }
        }
    }

    if !file_placeholder_consumed {
        for file in target_files {
            if !file.is_empty() {
                expanded.push(file.clone());
            }
        }
    }

    expanded
}

/// Validates a local file path before launching an external desktop application.
pub fn validate_local_file_target(file_path: &str) -> Result<PathBuf, AppError> {
    let trimmed = file_path.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "Target file path cannot be empty.".to_string(),
        ));
    }
    if trimmed.contains('\0') {
        return Err(AppError::Validation(
            "Target file path must not contain NUL bytes.".to_string(),
        ));
    }

    let path = PathBuf::from(trimmed);
    let meta = fs::metadata(&path).map_err(|e| {
        AppError::Filesystem(format!(
            "Cannot open local file '{}': {}",
            path.display(),
            e
        ))
    })?;

    if meta.is_dir() {
        return Err(AppError::Validation(format!(
            "'{}' is a directory. Use directory navigation to open folders.",
            path.display()
        )));
    }

    Ok(path)
}

/// Builds a verified, shell-free `AppLaunchPlan` for a desktop application and one or more target file paths.
pub fn build_app_launch_plan(
    app: &DesktopAppInfo,
    target_files: &[String],
    path_dirs: &[PathBuf],
    terminal_preference: Option<&TerminalPreference>,
) -> Result<AppLaunchPlan, AppError> {
    if target_files.is_empty() {
        return Err(AppError::Validation(
            "At least one target file path is required to launch an application.".to_string(),
        ));
    }

    for f in target_files {
        if f.trim().is_empty() || f.contains('\0') {
            return Err(AppError::Validation(
                "Invalid target file path for application launch.".to_string(),
            ));
        }
    }

    let resolved_exec =
        resolve_executable_in_paths(&app.executable, path_dirs).ok_or_else(|| {
            AppError::Filesystem(format!(
                "Application '{}' executable '{}' is no longer installed or not found on PATH.",
                app.name, app.executable
            ))
        })?;

    let app_args = expand_exec_args(app, target_files);

    if app.terminal {
        let default_pref = TerminalPreference::default();
        let pref = terminal_preference.unwrap_or(&default_pref);
        let resolved_term = resolve_terminal_for_preference(pref, path_dirs)?;

        let mut term_args = resolved_term.exec_prefix_args;
        term_args.push(resolved_exec.to_string_lossy().into_owned());
        term_args.extend(app_args);

        Ok(AppLaunchPlan {
            executable: resolved_term.executable_path,
            args: term_args,
        })
    } else {
        Ok(AppLaunchPlan {
            executable: resolved_exec,
            args: app_args,
        })
    }
}

/// Spawns the desktop application directly via `std::process::Command` without shell interpolation.
pub fn spawn_app_launch_plan(
    app: &DesktopAppInfo,
    plan: &AppLaunchPlan,
) -> Result<Option<u32>, AppError> {
    tracing::info!(
        app_id = %app.id,
        app_name = %app.name,
        executable = %plan.executable.display(),
        "Launching desktop application via direct process argv"
    );

    let mut child = Command::new(&plan.executable)
        .args(&plan.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            AppError::Filesystem(format!(
                "Failed to launch '{}' ({}): {}",
                app.name,
                plan.executable.display(),
                e
            ))
        })?;

    let pid = Some(child.id());

    // Reap the process asynchronously on exit so no <defunct> zombie process accumulates
    std::thread::spawn(move || {
        let _ = child.wait();
    });

    Ok(pid)
}

/// Discovers all installed desktop applications on the current Linux host.
pub fn discover_system_applications() -> Vec<DesktopAppInfo> {
    let app_dirs = xdg_application_dirs();
    let path_dirs = system_path_dirs();
    discover_applications_in_dirs(&app_dirs, &path_dirs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::os::unix::fs::PermissionsExt;

    fn create_temp_dir(prefix: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("linuxscp-xdg-{}-{}", prefix, uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("Should create temp dir");
        dir
    }

    fn create_fake_bin(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        File::create(&p).expect("Should create fake binary");
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755))
            .expect("Should chmod fake binary");
        p
    }

    #[test]
    fn test_normalize_file_extension_cases_compound_dotfiles_and_no_ext() {
        // Case insensitivity
        assert_eq!(normalize_file_extension("/home/user/Report.PDF"), ".pdf");
        assert_eq!(normalize_file_extension("README.MD"), ".md");
        assert_eq!(normalize_file_extension("script.Sh"), ".sh");

        // Compound extensions
        assert_eq!(
            normalize_file_extension("/var/backups/archive.TAR.GZ"),
            ".tar.gz"
        );
        assert_eq!(normalize_file_extension("release.v1.2.tar.xz"), ".tar.xz");
        assert_eq!(normalize_file_extension("types.d.ts"), ".d.ts");

        // Multiple dots (non-compound)
        assert_eq!(normalize_file_extension("my.custom.config.JSON"), ".json");

        // Dotfiles with no secondary extension -> NO_EXTENSION_KEY
        assert_eq!(
            normalize_file_extension("/home/user/.bashrc"),
            NO_EXTENSION_KEY
        );
        assert_eq!(normalize_file_extension(".gitignore"), NO_EXTENSION_KEY);

        // Dotfiles with a secondary extension -> extracts extension
        assert_eq!(normalize_file_extension(".eslintrc.json"), ".json");

        // Extensionless files -> NO_EXTENSION_KEY
        assert_eq!(
            normalize_file_extension("/home/user/Makefile"),
            NO_EXTENSION_KEY
        );
        assert_eq!(normalize_file_extension("LICENSE"), NO_EXTENSION_KEY);
    }

    #[test]
    fn test_desktop_entry_discovery_filtering_and_user_override_precedence() {
        let root = create_temp_dir("discovery");
        let bin_dir = root.join("bin");
        let user_apps = root.join("user_applications");
        let sys_apps = root.join("sys_applications");
        fs::create_dir_all(&bin_dir).unwrap();
        fs::create_dir_all(&user_apps).unwrap();
        fs::create_dir_all(&sys_apps).unwrap();

        create_fake_bin(&bin_dir, "my-editor");
        create_fake_bin(&bin_dir, "my-viewer");
        create_fake_bin(&bin_dir, "hidden-tool");

        // System editor entry (also includes a [Desktop Action] with a different Name/Exec)
        let sys_editor_desktop = r#"
[Desktop Entry]
Type=Application
Name=System Text Editor
Comment=Edit plain text files
Exec=my-editor %F
Icon=accessories-text-editor
MimeType=text/plain;text/markdown;

[Desktop Action new-window]
Name=New Window
Exec=my-editor --new-window
"#;
        fs::write(sys_apps.join("editor.desktop"), sys_editor_desktop).unwrap();

        // User override for editor.desktop with custom Name
        let user_editor_desktop = r#"
[Desktop Entry]
Type=Application
Name=Custom User Editor
Comment=User overridden editor
Exec=my-editor --user %f
MimeType=text/plain;
"#;
        fs::write(user_apps.join("editor.desktop"), user_editor_desktop).unwrap();

        // System viewer entry (not overridden)
        let sys_viewer_desktop = r#"
[Desktop Entry]
Type=Application
Name=Image Viewer
Exec=my-viewer %u
MimeType=image/png;image/jpeg;
"#;
        fs::write(sys_apps.join("viewer.desktop"), sys_viewer_desktop).unwrap();

        // NoDisplay=true entry (must be filtered out)
        let nodisplay_desktop = r#"
[Desktop Entry]
Type=Application
Name=Hidden Handler
NoDisplay=true
Exec=hidden-tool %f
"#;
        fs::write(sys_apps.join("nodisplay.desktop"), nodisplay_desktop).unwrap();

        let discovered = discover_applications_in_dirs(
            &[user_apps.clone(), sys_apps.clone()],
            std::slice::from_ref(&bin_dir),
        );

        assert_eq!(discovered.len(), 2);
        // Sorted alphabetically by Name: "Custom User Editor", "Image Viewer"
        assert_eq!(discovered[0].id, "editor.desktop");
        assert_eq!(discovered[0].name, "Custom User Editor");
        assert_eq!(discovered[0].args_template, vec!["--user", "%f"]);

        assert_eq!(discovered[1].id, "viewer.desktop");
        assert_eq!(discovered[1].name, "Image Viewer");
        assert_eq!(discovered[1].mime_types, vec!["image/png", "image/jpeg"]);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_expand_exec_args_and_safe_launch_plan_without_shell_injection() {
        let root = create_temp_dir("exec-expand");
        let bin_dir = root.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let editor_bin = create_fake_bin(&bin_dir, "safe-editor");

        let app = DesktopAppInfo {
            id: "safe-editor.desktop".to_string(),
            name: "Safe Editor".to_string(),
            comment: None,
            icon: Some("editor-icon".to_string()),
            exec_raw: "safe-editor --title %c %i -- %F %d".to_string(),
            executable: "safe-editor".to_string(),
            args_template: vec![
                "--title".to_string(),
                "%c".to_string(),
                "%i".to_string(),
                "--".to_string(),
                "%F".to_string(),
                "%d".to_string(), // deprecated code, should be stripped
            ],
            mime_types: vec!["text/plain".to_string()],
            categories: vec![],
            terminal: false,
            desktop_file_path: "/usr/share/applications/safe-editor.desktop".to_string(),
        };

        // File path containing spaces and shell metacharacters
        let tricky_path = "/tmp/my file; rm -rf $(whoami).txt".to_string();
        let plan = build_app_launch_plan(
            &app,
            std::slice::from_ref(&tricky_path),
            std::slice::from_ref(&bin_dir),
            None,
        )
        .expect("Should build launch plan");

        assert_eq!(plan.executable, editor_bin);
        assert_eq!(
            plan.args,
            vec![
                "--title",
                "Safe Editor",
                "--icon",
                "editor-icon",
                "--",
                "/tmp/my file; rm -rf $(whoami).txt"
            ]
        );

        // App without any %f/%F/%u/%U placeholder should still receive the target file at the end
        let app_no_placeholder = DesktopAppInfo {
            args_template: vec!["--new-window".to_string()],
            ..app
        };
        let plan2 = build_app_launch_plan(
            &app_no_placeholder,
            std::slice::from_ref(&tricky_path),
            std::slice::from_ref(&bin_dir),
            None,
        )
        .expect("Should build launch plan appending file");
        assert_eq!(
            plan2.args,
            vec!["--new-window", "/tmp/my file; rm -rf $(whoami).txt"]
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_host_desktop_discovery_and_multi_app_execution_verification() {
        // 1. Verify discovery against real Linux desktop entries on this machine
        let host_apps = discover_system_applications();
        assert!(
            !host_apps.is_empty(),
            "Should discover installed XDG desktop applications on the target Linux desktop"
        );

        // 2. Verify launching multiple distinct applications safely and capturing their exact argv
        let root = create_temp_dir("multi-app-exec");
        let bin_dir = root.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();

        let target_file = root.join("sample notes (v1).txt");
        fs::write(&target_file, "Hello LinuxSCP Open With!").unwrap();

        for app_idx in [1, 2] {
            let out_file = root.join(format!("app_{}_output.txt", app_idx));
            let bin_name = format!("test-desktop-app-{}", app_idx);
            let bin_path = bin_dir.join(&bin_name);

            let script = format!(
                "#!/bin/sh\nfor a in \"$@\"; do\n  printf '%s\\n' \"$a\" >> \"{}\"\ndone\n",
                out_file.display()
            );
            fs::write(&bin_path, script).unwrap();
            fs::set_permissions(&bin_path, fs::Permissions::from_mode(0o755)).unwrap();

            let desktop_content = format!(
                "[Desktop Entry]\nType=Application\nName=Test App {}\nExec={} --mode=app{} %f\nMimeType=text/plain;\n",
                app_idx, bin_name, app_idx
            );
            let desktop_path = root.join(format!("app-{}.desktop", app_idx));
            let parsed = parse_desktop_entry_content(
                &format!("app-{}.desktop", app_idx),
                &desktop_path,
                &desktop_content,
                std::slice::from_ref(&bin_dir),
            )
            .expect("Should parse desktop entry");

            let plan = build_app_launch_plan(
                &parsed,
                &[target_file.to_string_lossy().into_owned()],
                std::slice::from_ref(&bin_dir),
                None,
            )
            .expect("Should build launch plan");

            let pid = spawn_app_launch_plan(&parsed, &plan).expect("Should spawn desktop app");
            assert!(pid.is_some());

            for _ in 0..40 {
                if let Ok(content) = fs::read_to_string(&out_file) {
                    if content.lines().count() >= 2 {
                        break;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }

            let captured = fs::read_to_string(&out_file).expect("Should capture app output");
            let lines: Vec<&str> = captured.lines().collect();
            assert_eq!(
                lines,
                vec![
                    format!("--mode=app{}", app_idx).as_str(),
                    target_file.to_string_lossy().as_ref()
                ]
            );
        }

        let _ = fs::remove_dir_all(&root);
    }
}
