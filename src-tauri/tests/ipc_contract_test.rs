//! Automated tests for Tauri 2 Security Capabilities and IPC Type Contract enforcement.
//!
//! Ensures that:
//! 1. `capabilities/default.json` and `tauri.conf.json` strictly declare valid ACL permissions.
//! 2. Every Tauri command registered in `lib.rs` is accounted for in `src/types/ipc.ts`.

use std::fs;
use std::path::PathBuf;

fn get_workspace_root() -> PathBuf {
    // Current test file runs in src-tauri
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if manifest_dir.join("capabilities").exists() {
        manifest_dir
    } else {
        manifest_dir.join("src-tauri")
    }
}

#[test]
fn test_tauri2_capabilities_declaration() {
    let tauri_dir = get_workspace_root();
    let cap_file = tauri_dir.join("capabilities").join("default.json");
    assert!(
        cap_file.exists(),
        "capabilities/default.json must exist in src-tauri"
    );

    let content = fs::read_to_string(&cap_file).expect("Failed to read capabilities/default.json");
    let json: serde_json::Value =
        serde_json::from_str(&content).expect("capabilities/default.json must be valid JSON");

    // Check identifier
    assert_eq!(
        json.get("identifier").and_then(|v| v.as_str()),
        Some("default"),
        "Capability identifier must be 'default'"
    );

    // Check local restriction
    assert_eq!(
        json.get("local").and_then(|v| v.as_bool()),
        Some(true),
        "Capability must be explicitly set to local: true"
    );

    // Check windows target
    let windows = json.get("windows").and_then(|v| v.as_array()).expect("windows array required");
    assert!(
        windows.iter().any(|w| w.as_str() == Some("main")),
        "Capability must target the 'main' window"
    );

    // Check required granular permissions
    let perms = json.get("permissions").and_then(|v| v.as_array()).expect("permissions array required");
    let perm_strings: Vec<&str> = perms.iter().filter_map(|p| p.as_str()).collect();

    let required_perms = [
        "core:default",
        "core:event:default",
        "clipboard-manager:default",
        "clipboard-manager:allow-write-text",
        "clipboard-manager:allow-clear",
        "dialog:default",
        "dialog:allow-open",
        "dialog:allow-save",
        "notification:default",
        "notification:allow-notify",
        "shell:default",
        "shell:allow-open",
    ];

    for req in required_perms {
        assert!(
            perm_strings.contains(&req),
            "Missing required capability permission: {}",
            req
        );
    }

    // Security invariant: allow-read-text must NOT be granted to prevent clipboard snooping
    assert!(
        !perm_strings.contains(&"clipboard-manager:allow-read-text"),
        "clipboard-manager:allow-read-text must not be permitted in default capabilities"
    );
}

#[test]
fn test_tauri_conf_security_capabilities_binding() {
    let tauri_dir = get_workspace_root();
    let conf_file = tauri_dir.join("tauri.conf.json");
    assert!(conf_file.exists(), "tauri.conf.json must exist in src-tauri");

    let content = fs::read_to_string(&conf_file).expect("Failed to read tauri.conf.json");
    let json: serde_json::Value =
        serde_json::from_str(&content).expect("tauri.conf.json must be valid JSON");

    let security = json
        .get("app")
        .and_then(|a| a.get("security"))
        .expect("app.security must be defined");

    let caps = security
        .get("capabilities")
        .and_then(|c| c.as_array())
        .expect("app.security.capabilities must be an array");

    assert!(
        caps.iter().any(|c| c.as_str() == Some("default")),
        "app.security.capabilities must include 'default'"
    );
}

#[test]
fn test_ipc_schema_contract_covers_all_commands() {
    let tauri_dir = get_workspace_root();
    let workspace_root = tauri_dir.parent().unwrap_or(&tauri_dir);
    let ipc_ts_file = workspace_root.join("src").join("types").join("ipc.ts");

    assert!(
        ipc_ts_file.exists(),
        "src/types/ipc.ts must exist to enforce IPC schema contract"
    );

    let ts_content = fs::read_to_string(&ipc_ts_file).expect("Failed to read src/types/ipc.ts");

    // All 70+ commands registered in tauri::generate_handler! in lib.rs
    let registered_commands = [
        // Vault
        "create_vault",
        "open_vault",
        "lock_vault",
        "get_vault_info",
        "generate_key_file",
        "check_biometric_available",
        "is_biometric_enabled",
        "unlock_vault_biometric",
        "enable_biometric",
        "disable_biometric",
        // Hardware 2FA / YubiKey
        "check_hardware2fa_available",
        "list_hardware_keys",
        "is_hardware2fa_enabled",
        "get_hardware2fa_challenge",
        "open_vault_with_hardware2fa",
        "perform_hardware2fa_challenge",
        "enable_hardware2fa",
        "disable_hardware2fa",
        // Entries
        "list_entries",
        "search_entries",
        "get_entry",
        "add_entry",
        "update_entry",
        "update_entry_breach_status",
        "save_vault",
        "reload_vault",
        "delete_entry",
        "toggle_favorite",
        "toggle_pin",
        // Attachments
        "get_attachment_data",
        "add_attachment",
        "delete_attachment",
        // Trash
        "list_trash",
        "restore_from_trash",
        "permanent_delete",
        "empty_trash",
        "purge_expired_trash",
        "get_storage_metrics",
        "compact_vault",
        // Password History
        "get_password_history",
        // TOTP
        "generate_totp",
        "generate_totp_with_config",
        "parse_otpauth_uri",
        // Password Generator
        "generate_password",
        "generate_password_default",
        // Breach Detection
        "check_password_breach",
        "analyze_password_strength",
        // Security
        "security_audit",
        "change_master_password",
        // Tags
        "get_tags",
        "add_tag",
        "delete_tag",
        "update_tag",
        "reorder_tags",
        "check_vault_file_exists",
        "show_in_explorer",
        // Advanced features
        "autotype",
        "run_smart_autotype",
        "enable_autostart",
        "disable_autostart",
        "is_autostart_enabled",
        "get_favicon",
        "set_external_favicons_enabled",
        "is_external_favicons_enabled",
        "set_minimize_to_tray",
        "webdav_upload",
        "webdav_download",
        "webdav_sync",
        "webdav_test_connection",
        "run_p2p_sync_listener",
        "run_p2p_sync_client",
        "split_master_password",
        "reconstruct_master_password",
        "reconstruct_master_password_hash",
        "generate_emergency_kit",
        "get_emergency_kit_audit",
        "reset_emergency_kit_audit",
        // Export & Import
        "export_vault",
        "export_vault_csv",
        "export_vault_json",
        "get_vault_path",
        "query_mobile_autofill_status",
        "get_autofill_credentials_for_package",
        "parse_import_file",
        "parse_import_content",
        "import_entries",
        // Clipboard Defense & Native Zero-Disclosure IPC
        "copy_to_clipboard",
        "clear_clipboard",
        "copy_entry_password",
        "copy_entry_username",
        "copy_entry_totp",
        "create_vault_bytes",
        "open_vault_bytes",
        "change_master_password_bytes",
        "autotype_entry_password",
        "autotype_entry_smart",
        "verify_biometric_2fa",
        "get_installed_apps",
        "set_window_capture_protection",
        "set_lock_on_focus_loss",
        "set_lock_on_system_lock",
    ];

    for cmd in registered_commands {
        let pattern = format!("{}: {{", cmd);
        assert!(
            ts_content.contains(&pattern),
            "Command '{}' is registered in lib.rs but missing from src/types/ipc.ts schema contract",
            cmd
        );
    }
}
