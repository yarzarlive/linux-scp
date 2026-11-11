use std::fs;
use std::path::Path;

fn main() {
    println!("=== Linux XDG Application Discovery Spike ===");
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
                                discovered_apps.push((n, e, path.file_name().unwrap().to_string_lossy().to_string()));
                            }
                        }
                    }
                }
            }
        }
    }

    println!("Total user-facing desktop applications discovered: {}", discovered_apps.len());
    println!("Sample discovered desktop applications:");
    for (name, exec, desktop_id) in discovered_apps.iter().take(8) {
        println!("  - [{}] {} (Exec: {})", desktop_id, name, exec);
    }

    assert!(!discovered_apps.is_empty(), "Expected at least some desktop applications!");
    println!("\n=== XDG Application Discovery Verified Successfully ===");
}
