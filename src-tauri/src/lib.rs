pub mod commands;

pub use commands::AppState;
use std::sync::Mutex;
#[cfg(not(mobile))]
use tauri::menu::{Menu, MenuItem};
#[cfg(not(mobile))]
use tauri::tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState};
use tauri::{Manager, Emitter};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Disable core dumps and debugger attachment at process startup
    yntra_vault_core::crypto::prevent_core_dumps();

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            vault: Mutex::new(None),
            minimize_to_tray: std::sync::atomic::AtomicBool::new(true),
            lock_on_focus_loss: std::sync::atomic::AtomicBool::new(false),
            lock_on_system_lock: std::sync::atomic::AtomicBool::new(true),
            smart_login_cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        });

    #[cfg(not(mobile))]
    {
        builder = builder.on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::Focused(false) => {
                    let app = window.app_handle();
                    let state = app.state::<AppState>();
                    if state.lock_on_focus_loss.load(std::sync::atomic::Ordering::Relaxed) {
                        if let Ok(mut vault) = state.vault.lock() {
                            if let Some(ref mut manager) = *vault {
                                manager.lock();
                            }
                            *vault = None;
                        }
                        let _ = yntra_vault_core::crypto::clear_clipboard();
                        let _ = window.emit("vault-locked", ());
                    }
                }
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    let app = window.app_handle();
                    let state = app.state::<AppState>();
                    if state.minimize_to_tray.load(std::sync::atomic::Ordering::Relaxed) {
                        let _ = window.hide();
                        api.prevent_close();

                        // Lock the vault on close-to-tray
                        if let Ok(mut vault) = state.vault.lock() {
                            if let Some(ref mut manager) = *vault {
                                manager.lock();
                            }
                            *vault = None;
                        }
                        let _ = yntra_vault_core::crypto::clear_clipboard();
                        let _ = window.emit("vault-locked", ());
                    }
                }
                _ => {}
            }
        });
    }

    builder
        .invoke_handler(tauri::generate_handler![
            // Vault
            commands::create_vault,
            commands::open_vault,
            commands::lock_vault,
            commands::get_vault_info,
            commands::generate_key_file,
            commands::check_biometric_available,
            commands::is_biometric_enabled,
            commands::unlock_vault_biometric,
            commands::enable_biometric,
            commands::disable_biometric,
            // Hardware 2FA / YubiKey
            commands::check_hardware2fa_available,
            commands::list_hardware_keys,
            commands::is_hardware2fa_enabled,
            commands::get_hardware2fa_challenge,
            commands::open_vault_with_hardware2fa,
            commands::perform_hardware2fa_challenge,
            commands::enable_hardware2fa,
            commands::disable_hardware2fa,
            // Entries
            commands::list_entries,
            commands::search_entries,
            commands::get_entry,
            commands::add_entry,
            commands::update_entry,
            commands::update_entry_breach_status,
            commands::save_vault,
            commands::reload_vault,
            commands::delete_entry,
            commands::toggle_favorite,
            commands::toggle_pin,
            // Attachments
            commands::get_attachment_data,
            commands::add_attachment,
            commands::delete_attachment,
            // Trash
            commands::list_trash,
            commands::restore_from_trash,
            commands::permanent_delete,
            commands::empty_trash,
            commands::purge_expired_trash,
            commands::get_storage_metrics,
            commands::compact_vault,
            // Password History
            commands::get_password_history,
            // TOTP
            commands::generate_totp,
            commands::generate_totp_with_config,
            commands::parse_otpauth_uri,
            // Password Generator
            commands::generate_password,
            commands::generate_password_default,
            // Breach Detection
            commands::check_password_breach,
            commands::analyze_password_strength,
            // Security
            commands::security_audit,
            commands::change_master_password,
            // Tags
            commands::get_tags,
            commands::add_tag,
            commands::delete_tag,
            commands::update_tag,
            commands::reorder_tags,
            commands::check_vault_file_exists,
            commands::show_in_explorer,
            // Advanced features
            commands::autotype,
            commands::run_smart_autotype,
            commands::enable_autostart,
            commands::disable_autostart,
            commands::is_autostart_enabled,
            commands::get_favicon,
            commands::set_external_favicons_enabled,
            commands::is_external_favicons_enabled,
            commands::set_minimize_to_tray,
            commands::webdav_upload,
            commands::webdav_download,
            commands::webdav_sync,
            commands::webdav_test_connection,
            commands::run_p2p_sync_listener,
            commands::run_p2p_sync_client,
            commands::get_local_ip,
            commands::scan_p2p_discovery,
            commands::generate_pairing_code,
            commands::get_trusted_devices,
            commands::revoke_trusted_device,
            commands::start_pairing_host,
            commands::start_pairing_client,
            commands::scan_pairing_discovery,
            commands::split_master_password,
            commands::reconstruct_master_password,
            commands::reconstruct_master_password_hash,
            commands::generate_emergency_kit,
            commands::get_emergency_kit_audit,
            commands::reset_emergency_kit_audit,
            // Export & Import
            commands::export_vault,
            commands::export_vault_csv,
            commands::export_vault_json,
            commands::get_vault_path,
            commands::query_mobile_autofill_status,
            commands::get_autofill_credentials_for_package,
            commands::parse_import_file,
            commands::parse_import_content,
            commands::import_entries,
            // Clipboard Defense & Native Zero-Disclosure IPC
            commands::copy_to_clipboard,
            commands::clear_clipboard,
            commands::copy_entry_password,
            commands::copy_entry_username,
            commands::copy_entry_totp,
            commands::create_vault_bytes,
            commands::open_vault_bytes,
            commands::change_master_password_bytes,
            commands::autotype_entry_password,
            commands::autotype_entry_smart,
            commands::verify_biometric_2fa,
            commands::get_installed_apps,
            commands::set_window_capture_protection,
            commands::set_lock_on_focus_loss,
            commands::set_lock_on_system_lock,
            // Smart Login (CDP browser automation)
            commands::smart_login_precheck,
            commands::smart_login_close_browser,
            commands::smart_login_start,
            commands::smart_login_cancel,
        ])
        .setup(|app| {
            use tauri::{Manager, Emitter};

            #[cfg(target_os = "windows")]
            {
                // Enforce Window Capture Protection (WDA_EXCLUDEFROMCAPTURE) against screen scraping malware
                if let Some(window) = app.get_webview_window("main") {
                    if let Ok(hwnd) = window.hwnd() {
                        let _ = yntra_vault_core::crypto::set_window_capture_protection(hwnd.0 as isize, true);
                    }
                }
            }

            #[cfg(not(mobile))]
            {
                // Setup System Tray Menu & Icon on desktop platforms
                if let Ok(quit_i) = MenuItem::with_id(app, "quit", "Close", true, None::<&str>) {

                    if let Ok(show_i) = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>) {
                        if let Ok(menu) = Menu::with_items(app, &[&show_i, &quit_i]) {
                            let mut tray_builder = TrayIconBuilder::new()
                                .menu(&menu)
                                .show_menu_on_left_click(false);

                            if let Some(icon) = app.default_window_icon() {
                                tray_builder = tray_builder.icon(icon.clone());
                            }

                            let _ = tray_builder
                                .on_menu_event(|app, event| {
                                    match event.id.as_ref() {
                                        "quit" => {
                                            app.exit(0);
                                        }
                                        "show" => {
                                            if let Some(window) = app.get_webview_window("main") {
                                                let _ = window.show();
                                                let _ = window.set_focus();
                                            }
                                        }
                                        _ => {}
                                    }
                                })
                                .on_tray_icon_event(|tray, event| {
                                    if let TrayIconEvent::Click { button, button_state, .. } = event {
                                        if button == MouseButton::Left && button_state == MouseButtonState::Up {
                                            let app = tray.app_handle();
                                            if let Some(window) = app.get_webview_window("main") {
                                                if window.is_visible().unwrap_or(false) {
                                                    let _ = window.hide();
                                                } else {
                                                    let _ = window.show();
                                                    let _ = window.set_focus();
                                                }
                                            }
                                        }
                                    }
                                })
                                .build(app);
                        }
                    }
                }

                // Conditionally show main window based on launch argument
                let is_minimized = std::env::args().any(|arg| arg == "--minimized");
                if !is_minimized {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                    }
                }
            }

            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    let state = app_handle.state::<AppState>();

                    // Aggressive Auto-Lock: Check OS Workstation Lock / Screen Lock / Sleep
                    if state.lock_on_system_lock.load(std::sync::atomic::Ordering::Relaxed) {
                        if yntra_vault_core::crypto::is_workstation_locked() {
                            if let Ok(mut vault) = state.vault.lock() {
                                if vault.is_some() {
                                    if let Some(ref mut manager) = *vault {
                                        manager.lock();
                                    }
                                    *vault = None;
                                    let _ = yntra_vault_core::crypto::clear_clipboard();
                                    let _ = app_handle.emit("vault-locked", ());
                                }
                            }
                        }
                    }

                    // Extract path while holding lock briefly, then check filesystem outside lock
                    let vault_path_str = {
                        let vault = match state.vault.lock() {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        match *vault {
                            Some(ref manager) => Some(manager.info().path),
                            None => None,
                        }
                    };
                    // Filesystem check outside mutex scope
                    if let Some(path_str) = vault_path_str {
                        let path = std::path::Path::new(&path_str);
                        if !path.exists() {
                            let tmp_path = path.with_extension("vdb.tmp");
                            if !tmp_path.exists() {
                                std::thread::sleep(std::time::Duration::from_millis(200));
                                if !path.exists() && !tmp_path.exists() {
                                    if let Ok(mut vault) = state.vault.lock() {
                                        *vault = None;
                                    }
                                    let _ = app_handle.emit("vault-connection-lost", ());
                                }
                            }
                        }
                    }
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Yntra Vault");
}

