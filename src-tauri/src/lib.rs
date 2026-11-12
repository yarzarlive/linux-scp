pub mod commands;
pub mod errors;
pub mod logging;
pub mod models;
pub mod protocol;

use commands::system::{get_system_info, ping_backend};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    logging::init_logging();
    tracing::info!("Starting LinuxSCP desktop application runtime...");

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_system_info, ping_backend,])
        .run(tauri::generate_context!())
        .expect("Failed to run LinuxSCP application");
}
