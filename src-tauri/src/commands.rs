//! Tauri IPC Commands — Bridge between React frontend and yntra-vault-core
//!
//! Every #[tauri::command] becomes callable from JavaScript via invoke().

use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;
use uuid::Uuid;

use yntra_vault_core::vault::manager::{VaultManager, NewEntry, UpdateEntry, DecryptedEntry};
use yntra_vault_core::vault::types::*;
use yntra_vault_core::vault::entry::TrashedEntryPreview;
use yntra_vault_core::vault::history::DecryptedHistoryItem;
use yntra_vault_core::totp::{self, TotpConfig, TotpCode};
use yntra_vault_core::generator::{self, GeneratorOptions};
use yntra_vault_core::breach;
use yntra_vault_core::breach::strength;

use std::sync::atomic::{AtomicBool, Ordering};

/// Shared vault state across all commands.
pub struct AppState {
    pub vault: Mutex<Option<VaultManager>>,
    pub minimize_to_tray: AtomicBool,
}

// ─── Vault Commands ─────────────────────────────────────────────────────

#[tauri::command]
pub async fn create_vault(
    name: String,
    password: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let vault_path = PathBuf::from(&path);
    let manager = VaultManager::create(&name, &password, &vault_path)
        .map_err(|e| e.to_string())?;

    let info = manager.info();
    *state.vault.lock().map_err(|e| e.to_string())? = Some(manager);
    Ok(info)
}

#[tauri::command]
pub async fn open_vault(
    path: String,
    password: String,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let vault_path = PathBuf::from(&path);
    let manager = VaultManager::open(&vault_path, &password)
        .map_err(|e| e.to_string())?;

    let info = manager.info();
    *state.vault.lock().map_err(|e| e.to_string())? = Some(manager);
    Ok(info)
}

#[tauri::command]
pub async fn lock_vault(state: State<'_, AppState>) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    if let Some(ref mut manager) = *vault {
        manager.lock();
    }
    *vault = None;
    Ok(())
}

#[tauri::command]
pub async fn get_vault_info(state: State<'_, AppState>) -> Result<Option<VaultInfo>, String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    Ok(vault.as_ref().map(|m| m.info()))
}

// ─── Entry Commands ─────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_entries(state: State<'_, AppState>) -> Result<Vec<EntryPreview>, String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    manager.list_entries().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_entries(
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<EntryPreview>, String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    manager.search_entries(&query).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_entry(
    id: String,
    state: State<'_, AppState>,
) -> Result<DecryptedEntry, String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    manager.get_entry(uuid).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_entry(
    entry: NewEntry,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    let id = manager.add_entry(entry).map_err(|e| e.to_string())?;
    Ok(id.to_string())
}

#[tauri::command]
pub async fn update_entry(
    id: String,
    update: UpdateEntry,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    manager.update_entry(uuid, update).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_entry(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    manager.delete_entry(uuid).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_favorite(
    id: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    manager.toggle_favorite(uuid).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_pin(
    id: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    manager.toggle_pin(uuid).map_err(|e| e.to_string())
}

// ─── Trash Commands ─────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_trash(state: State<'_, AppState>) -> Result<Vec<TrashedEntryPreview>, String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    manager.list_trash().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_from_trash(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    manager.restore_from_trash(uuid).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn permanent_delete(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    manager.permanent_delete(uuid).map_err(|e| e.to_string())
}

// ─── Password History Commands ──────────────────────────────────────────

#[tauri::command]
pub async fn get_password_history(
    entry_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<DecryptedHistoryItem>, String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let uuid = Uuid::parse_str(&entry_id).map_err(|e| e.to_string())?;
    manager.get_password_history(uuid).map_err(|e| e.to_string())
}

// ─── TOTP Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn generate_totp(secret: String) -> Result<TotpCode, String> {
    let config = TotpConfig {
        secret,
        ..Default::default()
    };
    totp::generate_totp(&config).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn generate_totp_with_config(config: TotpConfig) -> Result<TotpCode, String> {
    totp::generate_totp(&config).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn parse_otpauth_uri(uri: String) -> Result<TotpConfig, String> {
    totp::parse_otpauth_uri(&uri).map_err(|e| e.to_string())
}

// ─── Password Generator Commands ────────────────────────────────────────

#[tauri::command]
pub async fn generate_password(options: GeneratorOptions) -> Result<String, String> {
    Ok(generator::generate_password(&options))
}

#[tauri::command]
pub async fn generate_password_default() -> Result<String, String> {
    Ok(generator::generate_password(&GeneratorOptions::default()))
}

// ─── Breach Detection Commands ──────────────────────────────────────────

#[tauri::command]
pub async fn check_password_breach(password: String) -> Result<breach::BreachResult, String> {
    breach::check_password_breach(&password).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn analyze_password_strength(password: String) -> Result<StrengthScore, String> {
    Ok(strength::analyze_password(&password))
}

// ─── Security Audit Commands ────────────────────────────────────────────

#[tauri::command]
pub async fn security_audit(state: State<'_, AppState>) -> Result<SecurityAudit, String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    manager.security_audit().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn change_master_password(
    current: String,
    new_password: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    manager.change_master_password(&current, &new_password).map_err(|e| e.to_string())
}

// ─── Tags Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_tags(state: State<'_, AppState>) -> Result<Vec<Tag>, String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    Ok(manager.tags().to_vec())
}

#[tauri::command]
pub async fn add_tag(
    name: String,
    color: String,
    icon: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    let id = manager.add_tag(&name, &color, &icon).map_err(|e| e.to_string())?;
    Ok(id.to_string())
}

#[tauri::command]
pub async fn delete_tag(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    manager.delete_tag(uuid).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn check_vault_file_exists(path: String) -> bool {
    std::path::Path::new(&path).exists()
}

#[tauri::command]
pub fn show_in_explorer(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new("explorer")
            .arg("/select,")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        use std::path::Path;
        use std::process::Command;
        if let Some(parent) = Path::new(&path).parent() {
            Command::new("xdg-open")
                .arg(parent)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

// ─── Autotype Commands ──────────────────────────────────────────────────

#[tauri::command]
pub async fn autotype(text: String, char_delay_ms: u64, settle_delay_ms: u64) -> Result<(), String> {
    yntra_vault_core::vault::autotype::autotype_text_with_delay(&text, char_delay_ms, settle_delay_ms).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_minimize_to_tray(enabled: bool, state: State<'_, AppState>) {
    state.minimize_to_tray.store(enabled, Ordering::Relaxed);
}

#[tauri::command]
pub async fn run_smart_autotype(
    username: String,
    password: String,
    totp_secret: String,
    url: String,
    launch_browser: bool,
    char_delay_ms: u64,
    field_delay_ms: u64,
) -> Result<(), String> {
    yntra_vault_core::vault::autotype::run_smart_autotype_with_delays(
        username,
        password,
        totp_secret,
        url,
        launch_browser,
        char_delay_ms,
        field_delay_ms,
    ).map_err(|e| e.to_string())
}

// ─── Autostart Commands ──────────────────────────────────────────────────

#[tauri::command]
pub async fn enable_autostart() -> Result<(), String> {
    yntra_vault_core::vault::autostart::enable_autostart().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn disable_autostart() -> Result<(), String> {
    yntra_vault_core::vault::autostart::disable_autostart().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn is_autostart_enabled() -> Result<bool, String> {
    yntra_vault_core::vault::autostart::is_autostart_enabled().map_err(|e| e.to_string())
}

// ─── Sync Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn webdav_upload(
    url: String,
    username: String,
    password: Option<String>,
    db_path: String,
) -> Result<(), String> {
    yntra_vault_core::vault::sync::webdav_upload(
        &url,
        &username,
        password.as_deref(),
        std::path::Path::new(&db_path),
    ).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn webdav_download(
    url: String,
    username: String,
    password: Option<String>,
    dest_db_path: String,
) -> Result<(), String> {
    yntra_vault_core::vault::sync::webdav_download(
        &url,
        &username,
        password.as_deref(),
        std::path::Path::new(&dest_db_path),
    ).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn run_p2p_sync_listener(
    listen_addr: String,
    db_path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let subkeys = manager.get_subkeys().map_err(|e| e.to_string())?;

    yntra_vault_core::vault::sync::run_p2p_sync_listener(
        &listen_addr,
        &subkeys.hmac_key,
        std::path::Path::new(&db_path),
    ).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn run_p2p_sync_client(
    server_addr: String,
    db_path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let subkeys = manager.get_subkeys().map_err(|e| e.to_string())?;

    yntra_vault_core::vault::sync::run_p2p_sync_client(
        &server_addr,
        &subkeys.hmac_key,
        std::path::Path::new(&db_path),
    ).map_err(|e| e.to_string())
}

// ─── Shamir Secret Sharing Recovery Commands ─────────────────────────────

#[tauri::command]
pub async fn split_master_password(password: String) -> Result<Vec<String>, String> {
    yntra_vault_core::crypto::split_password(&password).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reconstruct_master_password_hash(share_a: String, share_b: String) -> Result<String, String> {
    yntra_vault_core::crypto::reconstruct_password_to_hex(&share_a, &share_b).map_err(|e| e.to_string())
}

// ─── Export Commands ─────────────────────────────────────────────────────

#[tauri::command]
pub async fn export_vault(
    dest_path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let source = manager.info().path;
    std::fs::copy(&source, &dest_path)
        .map_err(|e| format!("Export failed: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn get_vault_path(state: State<'_, AppState>) -> Result<String, String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    Ok(manager.info().path)
}

// ─── Browser Extension Setup Commands ────────────────────────────────────

#[tauri::command]
pub async fn install_browser_extension() -> Result<String, String> {
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get exe path: {}", e))?;
    let host_name = "com.yntra.vault";

    // Build native host manifest path alongside the main executable
    let exe_dir = exe_path.parent().ok_or("No parent dir for exe")?;
    let host_exe = exe_dir.join("yntra-vault-native-host.exe");

    if !host_exe.exists() {
        return Err(format!("Native host binary not found at: {}", host_exe.display()));
    }

    let manifest = serde_json::json!({
        "name": host_name,
        "description": "Yntra Vault native messaging host",
        "path": host_exe.to_string_lossy(),
        "type": "stdio",
        "allowed_origins": [
            "chrome-extension://yntra-vault-extension/"
        ]
    });

    #[cfg(target_os = "windows")]
    {
        // Write manifest JSON next to the host binary
        let manifest_path = exe_dir.join("com.yntra.vault.json");
        std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest).unwrap())
            .map_err(|e| format!("Failed to write manifest: {}", e))?;

        // Register in Windows Registry for Chrome
        let reg_path = format!("SOFTWARE\\Google\\Chrome\\NativeMessagingHosts\\{}", host_name);
        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let (key, _) = hkcu.create_subkey(&reg_path)
            .map_err(|e| format!("Registry write failed: {}", e))?;
        key.set_value("", &manifest_path.to_string_lossy().to_string())
            .map_err(|e| format!("Registry value write failed: {}", e))?;

        return Ok(format!("Installed for Chrome. Manifest: {}", manifest_path.display()));
    }

    #[cfg(not(target_os = "windows"))]
    {
        // macOS/Linux: write manifest to well-known paths
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());

        #[cfg(target_os = "macos")]
        let chrome_dir = format!("{}/Library/Application Support/Google/Chrome/NativeMessagingHosts", home);
        #[cfg(target_os = "linux")]
        let chrome_dir = format!("{}/.config/google-chrome/NativeMessagingHosts", home);

        std::fs::create_dir_all(&chrome_dir).ok();
        let manifest_path = format!("{}/{}.json", chrome_dir, host_name);
        std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest).unwrap())
            .map_err(|e| format!("Failed to write manifest: {}", e))?;

        Ok(format!("Installed for Chrome. Manifest: {}", manifest_path))
    }
}

