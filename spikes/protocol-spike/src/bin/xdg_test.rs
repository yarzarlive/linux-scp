use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct DesktopApp {
    pub id: String,
    pub name: String,
    pub raw_exec: String,
    pub executable: String,
    pub args_template: Vec<String>,
}

impl DesktopApp {
    pub fn parse_exec(raw: &str) -> (String, Vec<String>) {
        // Safe argument splitting respecting quotes without using a shell
        let words = shlex::split(raw).unwrap_or_default();
        if words.is_empty() {
            return (String::new(), Vec::new());
        }
        let exec = words[0].clone();
        let args = words[1..].to_vec();
        (exec, args)
    }

    pub fn build_command(&self, target_file: &str) -> Command {
        let mut cmd = Command::new(&self.executable);
        for arg in &self.args_template {
            match arg.as_str() {
                "%f" | "%F" | "%u" | "%U" => {
                    cmd.arg(target_file);
                }
                other => {
                    // Strip other field codes or pass through verbatim
                    if !other.starts_with('%') {
                        cmd.arg(other);
                    }
                }
            }
        }
        cmd
    }
}

fn main() {
    println!("=== Linux XDG Application Discovery & Safe Launch Spike ===");
    let app_dirs = [
        shellexpand::tilde("~/.local/share/applications").to_string(),
        "/usr/share/applications".to_string(),
        "/var/lib/flatpak/exports/share/applications".to_string(),
    ];

    let mut discovered_apps = Vec::new();

    for dir_str in &app_dirs {
        let dir = Path::new(dir_str);
        if !dir.exists() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("desktop") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        let mut name = None;
                        let mut exec = None;
                        let mut no_display = false;

                        for line in content.lines() {
                            let line = line.trim();
                            if line.starts_with("Name=") && name.is_none() {
                                name = Some(line.trim_start_matches("Name=").to_string());
                            } else if line.starts_with("Exec=") && exec.is_none() {
                                exec = Some(line.trim_start_matches("Exec=").to_string());
                            } else if line == "NoDisplay=true" {
                                no_display = true;
                            }
                        }

                        if !no_display {
                            if let (Some(n), Some(e)) = (name, exec) {
                                let (executable, args_template) = DesktopApp::parse_exec(&e);
                                discovered_apps.push(DesktopApp {
                                    id: path.file_name().unwrap().to_string_lossy().to_string(),
                                    name: n,
                                    raw_exec: e,
                                    executable,
                                    args_template,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    println!("Total user-facing desktop applications discovered: {}", discovered_apps.len());
    assert!(!discovered_apps.is_empty(), "Expected desktop applications!");

    println!("Sample discovered desktop applications with parsed exec arguments:");
    for app in discovered_apps.iter().take(5) {
        println!("  - [{}] {} -> binary: '{}', args: {:?}", app.id, app.name, app.executable, app.args_template);
    }

    println!("\nTesting safe command execution without shell interpolation...");
    let test_app = DesktopApp {
        id: "test.desktop".to_string(),
        name: "Echo Test App".to_string(),
        raw_exec: "echo -n %f".to_string(),
        executable: "/usr/bin/echo".to_string(),
        args_template: vec!["-n".to_string(), "%f".to_string()],
    };

    let target_file = "/tmp/test file with spaces.txt";
    let mut cmd = test_app.build_command(target_file);
    let output = cmd.output().expect("Failed to execute test command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("Execution output: '{}'", stdout);
    assert_eq!(stdout, target_file, "Argument passed correctly with spaces and without shell expansion!");

    println!("\n=== XDG Application Discovery and Safe Launch Verified Successfully ===");
}
