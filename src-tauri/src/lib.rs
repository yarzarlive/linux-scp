pub mod commands;
pub mod desktop_integration;
pub mod errors;
pub mod filesystem;
pub mod logging;
pub mod models;
pub mod persistence;
pub mod protocol;
pub mod terminal;
pub mod transfer;

use std::sync::Arc;
use tauri::Manager;

use commands::desktop::{
    delete_file_app_association, discover_desktop_applications, list_file_app_associations,
    open_local_file_default, open_local_file_with_app,
};
use commands::filesystem::{
    create_local_directory, create_local_file, delete_local_entries, get_home_directory,
    list_local_directory, rename_local_entry,
};
use commands::profile::{
    check_keyring_status, delete_connection_profile, duplicate_connection_profile,
    get_connection_profile, list_connection_profiles, save_connection_profile,
};
use commands::session::{
    check_remote_path_is_dir, connect_sftp_session, create_remote_directory, create_remote_file,
    delete_remote_directory, delete_remote_file, disconnect_sftp_session, get_session_state,
    list_known_hosts, list_remote_directory, reconnect_sftp_session, rename_remote_entry,
    resolve_remote_identity, set_remote_ownership, set_remote_permissions, trust_host_key,
};
use commands::system::{get_system_info, ping_backend};
use commands::terminal::{get_terminal_environment, launch_ssh_terminal, save_terminal_preference};
use commands::transfer::{
    cancel_transfer, clear_finished_transfers, enqueue_downloads, enqueue_uploads, list_transfers,
    resolve_transfer_conflict, retry_transfer,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    logging::init_logging();
    tracing::info!("Starting LinuxSCP desktop application runtime...");

    let session_manager = protocol::SessionManager::new();
    let session_for_transfers = session_manager.clone();

    tauri::Builder::default()
        .manage(session_manager)
        .setup(move |app| {
            let sink = Arc::new(transfer::TauriTransferEventSink::new(app.handle().clone()));
            let transfer_manager = transfer::TransferManager::new(session_for_transfers, sink);
            app.manage(transfer_manager);
            Ok(())
        })
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
            connect_sftp_session,
            disconnect_sftp_session,
            reconnect_sftp_session,
            get_session_state,
            trust_host_key,
            list_known_hosts,
            list_remote_directory,
            check_remote_path_is_dir,
            create_remote_directory,
            create_remote_file,
            rename_remote_entry,
            delete_remote_file,
            delete_remote_directory,
            set_remote_permissions,
            set_remote_ownership,
            resolve_remote_identity,
            enqueue_uploads,
            enqueue_downloads,
            list_transfers,
            cancel_transfer,
            retry_transfer,
            resolve_transfer_conflict,
            clear_finished_transfers,
            get_terminal_environment,
            save_terminal_preference,
            launch_ssh_terminal,
            discover_desktop_applications,
            open_local_file_default,
            open_local_file_with_app,
            list_file_app_associations,
            delete_file_app_association,
        ])
        .run(tauri::generate_context!())
        .expect("Failed to run LinuxSCP application");
}
