use std::fs;
use std::path::Path;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct LocalItemInfo {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct ProbeResult {
    pub success: bool,
    pub platform: String,
    pub message: String,
    pub sample_items: Vec<LocalItemInfo>,
}

#[tauri::command]
fn probe_environment(dir_path: String) -> Result<ProbeResult, String> {
    let path = Path::new(&dir_path);
    if !path.exists() {
        return Err(format!("Directory path does not exist: {}", dir_path));
    }

    let mut items = Vec::new();
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten().take(5) {
            let metadata = entry.metadata().ok();
            items.push(LocalItemInfo {
                name: entry.file_name().to_string_lossy().to_string(),
                is_dir: metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                size: metadata.as_ref().map(|m| m.len()).unwrap_or(0),
            });
        }
    }

    Ok(ProbeResult {
        success: true,
        platform: std::env::consts::OS.to_string(),
        message: format!("Successfully read {} entries from {}", items.len(), dir_path),
        sample_items: items,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![probe_environment])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probe_environment() {
        let res = probe_environment("/usr".to_string()).expect("Should probe /usr");
        assert!(res.success);
        assert_eq!(res.platform, "linux");
        assert!(!res.sample_items.is_empty());
    }
}
