pub mod commands;
pub mod errors;
pub mod filesystem;
pub mod logging;
pub mod models;
pub mod protocol;

use commands::filesystem::{
    create_local_directory, create_local_file, delete_local_entries, get_home_directory,
    list_local_directory, rename_local_entry,
};
use commands::system::{get_system_info, ping_backend};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    logging::init_logging();
    tracing::info!("Starting LinuxSCP desktop application runtime...");

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_system_info,
            ping_backend,
            list_local_directory,
            get_home_directory,
            create_local_directory,
            create_local_file,
            rename_local_entry,
            delete_local_entries,
        ])
        .run(tauri::generate_context!())
        .expect("Failed to run LinuxSCP application");
}
