mod commands;

use commands::AppState;
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState};
use tauri::{Manager, Emitter};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            vault: Mutex::new(None),
            minimize_to_tray: std::sync::atomic::AtomicBool::new(true),
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
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
                    let _ = window.emit("vault-locked", ());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            // Vault
            commands::create_vault,
            commands::open_vault,
            commands::lock_vault,
            commands::get_vault_info,
            // Entries
            commands::list_entries,
            commands::search_entries,
            commands::get_entry,
            commands::add_entry,
            commands::update_entry,
            commands::delete_entry,
            commands::toggle_favorite,
            commands::toggle_pin,
            // Trash
            commands::list_trash,
            commands::restore_from_trash,
            commands::permanent_delete,
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
            commands::check_vault_file_exists,
            commands::show_in_explorer,
            // Advanced features
            commands::autotype,
            commands::run_smart_autotype,
            commands::enable_autostart,
            commands::disable_autostart,
            commands::is_autostart_enabled,
            commands::set_minimize_to_tray,
            commands::webdav_upload,
            commands::webdav_download,
            commands::run_p2p_sync_listener,
            commands::run_p2p_sync_client,
            commands::split_master_password,
            commands::reconstruct_master_password_hash,
            // Export
            commands::export_vault,
            commands::get_vault_path,
            // Browser Extension
            commands::install_browser_extension,
        ])
        .setup(|app| {
            use tauri::{Manager, Emitter};

            // Setup System Tray Menu & Icon
            let quit_i = MenuItem::with_id(app, "quit", "Close", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
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
                .build(app)?;

            // Conditionally show main window based on launch argument
            let is_minimized = std::env::args().any(|arg| arg == "--minimized");
            if !is_minimized {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                }
            }

            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    let state = app_handle.state::<AppState>();
                    let mut vault = match state.vault.lock() {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    if let Some(ref manager) = *vault {
                        let path_str = manager.info().path;
                        let path = std::path::Path::new(&path_str);
                        if !path.exists() {
                            // Vault file was deleted or moved!
                            *vault = None;
                            let _ = app_handle.emit("vault-connection-lost", ());
                        }
                    }
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Yntra Vault");
}
