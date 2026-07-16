mod commands;

use commands::AppState;
use std::sync::Mutex;

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running Yntra Vault");
}
