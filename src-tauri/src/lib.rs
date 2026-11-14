pub mod commands;
pub mod desktop_integration;
pub mod errors;
pub mod filesystem;
pub mod logging;
pub mod models;
pub mod persistence;
pub mod protocol;
pub mod remote_edit;
pub mod terminal;
pub mod transfer;

use std::sync::Arc;
use std::time::Duration;
use tauri::Manager;

use commands::desktop::{
    delete_file_app_association, discard_remote_edit_session, discover_desktop_applications,
    get_remote_edit_upload_mode, keep_local_remote_edit_changes, list_file_app_associations,
    list_remote_edit_sessions, open_local_file_default, open_local_file_with_app,
    open_remote_file_default, open_remote_file_with_app, reload_remote_edit_from_remote,
    save_remote_edit_local_copy, scan_remote_edit_changes, set_remote_edit_upload_mode,
    upload_remote_edit_session,
};
use commands::filesystem::{
    create_local_directory, create_local_file, delete_local_entries, get_home_directory,
    get_saved_paths, list_local_directory, rename_local_entry, save_saved_paths,
};
use commands::profile::{
    check_keyring_status, delete_connection_profile, duplicate_connection_profile,
    export_connection_profiles, get_connection_profile, import_connection_profiles,
    list_connection_profiles, save_connection_profile,
};
use commands::session::{
    check_remote_path_is_dir, connect_sftp_session, create_remote_directory, create_remote_file,
    delete_remote_directory, delete_remote_file, disconnect_sftp_session, get_session_state,
    list_known_hosts, list_remote_directory, reconnect_sftp_session, rename_remote_entry,
    resolve_remote_identity, set_active_session, set_remote_ownership, set_remote_permissions,
    trust_host_key,
};
use commands::system::{get_system_info, get_theme_preference, ping_backend, set_theme_preference};
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
    let session_for_remote_edit = session_manager.clone();

    tauri::Builder::default()
        .manage(session_manager)
        .setup(move |app| {
            let sink = Arc::new(transfer::TauriTransferEventSink::new(app.handle().clone()));
            let transfer_manager = transfer::TransferManager::new(session_for_transfers, sink);
            app.manage(transfer_manager);

            let edit_root = remote_edit::default_workspace_root()
                .unwrap_or_else(|_| std::env::temp_dir().join("linuxscp").join("remote-edits"));
            let edit_backend = remote_edit::SftpRemoteEditBackend::new(session_for_remote_edit);
            let edit_sink = Arc::new(remote_edit::TauriRemoteEditEventSink::new(
                app.handle().clone(),
            ));
            let initial_upload_mode = persistence::Database::open_default()
                .ok()
                .and_then(|db| {
                    persistence::PreferencesRepository::new(&db)
                        .get_remote_edit_upload_mode()
                        .ok()
                })
                .unwrap_or_default();

            let edit_manager = remote_edit::RemoteEditManager::new_with_mode(
                edit_backend,
                edit_sink,
                edit_root,
                remote_edit::RemoteEditConfig::default(),
                initial_upload_mode,
            );
            let edit_for_restore = edit_manager.clone();
            tauri::async_runtime::spawn(async move {
                let _ = edit_for_restore.restore_from_disk().await;
                let _ = edit_for_restore.scan_local_changes(false).await;
            });
            remote_edit::spawn_background_watcher(edit_manager.clone(), Duration::from_millis(800));
            app.manage(edit_manager);

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                let mgr = window
                    .state::<remote_edit::RemoteEditManager>()
                    .inner()
                    .clone();
                tauri::async_runtime::spawn(async move {
                    let _ = mgr.cleanup_on_exit().await;
                });
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_system_info,
            ping_backend,
            get_theme_preference,
            set_theme_preference,
            list_local_directory,
            get_home_directory,
            create_local_directory,
            create_local_file,
            rename_local_entry,
            delete_local_entries,
            get_saved_paths,
            save_saved_paths,
            list_connection_profiles,
            get_connection_profile,
            save_connection_profile,
            duplicate_connection_profile,
            delete_connection_profile,
            export_connection_profiles,
            import_connection_profiles,
            check_keyring_status,
            connect_sftp_session,
            disconnect_sftp_session,
            reconnect_sftp_session,
            get_session_state,
            set_active_session,
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
            open_remote_file_default,
            open_remote_file_with_app,
            list_remote_edit_sessions,
            scan_remote_edit_changes,
            upload_remote_edit_session,
            keep_local_remote_edit_changes,
            save_remote_edit_local_copy,
            discard_remote_edit_session,
            reload_remote_edit_from_remote,
            get_remote_edit_upload_mode,
            set_remote_edit_upload_mode,
        ])
        .run(tauri::generate_context!())
        .expect("Failed to run LinuxSCP application");
}
