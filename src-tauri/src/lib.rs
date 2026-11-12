pub mod commands;
pub mod errors;
pub mod filesystem;
pub mod logging;
pub mod models;
pub mod persistence;
pub mod protocol;

use commands::filesystem::{
    create_local_directory, create_local_file, delete_local_entries, get_home_directory,
    list_local_directory, rename_local_entry,
};
use commands::profile::{
    check_keyring_status, delete_connection_profile, duplicate_connection_profile,
    get_connection_profile, list_connection_profiles, save_connection_profile,
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
            list_connection_profiles,
            get_connection_profile,
            save_connection_profile,
            duplicate_connection_profile,
            delete_connection_profile,
            check_keyring_status,
        ])
        .run(tauri::generate_context!())
        .expect("Failed to run LinuxSCP application");
}
