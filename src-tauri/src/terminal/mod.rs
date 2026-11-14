use crate::errors::AppError;
use crate::models::{AuthMethod, ConnectionProfile, ProtocolType};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Static specification of a supported Linux desktop terminal emulator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupportedTerminalSpec {
    /// Executable binary name searched on `PATH`.
    pub binary: &'static str,
    /// Human-readable display label.
    pub display_name: &'static str,
    /// Distinct argument prefix required before the `ssh` command vector.
    pub exec_prefix_args: &'static [&'static str],
}

/// Documented list of supported Linux terminal emulators searched in priority order.
pub const SUPPORTED_TERMINALS: &[SupportedTerminalSpec] = &[
    SupportedTerminalSpec {
        binary: "x-terminal-emulator",
        display_name: "System Default (x-terminal-emulator)",
        exec_prefix_args: &["-e"],
    },
    SupportedTerminalSpec {
        binary: "gnome-terminal",
        display_name: "GNOME Terminal (gnome-terminal)",
        exec_prefix_args: &["--"],
    },
    SupportedTerminalSpec {
        binary: "ptyxis",
        display_name: "Ptyxis (ptyxis)",
        exec_prefix_args: &["--"],
    },
    SupportedTerminalSpec {
        binary: "konsole",
        display_name: "KDE Konsole (konsole)",
        exec_prefix_args: &["-e"],
    },
    SupportedTerminalSpec {
        binary: "alacritty",
        display_name: "Alacritty (alacritty)",
        exec_prefix_args: &["-e"],
    },
    SupportedTerminalSpec {
        binary: "kitty",
        display_name: "Kitty (kitty)",
        exec_prefix_args: &["--"],
    },
    SupportedTerminalSpec {
        binary: "xfce4-terminal",
        display_name: "XFCE Terminal (xfce4-terminal)",
        exec_prefix_args: &["-x"],
    },
    SupportedTerminalSpec {
        binary: "kgx",
        display_name: "GNOME Console (kgx)",
        exec_prefix_args: &["--"],
    },
    SupportedTerminalSpec {
        binary: "mate-terminal",
        display_name: "MATE Terminal (mate-terminal)",
        exec_prefix_args: &["-x"],
    },
    SupportedTerminalSpec {
        binary: "tilix",
        display_name: "Tilix (tilix)",
        exec_prefix_args: &["-e"],
    },
    SupportedTerminalSpec {
        binary: "terminator",
        display_name: "Terminator (terminator)",
        exec_prefix_args: &["-x"],
    },
    SupportedTerminalSpec {
        binary: "wezterm",
        display_name: "WezTerm (wezterm)",
        exec_prefix_args: &["start", "--"],
    },
    SupportedTerminalSpec {
        binary: "foot",
        display_name: "Foot (foot)",
        exec_prefix_args: &["--"],
    },
    SupportedTerminalSpec {
        binary: "lxterminal",
        display_name: "LXTerminal (lxterminal)",
        exec_prefix_args: &["-e"],
    },
    SupportedTerminalSpec {
        binary: "xterm",
        display_name: "XTerm (xterm)",
        exec_prefix_args: &["-e"],
    },
];

/// Serializable descriptor of a supported terminal emulator and its availability on the host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupportedTerminalInfo {
    pub binary: String,
    pub display_name: String,
    pub exec_prefix_args: Vec<String>,
    pub installed: bool,
    pub resolved_path: Option<String>,
}

/// Mode for selecting the terminal emulator to launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TerminalPreferenceMode {
    /// Automatically choose the first installed terminal from `SUPPORTED_TERMINALS`.
    #[default]
    Auto,
    /// Use a specific terminal binary from `SUPPORTED_TERMINALS`.
    Preset,
    /// Use a user-configured executable and distinct argument prefix.
    Custom,
}

/// Persistent user configuration for external SSH terminal launching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalPreference {
    pub mode: TerminalPreferenceMode,
    pub preset_binary: Option<String>,
    pub custom_executable: Option<String>,
    #[serde(default)]
    pub custom_args_prefix: Vec<String>,
}

impl Default for TerminalPreference {
    fn default() -> Self {
        Self {
            mode: TerminalPreferenceMode::Auto,
            preset_binary: None,
            custom_executable: None,
            custom_args_prefix: vec!["-e".to_string()],
        }
    }
}

impl TerminalPreference {
    /// Validates the terminal preference without spawning any process.
    pub fn validate(&self) -> Result<(), AppError> {
        match self.mode {
            TerminalPreferenceMode::Auto => Ok(()),
            TerminalPreferenceMode::Preset => {
                let binary = self
                    .preset_binary
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| {
                        AppError::Validation(
                            "A supported terminal binary must be selected in Preset mode."
                                .to_string(),
                        )
                    })?;

                if !SUPPORTED_TERMINALS.iter().any(|t| t.binary == binary) {
                    return Err(AppError::Validation(format!(
                        "'{}' is not in the documented list of supported terminal emulators.",
                        binary
                    )));
                }
                Ok(())
            }
            TerminalPreferenceMode::Custom => {
                let raw_exec = self
                    .custom_executable
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| {
                        AppError::Validation(
                            "Custom terminal executable cannot be empty.".to_string(),
                        )
                    })?;

                validate_safe_executable_token(raw_exec)?;

                for arg in &self.custom_args_prefix {
                    if arg.chars().any(|c| c == '\0' || c == '\n' || c == '\r') {
                        return Err(AppError::Validation(
                            "Custom terminal arguments must not contain control characters."
                                .to_string(),
                        ));
                    }
                }

                Ok(())
            }
        }
    }
}

/// Environment snapshot returned to the frontend for terminal settings and diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalEnvironmentInfo {
    pub preference: TerminalPreference,
    pub supported_terminals: Vec<SupportedTerminalInfo>,
    pub detected_default: Option<SupportedTerminalInfo>,
    pub ssh_binary_path: Option<String>,
}

/// Explicit launch outcome status.
/// Never claims SSH authentication succeeded merely because the terminal process was spawned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalLaunchStatus {
    ProcessSpawned,
}

/// Result returned to the frontend when an external SSH terminal process is spawned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalLaunchResult {
    pub profile_id: String,
    pub profile_name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub terminal_binary: String,
    pub pid: Option<u32>,
    pub status: TerminalLaunchStatus,
    pub message: String,
}

/// Resolved terminal emulator ready for direct process invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTerminal {
    pub binary_label: String,
    pub executable_path: PathBuf,
    pub exec_prefix_args: Vec<String>,
}

/// Structured, shell-free launch plan for spawning the terminal emulator and OpenSSH client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLaunchPlan {
    pub terminal_binary: String,
    pub terminal_executable: PathBuf,
    pub terminal_args: Vec<String>,
    pub ssh_executable: String,
    pub ssh_args: Vec<String>,
    pub remote_directory: Option<String>,
}

/// Returns the directories listed in the current process `PATH` environment variable.
pub fn system_path_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = env::var_os("PATH")
        .map(|val| env::split_paths(&val).collect())
        .unwrap_or_default();

    for fallback in ["/usr/local/bin", "/usr/bin", "/bin"] {
        let p = PathBuf::from(fallback);
        if !dirs.contains(&p) {
            dirs.push(p);
        }
    }
    dirs
}

/// Checks if a filesystem path points to a regular file with at least one execute bit set.
pub fn is_executable_file(path: &Path) -> bool {
    match fs::metadata(path) {
        Ok(meta) => meta.is_file() && (meta.permissions().mode() & 0o111 != 0),
        Err(_) => false,
    }
}

/// Validates that an executable token is a single binary name or path, not a shell command string.
fn validate_safe_executable_token(raw_exec: &str) -> Result<(), AppError> {
    let trimmed = raw_exec.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "Terminal executable cannot be empty.".to_string(),
        ));
    }
    if trimmed.starts_with('-') {
        return Err(AppError::Validation(
            "Terminal executable must not start with '-'.".to_string(),
        ));
    }
    if trimmed.chars().any(|c| {
        c.is_control()
            || matches!(
                c,
                ';' | '|' | '&' | '$' | '`' | '>' | '<' | '(' | ')' | '\'' | '"'
            )
    }) {
        return Err(AppError::Validation(
            "Terminal executable contains forbidden shell metacharacters. Specify only the binary name or path."
                .to_string(),
        ));
    }
    if !trimmed.contains('/') && trimmed.chars().any(char::is_whitespace) {
        return Err(AppError::Validation(
            "Terminal executable must be a single binary name or path (do not concatenate arguments into the executable field)."
                .to_string(),
        ));
    }
    Ok(())
}

/// Resolves an executable name or path against the provided search directories.
pub fn resolve_executable_in_paths(executable: &str, search_dirs: &[PathBuf]) -> Option<PathBuf> {
    let trimmed = executable.trim();
    if trimmed.is_empty() {
        return None;
    }

    let expanded = shellexpand::tilde(trimmed);
    let candidate_path = Path::new(expanded.as_ref());

    if expanded.contains('/') {
        if is_executable_file(candidate_path) {
            return Some(candidate_path.to_path_buf());
        }
        return None;
    }

    for dir in search_dirs {
        let full = dir.join(expanded.as_ref());
        if is_executable_file(&full) {
            return Some(full);
        }
    }

    None
}

/// Resolves the system OpenSSH (`ssh`) client binary in the provided search directories.
pub fn resolve_ssh_binary_in_paths(search_dirs: &[PathBuf]) -> Result<PathBuf, AppError> {
    if let Some(found) = resolve_executable_in_paths("ssh", search_dirs) {
        return Ok(found);
    }

    let standard = PathBuf::from("/usr/bin/ssh");
    if is_executable_file(&standard) {
        return Ok(standard);
    }

    Err(AppError::Validation(
        "System OpenSSH client ('ssh') was not found on PATH or at /usr/bin/ssh. Please install openssh-client."
            .to_string(),
    ))
}

/// Evaluates all documented supported terminal emulators against the given `PATH` directories.
pub fn discover_terminals_in_paths(search_dirs: &[PathBuf]) -> Vec<SupportedTerminalInfo> {
    SUPPORTED_TERMINALS
        .iter()
        .map(|spec| {
            let resolved = resolve_executable_in_paths(spec.binary, search_dirs);
            SupportedTerminalInfo {
                binary: spec.binary.to_string(),
                display_name: spec.display_name.to_string(),
                exec_prefix_args: spec
                    .exec_prefix_args
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect(),
                installed: resolved.is_some(),
                resolved_path: resolved.map(|p| p.to_string_lossy().into_owned()),
            }
        })
        .collect()
}

/// Resolves which terminal emulator to execute based on the user's `TerminalPreference` and `search_dirs`.
pub fn resolve_terminal_for_preference(
    preference: &TerminalPreference,
    search_dirs: &[PathBuf],
) -> Result<ResolvedTerminal, AppError> {
    preference.validate()?;

    match preference.mode {
        TerminalPreferenceMode::Auto => {
            for spec in SUPPORTED_TERMINALS {
                if let Some(path) = resolve_executable_in_paths(spec.binary, search_dirs) {
                    return Ok(ResolvedTerminal {
                        binary_label: spec.binary.to_string(),
                        executable_path: path,
                        exec_prefix_args: spec
                            .exec_prefix_args
                            .iter()
                            .map(|s| (*s).to_string())
                            .collect(),
                    });
                }
            }

            let supported_names: Vec<&str> = SUPPORTED_TERMINALS.iter().map(|s| s.binary).collect();
            Err(AppError::Validation(format!(
                "No supported terminal emulator was found on PATH (searched: {}). Install one of these terminal emulators or configure a custom terminal command in Terminal Settings.",
                supported_names.join(", ")
            )))
        }
        TerminalPreferenceMode::Preset => {
            let binary = preference
                .preset_binary
                .as_deref()
                .map(str::trim)
                .unwrap_or_default();

            let spec = SUPPORTED_TERMINALS
                .iter()
                .find(|s| s.binary == binary)
                .ok_or_else(|| {
                    AppError::Validation(format!(
                        "Unsupported terminal preset '{}'. Choose a supported terminal or use Custom mode.",
                        binary
                    ))
                })?;

            if let Some(path) = resolve_executable_in_paths(spec.binary, search_dirs) {
                Ok(ResolvedTerminal {
                    binary_label: spec.binary.to_string(),
                    executable_path: path,
                    exec_prefix_args: spec
                        .exec_prefix_args
                        .iter()
                        .map(|s| (*s).to_string())
                        .collect(),
                })
            } else {
                let installed: Vec<String> = discover_terminals_in_paths(search_dirs)
                    .into_iter()
                    .filter(|t| t.installed)
                    .map(|t| t.binary)
                    .collect();
                let suggestion = if installed.is_empty() {
                    "No supported terminals were detected on PATH.".to_string()
                } else {
                    format!("Available detected terminals: {}.", installed.join(", "))
                };
                Err(AppError::Validation(format!(
                    "Configured terminal emulator '{}' is not installed or not found on PATH. {} Update your selection in Terminal Settings.",
                    spec.binary, suggestion
                )))
            }
        }
        TerminalPreferenceMode::Custom => {
            let raw_exec = preference
                .custom_executable
                .as_deref()
                .map(str::trim)
                .unwrap_or_default();

            let path = resolve_executable_in_paths(raw_exec, search_dirs).ok_or_else(|| {
                AppError::Validation(format!(
                    "Configured custom terminal executable '{}' was not found or is not executable. Verify the binary path in Terminal Settings.",
                    raw_exec
                ))
            })?;

            let binary_label = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| raw_exec.to_string());

            let cleaned_prefix: Vec<String> = preference
                .custom_args_prefix
                .iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            let exec_prefix_args = if !cleaned_prefix.is_empty() {
                cleaned_prefix
            } else if let Some(known) = SUPPORTED_TERMINALS
                .iter()
                .find(|s| s.binary == binary_label)
            {
                known
                    .exec_prefix_args
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect()
            } else {
                vec!["-e".to_string()]
            };

            Ok(ResolvedTerminal {
                binary_label,
                executable_path: path,
                exec_prefix_args,
            })
        }
    }
}

/// Constructs distinct OpenSSH command-line arguments for a connection profile and optional remote directory.
///
/// Security invariants:
/// - Passes `-p <port>`, `-l <username>`, optional `-i <identity_file>`, and `<host>` as distinct arguments.
/// - If `remote_directory` is specified, adds `-t` for interactive pseudo-terminal allocation
///   and a POSIX single-quoted `cd -- <dir> 2>/dev/null || cd ~; exec ${SHELL:-/bin/bash} -l` command.
/// - Never includes passwords, passphrases, or secret tokens in arguments.
/// - Rejects values starting with `-` or containing whitespace/control characters to prevent option injection.
/// - Preserves the user's normal `~/.ssh/config` and `~/.ssh/known_hosts` behavior.
pub fn build_ssh_args_with_directory(
    profile: &ConnectionProfile,
    remote_directory: Option<&str>,
) -> Result<Vec<String>, AppError> {
    profile.validate()?;

    match profile.protocol {
        ProtocolType::Sftp | ProtocolType::Scp => {}
        ProtocolType::Ftp | ProtocolType::Ftps => {
            return Err(AppError::Validation(format!(
                "External SSH terminal is only available for SSH-based connections (SFTP/SCP), not {}.",
                profile.protocol.as_str()
            )));
        }
    }

    let host = profile.host.trim();
    if host.is_empty() {
        return Err(AppError::Validation("Hostname cannot be empty".to_string()));
    }
    if host.starts_with('-') {
        return Err(AppError::Validation(
            "Hostname must not start with '-' (SSH option injection refused)".to_string(),
        ));
    }
    if host.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(AppError::Validation(
            "Hostname must not contain whitespace or control characters".to_string(),
        ));
    }

    let username = profile.username.trim();
    if username.is_empty() {
        return Err(AppError::Validation("Username cannot be empty".to_string()));
    }
    if username.starts_with('-') {
        return Err(AppError::Validation(
            "Username must not start with '-' (SSH option injection refused)".to_string(),
        ));
    }
    if username
        .chars()
        .any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(AppError::Validation(
            "Username must not contain whitespace or control characters".to_string(),
        ));
    }

    let mut args = Vec::new();

    let target_dir = remote_directory
        .map(str::trim)
        .filter(|d| !d.is_empty() && *d != "~");

    if target_dir.is_some() {
        args.push("-t".to_string());
    }

    args.push("-p".to_string());
    args.push(profile.port.to_string());
    args.push("-l".to_string());
    args.push(username.to_string());

    if let AuthMethod::PublicKey {
        ref private_key_path,
        ..
    } = profile.auth_method
    {
        let raw_key = private_key_path.trim();
        if raw_key.is_empty() {
            return Err(AppError::Validation(
                "Private key file path cannot be empty for public key authentication".to_string(),
            ));
        }
        if raw_key.starts_with('-') {
            return Err(AppError::Validation(
                "Private key file path must not start with '-'".to_string(),
            ));
        }
        if raw_key.chars().any(char::is_control) {
            return Err(AppError::Validation(
                "Private key file path must not contain control characters".to_string(),
            ));
        }

        let expanded_key = shellexpand::tilde(raw_key).into_owned();
        if expanded_key.starts_with('-') {
            return Err(AppError::Validation(
                "Expanded private key file path must not start with '-'".to_string(),
            ));
        }

        args.push("-i".to_string());
        args.push(expanded_key);
    }

    args.push(host.to_string());

    if let Some(dir) = target_dir {
        let escaped_dir = format!("'{}'", dir.replace('\'', r"'\''"));
        let remote_cmd = format!(
            "cd -- {} 2>/dev/null || cd ~; exec ${{SHELL:-/bin/bash}} -l",
            escaped_dir
        );
        args.push(remote_cmd);
    }

    Ok(args)
}

pub fn build_ssh_args(profile: &ConnectionProfile) -> Result<Vec<String>, AppError> {
    build_ssh_args_with_directory(profile, None)
}

/// Builds a complete, verified launch plan combining the resolved terminal emulator, OpenSSH arguments, and optional remote directory.
pub fn build_terminal_launch_plan_with_directory(
    profile: &ConnectionProfile,
    preference: &TerminalPreference,
    search_dirs: &[PathBuf],
    remote_directory: Option<&str>,
) -> Result<TerminalLaunchPlan, AppError> {
    let ssh_args = build_ssh_args_with_directory(profile, remote_directory)?;
    let resolved_terminal = resolve_terminal_for_preference(preference, search_dirs)?;
    let ssh_path = resolve_ssh_binary_in_paths(search_dirs)?;
    let ssh_executable = ssh_path.to_string_lossy().into_owned();

    let mut terminal_args = resolved_terminal.exec_prefix_args.clone();
    terminal_args.push(ssh_executable.clone());
    terminal_args.extend(ssh_args.clone());

    Ok(TerminalLaunchPlan {
        terminal_binary: resolved_terminal.binary_label,
        terminal_executable: resolved_terminal.executable_path,
        terminal_args,
        ssh_executable,
        ssh_args,
        remote_directory: remote_directory
            .map(|d| d.trim().to_string())
            .filter(|d| !d.is_empty()),
    })
}

pub fn build_terminal_launch_plan(
    profile: &ConnectionProfile,
    preference: &TerminalPreference,
    search_dirs: &[PathBuf],
) -> Result<TerminalLaunchPlan, AppError> {
    build_terminal_launch_plan_with_directory(profile, preference, search_dirs, None)
}

/// Spawns the external terminal emulator executing the OpenSSH session using direct `Command` argv.
/// Never invokes a shell (`sh -c`) and never passes credentials in arguments or environment.
pub fn spawn_terminal_plan(
    profile: &ConnectionProfile,
    plan: &TerminalLaunchPlan,
) -> Result<TerminalLaunchResult, AppError> {
    tracing::info!(
        profile_id = %profile.id,
        terminal = %plan.terminal_binary,
        host = %profile.host.trim(),
        port = profile.port,
        username = %profile.username.trim(),
        "Spawning external SSH terminal process"
    );

    let mut child = Command::new(&plan.terminal_executable)
        .args(&plan.terminal_args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            AppError::Filesystem(format!(
                "Failed to launch terminal '{}': {}",
                plan.terminal_executable.display(),
                e
            ))
        })?;

    let pid = Some(child.id());

    // Reap the child process asynchronously when the terminal closes to prevent <defunct> zombies
    std::thread::spawn(move || {
        let _ = child.wait();
    });

    let elevation_notice = if profile.sudo_sftp.enabled {
        " Note: This terminal opens an independent SSH shell as the login user, separate from any sudo-elevated SFTP channel."
    } else {
        ""
    };

    let dir_notice = if let Some(ref dir) = plan.remote_directory {
        format!(" in '{}'", dir)
    } else {
        String::new()
    };

    let message = format!(
        "Launched '{}' starting OpenSSH for {}@{}:{}{}. Complete any required authentication or host-key prompt inside the terminal window.{}",
        plan.terminal_binary,
        profile.username.trim(),
        profile.host.trim(),
        profile.port,
        dir_notice,
        elevation_notice
    );

    Ok(TerminalLaunchResult {
        profile_id: profile.id.clone(),
        profile_name: profile.name.clone(),
        host: profile.host.trim().to_string(),
        port: profile.port,
        username: profile.username.trim().to_string(),
        terminal_binary: plan.terminal_binary.clone(),
        pid,
        status: TerminalLaunchStatus::ProcessSpawned,
        message,
    })
}

/// Collects the current terminal environment information for the settings UI.
pub fn inspect_terminal_environment(
    preference: TerminalPreference,
    search_dirs: &[PathBuf],
) -> TerminalEnvironmentInfo {
    let supported_terminals = discover_terminals_in_paths(search_dirs);
    let detected_default = supported_terminals.iter().find(|t| t.installed).cloned();
    let ssh_binary_path = resolve_ssh_binary_in_paths(search_dirs)
        .ok()
        .map(|p| p.to_string_lossy().into_owned());

    TerminalEnvironmentInfo {
        preference,
        supported_terminals,
        detected_default,
        ssh_binary_path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::os::unix::fs::PermissionsExt;

    fn create_temp_bin_dir(prefix: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("linuxscp-{}-{}", prefix, uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("Should create temp bin dir");
        dir
    }

    fn create_fake_executable(dir: &Path, name: &str, executable: bool) -> PathBuf {
        let path = dir.join(name);
        File::create(&path).expect("Should create file");
        let mode = if executable { 0o755 } else { 0o644 };
        fs::set_permissions(&path, fs::Permissions::from_mode(mode))
            .expect("Should set permissions");
        path
    }

    #[test]
    fn test_build_ssh_args_password_profile_distinct_args_and_no_secrets() {
        let mut profile = ConnectionProfile::new(
            "Ubuntu Test".to_string(),
            ProtocolType::Sftp,
            "192.168.122.10".to_string(),
            "ubuntu".to_string(),
            AuthMethod::Password,
        );
        profile.port = 2222;

        let args = build_ssh_args(&profile).expect("Should construct ssh args");
        assert_eq!(args, vec!["-p", "2222", "-l", "ubuntu", "192.168.122.10"]);
        assert!(!args.iter().any(|a| a.contains("ubuntu@192.168.122.10")));
        assert!(!args.iter().any(|a| a.contains("-i")));
    }

    #[test]
    fn test_build_ssh_args_public_key_includes_distinct_identity_file_option() {
        let mut profile = ConnectionProfile::new(
            "Key Server".to_string(),
            ProtocolType::Sftp,
            "10.0.0.42".to_string(),
            "deploy".to_string(),
            AuthMethod::PublicKey {
                private_key_path: "/home/testuser/my keys/id_ed25519".to_string(),
                requires_passphrase: true,
            },
        );
        profile.port = 2200;

        let args = build_ssh_args(&profile).expect("Should construct ssh args with identity file");
        assert_eq!(
            args,
            vec![
                "-p",
                "2200",
                "-l",
                "deploy",
                "-i",
                "/home/testuser/my keys/id_ed25519",
                "10.0.0.42"
            ]
        );
    }

    #[test]
    fn test_build_ssh_args_expands_tilde_in_identity_file() {
        let profile = ConnectionProfile::new(
            "Tilde Key Server".to_string(),
            ProtocolType::Sftp,
            "192.168.122.10".to_string(),
            "ubuntu".to_string(),
            AuthMethod::PublicKey {
                private_key_path: "~/.ssh/id_rsa".to_string(),
                requires_passphrase: false,
            },
        );

        let args = build_ssh_args(&profile).expect("Should expand tilde in key path");
        assert_eq!(args[0], "-p");
        assert_eq!(args[1], "22");
        assert_eq!(args[2], "-l");
        assert_eq!(args[3], "ubuntu");
        assert_eq!(args[4], "-i");
        assert!(
            !args[5].starts_with('~'),
            "Tilde should be expanded: {}",
            args[5]
        );
        assert!(args[5].ends_with("/.ssh/id_rsa"));
        assert_eq!(args[6], "192.168.122.10");
    }

    #[test]
    fn test_build_ssh_args_with_remote_directory() {
        let profile = ConnectionProfile::new(
            "Ubuntu Test".to_string(),
            ProtocolType::Sftp,
            "192.168.122.10".to_string(),
            "ubuntu".to_string(),
            AuthMethod::Password,
        );

        // 1. With standard directory path
        let args = build_ssh_args_with_directory(&profile, Some("/var/www"))
            .expect("Should construct ssh args with remote directory");
        assert_eq!(args[0], "-t");
        assert_eq!(args[1], "-p");
        assert_eq!(args[2], "22");
        assert_eq!(args[3], "-l");
        assert_eq!(args[4], "ubuntu");
        assert_eq!(args[5], "192.168.122.10");
        assert_eq!(
            args[6],
            "cd -- '/var/www' 2>/dev/null || cd ~; exec ${SHELL:-/bin/bash} -l"
        );

        // 2. With path containing single quotes and spaces
        let args_quotes = build_ssh_args_with_directory(&profile, Some("/home/ubuntu/it's a test"))
            .expect("Should escape single quotes in path");
        assert_eq!(args_quotes[0], "-t");
        assert_eq!(
            args_quotes[6],
            "cd -- '/home/ubuntu/it'\\''s a test' 2>/dev/null || cd ~; exec ${SHELL:-/bin/bash} -l"
        );

        // 3. With home or empty directory (should not add -t or cd command)
        let args_home = build_ssh_args_with_directory(&profile, Some("~"))
            .expect("Should not add cd for home dir");
        assert_eq!(
            args_home,
            vec!["-p", "22", "-l", "ubuntu", "192.168.122.10"]
        );

        let args_empty = build_ssh_args_with_directory(&profile, Some("   "))
            .expect("Should not add cd for empty dir");
        assert_eq!(
            args_empty,
            vec!["-p", "22", "-l", "ubuntu", "192.168.122.10"]
        );
    }

    #[test]
    fn test_build_ssh_args_rejects_option_injection_and_non_ssh_protocols() {
        let mut bad_host = ConnectionProfile::new(
            "Injected Host".to_string(),
            ProtocolType::Sftp,
            "-oProxyCommand=evil".to_string(),
            "ubuntu".to_string(),
            AuthMethod::Agent,
        );
        assert!(build_ssh_args(&bad_host).is_err());

        bad_host.host = "192.168.122.10".to_string();
        bad_host.username = "-oProxyCommand=evil".to_string();
        assert!(build_ssh_args(&bad_host).is_err());

        let bad_key = ConnectionProfile::new(
            "Injected Key".to_string(),
            ProtocolType::Sftp,
            "192.168.122.10".to_string(),
            "ubuntu".to_string(),
            AuthMethod::PublicKey {
                private_key_path: "-oProxyCommand=evil".to_string(),
                requires_passphrase: false,
            },
        );
        assert!(build_ssh_args(&bad_key).is_err());

        let ftp_profile = ConnectionProfile::new(
            "FTP Server".to_string(),
            ProtocolType::Ftp,
            "192.168.122.10".to_string(),
            "ubuntu".to_string(),
            AuthMethod::Password,
        );
        assert!(build_ssh_args(&ftp_profile).is_err());
    }

    #[test]
    fn test_terminal_discovery_priority_and_non_executable_filtering() {
        let dir = create_temp_bin_dir("discovery");
        // Create non-executable x-terminal-emulator (should be skipped)
        create_fake_executable(&dir, "x-terminal-emulator", false);
        // Create executable konsole and alacritty
        let konsole_path = create_fake_executable(&dir, "konsole", true);
        let _alacritty_path = create_fake_executable(&dir, "alacritty", true);
        let ssh_path = create_fake_executable(&dir, "ssh", true);

        let search_dirs = vec![dir.clone()];
        let discovered = discover_terminals_in_paths(&search_dirs);

        let xterm_emu = discovered
            .iter()
            .find(|t| t.binary == "x-terminal-emulator")
            .unwrap();
        assert!(!xterm_emu.installed);

        let konsole_info = discovered.iter().find(|t| t.binary == "konsole").unwrap();
        assert!(konsole_info.installed);
        assert_eq!(
            konsole_info.resolved_path.as_deref(),
            Some(konsole_path.to_string_lossy().as_ref())
        );

        let profile = ConnectionProfile::new(
            "Test Server".to_string(),
            ProtocolType::Sftp,
            "192.168.122.10".to_string(),
            "ubuntu".to_string(),
            AuthMethod::Agent,
        );

        let plan =
            build_terminal_launch_plan(&profile, &TerminalPreference::default(), &search_dirs)
                .expect("Should build launch plan with first installed terminal");

        assert_eq!(plan.terminal_binary, "konsole");
        assert_eq!(plan.terminal_executable, konsole_path);
        assert_eq!(
            plan.terminal_args,
            vec![
                "-e".to_string(),
                ssh_path.to_string_lossy().into_owned(),
                "-p".to_string(),
                "22".to_string(),
                "-l".to_string(),
                "ubuntu".to_string(),
                "192.168.122.10".to_string(),
            ]
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_terminal_discovery_returns_useful_error_when_none_found() {
        let empty_dir = create_temp_bin_dir("empty");
        let search_dirs = vec![empty_dir.clone()];

        let res = resolve_terminal_for_preference(&TerminalPreference::default(), &search_dirs);
        assert!(res.is_err());
        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("No supported terminal emulator was found on PATH"));
        assert!(err_msg.contains("gnome-terminal"));
        assert!(err_msg.contains("konsole"));

        let _ = fs::remove_dir_all(&empty_dir);
    }

    #[test]
    fn test_custom_terminal_preference_and_shell_concatenation_rejection() {
        let dir = create_temp_bin_dir("custom");
        let custom_bin = create_fake_executable(&dir, "my-custom-term", true);
        let ssh_bin = create_fake_executable(&dir, "ssh", true);
        let search_dirs = vec![dir.clone()];

        let pref = TerminalPreference {
            mode: TerminalPreferenceMode::Custom,
            preset_binary: None,
            custom_executable: Some("my-custom-term".to_string()),
            custom_args_prefix: vec!["--exec".to_string()],
        };

        let profile = ConnectionProfile::new(
            "Custom Term Server".to_string(),
            ProtocolType::Sftp,
            "192.168.122.10".to_string(),
            "ubuntu".to_string(),
            AuthMethod::Password,
        );

        let plan = build_terminal_launch_plan(&profile, &pref, &search_dirs)
            .expect("Should resolve custom terminal plan");
        assert_eq!(plan.terminal_executable, custom_bin);
        assert_eq!(
            plan.terminal_args,
            vec![
                "--exec".to_string(),
                ssh_bin.to_string_lossy().into_owned(),
                "-p".to_string(),
                "22".to_string(),
                "-l".to_string(),
                "ubuntu".to_string(),
                "192.168.122.10".to_string(),
            ]
        );

        // Attempting to pass a shell string with spaces or metacharacters in custom_executable must fail
        let shell_pref = TerminalPreference {
            mode: TerminalPreferenceMode::Custom,
            preset_binary: None,
            custom_executable: Some("sh -c xterm".to_string()),
            custom_args_prefix: vec![],
        };
        assert!(shell_pref.validate().is_err());

        let meta_pref = TerminalPreference {
            mode: TerminalPreferenceMode::Custom,
            preset_binary: None,
            custom_executable: Some("xterm;id".to_string()),
            custom_args_prefix: vec![],
        };
        assert!(meta_pref.validate().is_err());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_spawn_terminal_plan_captures_distinct_argv_and_honest_status() {
        let dir = create_temp_bin_dir("spawn");
        let argv_out = dir.join("captured_argv.txt");
        let wrapper_path = dir.join("fake-terminal");

        let script = format!(
            "#!/bin/sh\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\" >> \"{}\"\ndone\n",
            argv_out.display()
        );
        fs::write(&wrapper_path, script).expect("Should write wrapper script");
        fs::set_permissions(&wrapper_path, fs::Permissions::from_mode(0o755))
            .expect("Should chmod wrapper");

        let ssh_bin = create_fake_executable(&dir, "ssh", true);
        let search_dirs = vec![dir.clone()];

        let pref = TerminalPreference {
            mode: TerminalPreferenceMode::Custom,
            preset_binary: None,
            custom_executable: Some(wrapper_path.to_string_lossy().into_owned()),
            custom_args_prefix: vec!["--".to_string()],
        };

        let mut profile = ConnectionProfile::new(
            "Elevated Prod".to_string(),
            ProtocolType::Sftp,
            "192.168.122.10".to_string(),
            "ubuntu".to_string(),
            AuthMethod::PublicKey {
                private_key_path: "/tmp/my keys/id_ed25519".to_string(),
                requires_passphrase: true,
            },
        );
        profile.port = 2222;
        profile.sudo_sftp.enabled = true;

        let plan =
            build_terminal_launch_plan(&profile, &pref, &search_dirs).expect("Should build plan");
        let launch_res = spawn_terminal_plan(&profile, &plan).expect("Should spawn wrapper");

        assert_eq!(launch_res.status, TerminalLaunchStatus::ProcessSpawned);
        assert!(launch_res.pid.is_some());
        assert!(
            launch_res
                .message
                .contains("Complete any required authentication"),
            "Should not claim terminal is already authenticated"
        );
        assert!(
            launch_res.message.contains("independent SSH shell"),
            "Should clarify terminal is separate from sudo-elevated SFTP"
        );

        // Wait briefly for the spawned wrapper script to write argv_out
        for _ in 0..40 {
            if argv_out.exists() {
                if let Ok(content) = fs::read_to_string(&argv_out) {
                    if content.lines().count() >= 8 {
                        break;
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }

        let captured = fs::read_to_string(&argv_out).expect("Should read captured argv");
        let lines: Vec<&str> = captured.lines().collect();
        assert_eq!(
            lines,
            vec![
                "--",
                ssh_bin.to_string_lossy().as_ref(),
                "-p",
                "2222",
                "-l",
                "ubuntu",
                "-i",
                "/tmp/my keys/id_ed25519",
                "192.168.122.10",
            ]
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_live_terminal_ssh_invocation_against_test_server() {
        // Check if the disposable test VM (192.168.122.10:22) is reachable
        if std::net::TcpStream::connect_timeout(
            &"192.168.122.10:22".parse().unwrap(),
            std::time::Duration::from_secs(2),
        )
        .is_err()
        {
            return;
        }

        let dir = create_temp_bin_dir("live-ssh");
        let output_file = dir.join("remote_uname.txt");
        let wrapper_path = dir.join("headless-terminal-wrapper");

        // The wrapper receives: ["-e", "/usr/bin/ssh", "-p", "22", "-l", "ubuntu", "-i", "<expanded>", "192.168.122.10"]
        // It strips "-e" and executes the exact OpenSSH command with a non-interactive verification command.
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = \"-e\" ]; then shift; fi\nexec \"$@\" -o BatchMode=yes \"echo LIVE_TERMINAL_OK && id -un\" > \"{}\" 2>&1\n",
            output_file.display()
        );
        fs::write(&wrapper_path, script).expect("Should write headless terminal wrapper");
        fs::set_permissions(&wrapper_path, fs::Permissions::from_mode(0o755))
            .expect("Should chmod headless terminal wrapper");

        let pref = TerminalPreference {
            mode: TerminalPreferenceMode::Custom,
            preset_binary: None,
            custom_executable: Some(wrapper_path.to_string_lossy().into_owned()),
            custom_args_prefix: vec!["-e".to_string()],
        };

        let profile = ConnectionProfile::new(
            "Live Ubuntu VM".to_string(),
            ProtocolType::Sftp,
            "192.168.122.10".to_string(),
            "ubuntu".to_string(),
            AuthMethod::PublicKey {
                private_key_path: "~/.ssh/id_rsa".to_string(),
                requires_passphrase: false,
            },
        );

        let search_dirs = system_path_dirs();
        let plan = build_terminal_launch_plan(&profile, &pref, &search_dirs)
            .expect("Should build live launch plan");
        let res = spawn_terminal_plan(&profile, &plan).expect("Should spawn live terminal wrapper");
        assert_eq!(res.status, TerminalLaunchStatus::ProcessSpawned);

        for _ in 0..80 {
            if let Ok(content) = fs::read_to_string(&output_file) {
                if content.contains("LIVE_TERMINAL_OK") && content.contains("ubuntu") {
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        let output = fs::read_to_string(&output_file).expect("Should read remote command output");
        assert!(
            output.contains("LIVE_TERMINAL_OK"),
            "Unexpected SSH output: {}",
            output
        );
        assert!(
            output.contains("ubuntu"),
            "Expected remote user ubuntu: {}",
            output
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
