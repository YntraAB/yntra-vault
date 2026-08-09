//! Tauri IPC Commands — Bridge between React frontend and yntra-vault-core
//!
//! Every #[tauri::command] becomes callable from JavaScript via invoke().

use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;
use uuid::Uuid;

use serde::{Serialize, Deserialize};
use yntra_vault_core::vault::manager::{VaultManager, NewEntry, UpdateEntry, DecryptedEntry};
use yntra_vault_core::vault::types::*;
use yntra_vault_core::vault::entry::TrashedEntryPreview;
use yntra_vault_core::vault::history::DecryptedHistoryItem;
use yntra_vault_core::totp::{self, TotpConfig, TotpCode};
use yntra_vault_core::generator::{self, GeneratorOptions};
use yntra_vault_core::breach;
use yntra_vault_core::breach::strength;
use zeroize::Zeroize;

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
    mut password: String,
    path: String,
    key_file_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let vault_path = PathBuf::from(&path);
    let kf_path = key_file_path.as_ref().map(PathBuf::from);
    let res = VaultManager::create_with_keyfile(&name, &password, kf_path.as_deref(), &vault_path)
        .map_err(|e| e.to_string());
    password.zeroize();
    let manager = res?;

    let info = manager.info();
    *state.vault.lock().map_err(|e| e.to_string())? = Some(manager);
    Ok(info)
}

#[tauri::command]
pub async fn open_vault(
    path: String,
    mut password: String,
    key_file_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let vault_path = PathBuf::from(&path);
    let kf_path = key_file_path.as_ref().map(PathBuf::from);
    let res = VaultManager::open_with_keyfile(&vault_path, &password, kf_path.as_deref())
        .map_err(|e| e.to_string());
    password.zeroize();
    let manager = res?;

    let info = manager.info();
    *state.vault.lock().map_err(|e| e.to_string())? = Some(manager);
    Ok(info)
}

#[tauri::command]
pub async fn check_biometric_available() -> Result<yntra_vault_core::crypto::biometric::BiometricInfo, String> {
    Ok(yntra_vault_core::crypto::biometric::check_biometric_availability())
}

#[tauri::command]
pub async fn is_biometric_enabled(path: String, state: State<'_, AppState>) -> Result<bool, String> {
    let vault_path = PathBuf::from(&path);
    if let Ok(vault_lock) = state.vault.lock() {
        if let Some(ref manager) = *vault_lock {
            if manager.is_biometric_enabled() {
                return Ok(true);
            }
        }
    }
    if yntra_vault_core::crypto::biometric::is_biometric_enabled(&vault_path) {
        return Ok(true);
    }
    if let Ok(canonical) = vault_path.canonicalize() {
        if yntra_vault_core::crypto::biometric::is_biometric_enabled(&canonical) {
            return Ok(true);
        }
    }
    Ok(false)
}

#[tauri::command]
pub async fn unlock_vault_biometric(
    path: String,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let vault_path = PathBuf::from(&path);
    let manager = VaultManager::open_with_biometric(&vault_path)
        .map_err(|e| e.to_string())?;

    let info = manager.info();
    *state.vault.lock().map_err(|e| e.to_string())? = Some(manager);
    Ok(info)
}

#[tauri::command]
pub async fn verify_biometric_2fa(
    window: tauri::Window,
    prompt: Option<String>,
) -> Result<(), String> {
    let _ = window.set_focus();
    let _ = window.set_always_on_top(true);
    let hwnd_raw = window.hwnd().map(|h| h.0 as isize).ok();
    let msg = prompt.unwrap_or_else(|| "Unlock Yntra Vault".to_string());
    let res = yntra_vault_core::crypto::biometric::request_user_consent_with_hwnd(&msg, hwnd_raw);
    let _ = window.set_always_on_top(false);
    res.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn enable_biometric(state: State<'_, AppState>) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    manager.enable_biometric().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn disable_biometric(state: State<'_, AppState>) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    manager.disable_biometric().map_err(|e| e.to_string())
}

// ─── Hardware 2FA Commands ────────────────────────────────────────────────

#[tauri::command]
pub async fn check_hardware2fa_available() -> Result<yntra_vault_core::crypto::hardware2fa::Hardware2FaInfo, String> {
    Ok(yntra_vault_core::crypto::hardware2fa::check_hardware2fa_availability())
}

#[tauri::command]
pub async fn list_hardware_keys() -> Result<Vec<yntra_vault_core::crypto::hardware2fa::HardwareKeyInfo>, String> {
    Ok(yntra_vault_core::crypto::hardware2fa::list_hardware_keys())
}

#[tauri::command]
pub async fn is_hardware2fa_enabled(path: String) -> Result<bool, String> {
    let vault_path = PathBuf::from(&path);
    Ok(VaultManager::is_hardware2fa_enabled_file(&vault_path))
}

#[tauri::command]
pub async fn open_vault_with_hardware2fa(
    path: String,
    mut password: String,
    key_file_path: Option<String>,
    hardware_response: Vec<u8>,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let vault_path = PathBuf::from(&path);
    let kf_path = key_file_path.as_ref().map(PathBuf::from);
    let res = VaultManager::open_with_hardware2fa(
        &vault_path,
        &password,
        kf_path.as_deref(),
        &hardware_response,
    ).map_err(|e| e.to_string());
    password.zeroize();
    let manager = res?;

    let info = manager.info();
    *state.vault.lock().map_err(|e| e.to_string())? = Some(manager);
    Ok(info)
}

#[tauri::command]
pub async fn perform_hardware2fa_challenge(
    protocol: String,
    challenge: Option<Vec<u8>>,
) -> Result<Vec<u8>, String> {
    let proto = match protocol.as_str() {
        "Fido2Ctap2HmacSecret" | "fido2" => yntra_vault_core::crypto::hardware2fa::Hardware2FaProtocol::Fido2Ctap2HmacSecret,
        _ => yntra_vault_core::crypto::hardware2fa::Hardware2FaProtocol::YubiKeyChallengeResponse,
    };
    let chall = challenge.unwrap_or_else(|| b"yntra-vault-hardware2fa-default-challenge".to_vec());
    yntra_vault_core::crypto::hardware2fa::perform_hardware2fa_challenge(proto, &chall)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn enable_hardware2fa(
    protocol: String,
    key_name: String,
    hardware_response: Vec<u8>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    let proto = match protocol.as_str() {
        "Fido2Ctap2HmacSecret" | "fido2" => yntra_vault_core::crypto::hardware2fa::Hardware2FaProtocol::Fido2Ctap2HmacSecret,
        _ => yntra_vault_core::crypto::hardware2fa::Hardware2FaProtocol::YubiKeyChallengeResponse,
    };
    manager.enable_hardware2fa(proto, &key_name, &hardware_response).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn disable_hardware2fa(state: State<'_, AppState>) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    manager.disable_hardware2fa().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn generate_key_file(path: String) -> Result<(), String> {
    let kf_path = PathBuf::from(&path);
    VaultManager::generate_key_file(&kf_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn lock_vault(state: State<'_, AppState>) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    if let Some(ref mut manager) = *vault {
        manager.lock();
    }
    *vault = None;
    let _ = yntra_vault_core::crypto::clear_clipboard();
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

// ─── Attachment Commands ────────────────────────────────────────────────

#[tauri::command]
pub async fn get_attachment_data(
    entry_id: String,
    attachment_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<u8>, String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let e_uuid = Uuid::parse_str(&entry_id).map_err(|e| e.to_string())?;
    let a_uuid = Uuid::parse_str(&attachment_id).map_err(|e| e.to_string())?;
    manager.get_attachment_data(e_uuid, a_uuid).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_attachment(
    entry_id: String,
    name: String,
    mime_type: String,
    data: Vec<u8>,
    state: State<'_, AppState>,
) -> Result<AttachmentInfo, String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    let e_uuid = Uuid::parse_str(&entry_id).map_err(|e| e.to_string())?;
    manager.add_attachment(e_uuid, &name, &mime_type, &data).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_attachment(
    entry_id: String,
    attachment_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    let e_uuid = Uuid::parse_str(&entry_id).map_err(|e| e.to_string())?;
    let a_uuid = Uuid::parse_str(&attachment_id).map_err(|e| e.to_string())?;
    manager.delete_attachment(e_uuid, a_uuid).map_err(|e| e.to_string())
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

#[tauri::command]
pub async fn empty_trash(state: State<'_, AppState>) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    manager.empty_trash().map_err(|e| e.to_string())
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
pub async fn generate_totp(mut secret: String) -> Result<TotpCode, String> {
    let config = TotpConfig {
        secret: secret.clone(),
        ..Default::default()
    };
    let res = totp::generate_totp(&config).map_err(|e| e.to_string());
    secret.zeroize();
    res
}

#[tauri::command]
pub async fn generate_totp_with_config(mut config: TotpConfig) -> Result<TotpCode, String> {
    let res = totp::generate_totp(&config).map_err(|e| e.to_string());
    config.secret.zeroize();
    res
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
pub async fn check_password_breach(mut password: String) -> Result<breach::BreachResult, String> {
    let res = breach::check_password_breach(&password).await.map_err(|e| e.to_string());
    password.zeroize();
    res
}

#[tauri::command]
pub async fn analyze_password_strength(mut password: String) -> Result<StrengthScore, String> {
    let score = strength::analyze_password(&password);
    password.zeroize();
    Ok(score)
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
    mut current: String,
    mut new_password: String,
    current_key_file: Option<String>,
    new_key_file: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    let cur_kf = current_key_file.as_ref().map(PathBuf::from);
    let new_kf = new_key_file.as_ref().map(PathBuf::from);
    let res = manager
        .change_master_password_with_keyfiles(&current, cur_kf.as_deref(), &new_password, new_kf.as_deref())
        .map_err(|e| e.to_string());
    current.zeroize();
    new_password.zeroize();
    res
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
pub async fn update_tag(
    id: String,
    name: String,
    color: String,
    icon: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    manager.update_tag(uuid, &name, &color, &icon).map_err(|e| e.to_string())
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
        }
        Ok(())
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub struct InstalledApp {
    pub name: String,
    pub path: String,
    pub category: String,
    pub is_system: bool,
}

#[tauri::command]
pub async fn get_installed_apps() -> Result<Vec<InstalledApp>, String> {
    let mut apps = Vec::new();

    #[cfg(target_os = "windows")]
    {
        let dirs_to_scan = vec![
            std::env::var("APPDATA").ok().map(|p| format!("{}\\Microsoft\\Windows\\Start Menu\\Programs", p)),
            std::env::var("ProgramData").ok().map(|p| format!("{}\\Microsoft\\Windows\\Start Menu\\Programs", p)),
        ];

        for dir_opt in dirs_to_scan {
            if let Some(dir) = dir_opt {
                let path = std::path::Path::new(&dir);
                if path.exists() {
                    scan_dir_for_apps(path, &mut apps, 0);
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let mac_dirs = vec!["/Applications", "/System/Applications"];
        for dir in mac_dirs {
            let path = std::path::Path::new(dir);
            if path.exists() {
                scan_dir_for_apps(path, &mut apps, 0);
            }
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let linux_dirs = vec!["/usr/share/applications", "/usr/local/share/applications"];
        for dir in linux_dirs {
            let path = std::path::Path::new(dir);
            if path.exists() {
                scan_dir_for_apps(path, &mut apps, 0);
            }
        }
    }

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps.dedup_by(|a, b| a.name.to_lowercase() == b.name.to_lowercase() || a.path == b.path);

    Ok(apps)
}

fn categorize_app(name: &str, path: &str) -> (String, bool) {
    let lower_name = name.to_lowercase();
    let lower_path = path.to_lowercase();

    // Cross-platform system utility noise filter (Windows, macOS, Linux)
    let system_noise = [
        // Windows system noise & non-login utilities
        "calculator", "character map", "snipping tool", "snip & sketch", "paint", "wordpad",
        "defragment", "disk cleanup", "component services", "computer management", "event viewer",
        "memory diagnostic", "services", "system information", "system configuration",
        "task manager", "registry editor", "regedit", "control panel", "windows defender",
        "windows security", "narrator", "magnifier", "on-screen keyboard", "voice recorder",
        "steps recorder", "windows fax", "windows media player", "quick assist", "math input panel",
        "administrative tools", "windows accessories", "windows system", "windows powershell",
        "windows kits", "accessibility", "accessability", "command prompt", "task scheduler", "taskschd", "python", "pydoc", "sample uwp",
        "recoverydrive", "recovery drive", "performance monitor", "perfmon", "livecaptions",
        "live captions", "iscsi", "git bash", "git gui", "release notes", "dfrgui",
        "application verifier", "appverif", "file explorer", "odbc", "eventvwr", "resmon",
        "resource monitor", "sysedit", "msconfig", "dxdiag", "cleanmgr", "compmgmt",
        "hyper-v", "print management", "debuggable package", "wsl", "windows subsystem for linux",
        "run", "7-zip", "7zip", "node.js", "nodejs", "autohotkey",
        // macOS system noise
        "activity monitor", "audio midi setup", "bluetooth file exchange", "colorsync utility",
        "console", "digital color meter", "disk utility", "grapher", "keychain access",
        "migration assistant", "screen sharing", "system settings", "system preferences",
        "font book", "image capture", "launchpad", "mission control", "stickies", "stocks",
        "voice memos", "automator", "chess", "dictionary", "clock",
        // Linux system noise
        "gnome-calculator", "gnome-disks", "gnome-system", "gnome-logs", "gnome-font",
        "xfce4-taskmanager", "baobab", "gparted", "nm-connection-editor", "dconf-editor",
        "system-config-", "htop", "top",
    ];

    let is_system = system_noise.iter().any(|&s| lower_name.contains(s) || lower_path.contains(s)) ||
        lower_path.contains("system32") ||
        lower_path.contains("windows\\syswow64") ||
        lower_path.contains("/system/applications/") ||
        lower_path.contains("/applications/utilities/");

    // Browsers & Communication
    if lower_name.contains("chrome") || lower_name.contains("firefox") || lower_name.contains("edge") ||
       lower_name.contains("brave") || lower_name.contains("opera") || lower_name.contains("vivaldi") ||
       lower_name.contains("tor browser") || lower_name.contains("arc") || lower_name.contains("safari") ||
       lower_name.contains("duckduckgo") || lower_name.contains("waterfox") || lower_name.contains("librewolf") ||
       lower_name.contains("discord") || lower_name.contains("slack") || lower_name.contains("telegram") ||
       lower_name.contains("whatsapp") || lower_name.contains("signal") || lower_name.contains("teams") ||
       lower_name.contains("outlook") || lower_name.contains("thunderbird") || lower_name.contains("zoom") ||
       lower_name.contains("skype") || lower_name.contains("viber") || lower_name.contains("messenger") ||
       lower_name.contains("element") || lower_name.contains("session") || lower_name.contains("mailspring") ||
       lower_path.contains("discord://") || lower_path.contains("tg://") || lower_path.contains("slack://") {
        return ("browsers_communication".into(), is_system);
    }

    // Developer, Cloud & Productivity
    if lower_name.contains("code") || lower_name.contains("cursor") || lower_name.contains("antigravity") ||
       lower_name.contains("windsurf") || lower_name.contains("zed") || lower_name.contains("vscodium") ||
       lower_name.contains("visual studio") || lower_name.contains("intellij") || lower_name.contains("datagrip") ||
       lower_name.contains("rustrover") || lower_name.contains("pycharm") || lower_name.contains("webstorm") ||
       lower_name.contains("rider") || lower_name.contains("goland") || lower_name.contains("phpstorm") ||
       lower_name.contains("clion") || lower_name.contains("android studio") || lower_name.contains("sublime") ||
       lower_name.contains("notepad++") || lower_name.contains("xcode") || lower_name.contains("fleet") ||
       lower_name.contains("dbeaver") || lower_name.contains("tableplus") || lower_name.contains("pgadmin") ||
       lower_name.contains("workbench") || lower_name.contains("compass") || lower_name.contains("redis") ||
       lower_name.contains("postman") || lower_name.contains("insomnia") || lower_name.contains("bruno") ||
       lower_name.contains("warp") || lower_name.contains("sourcetree") || lower_name.contains("gitkraken") ||
       lower_name.contains("github desktop") || lower_name.contains("docker") || lower_name.contains("filezilla") ||
       lower_name.contains("termius") || lower_name.contains("obsidian") || lower_name.contains("notion") ||
       lower_name.contains("logseq") || lower_name.contains("joplin") || lower_name.contains("evernote") ||
       lower_name.contains("onenote") || lower_name.contains("linear") || lower_name.contains("clickup") ||
       lower_name.contains("jira") || lower_name.contains("confluence") || lower_name.contains("trello") ||
       lower_name.contains("office") || lower_name.contains("word") || lower_name.contains("excel") ||
       lower_name.contains("powerpoint") || lower_name.contains("libreoffice") || lower_name.contains("figma") ||
       lower_name.contains("adobe") || lower_name.contains("creative cloud") || lower_name.contains("photoshop") ||
       lower_name.contains("illustrator") || lower_name.contains("premiere") || lower_name.contains("after effects") ||
       lower_name.contains("acrobat") || lower_name.contains("affinity") || lower_name.contains("blender") ||
       lower_name.contains("gimp") || lower_name.contains("inkscape") || lower_name.contains("davinci") ||
       lower_name.contains("wireguard") || lower_name.contains("tailscale") || lower_name.contains("mullvad") ||
       lower_name.contains("protonvpn") || lower_name.contains("nordvpn") || lower_name.contains("bitwarden") ||
       lower_name.contains("1password") || lower_name.contains("keepass") || lower_name.contains("syntra") ||
       lower_path.contains("vscode://") || lower_path.contains("cursor://") || lower_path.contains("windsurf://") ||
       lower_path.contains("obsidian://") || lower_path.contains("notion://") || lower_path.contains("linear://") {
        return ("productivity_dev".into(), is_system);
    }

    // Games, Launchers & Media
    if lower_name.contains("steam") || lower_name.contains("epic") || lower_name.contains("gog") ||
       lower_name.contains("ubisoft") || lower_name.contains("battle.net") || lower_name.contains("ea") ||
       lower_name.contains("riot") || lower_name.contains("league") || lower_name.contains("valorant") ||
       lower_name.contains("roblox") || lower_name.contains("minecraft") || lower_name.contains("geforce") ||
       lower_name.contains("nvidia") || lower_name.contains("heroic") || lower_name.contains("lutris") || lower_name.contains("prism") ||
       lower_name.contains("spotify") || lower_name.contains("tidal") || lower_name.contains("deezer") ||
       lower_name.contains("vlc") || lower_name.contains("plex") || lower_name.contains("kodi") ||
       lower_name.contains("obs studio") || lower_name.contains("audacity") || lower_name.contains("handbrake") ||
       lower_path.contains("steam://") || lower_path.contains("spotify://") {
        return ("gaming".into(), is_system);
    }

    if is_system || lower_name.contains("terminal") || lower_name.contains("powershell") || lower_name.contains("cmd") || lower_name.contains("putty") || lower_name.contains("konsole") {
        return ("system".into(), true);
    }

    ("general".into(), is_system)
}

fn scan_dir_for_apps(dir: &std::path::Path, apps: &mut Vec<InstalledApp>, depth: usize) {
    if depth > 4 { return; }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if p.extension().map_or(false, |ext| ext == "app") {
                    let name = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
                    let path_str = p.to_string_lossy().to_string();
                    let (category, is_system) = categorize_app(&name, &path_str);
                    apps.push(InstalledApp {
                        name,
                        path: path_str,
                        category,
                        is_system,
                    });
                } else {
                    scan_dir_for_apps(&p, apps, depth + 1);
                }
            } else if let Some(ext) = p.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if ext_str == "exe" || ext_str == "lnk" || ext_str == "desktop" {
                    let name = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
                    let lower = name.to_lowercase();
                    if lower.contains("uninstall") || lower.contains("setup") || lower.contains("update") || lower.contains("readme") || lower.contains("help") || lower.contains("license") || lower.contains("changelog") {
                        continue;
                    }
                    let path_str = p.to_string_lossy().to_string();
                    let (category, is_system) = categorize_app(&name, &path_str);
                    apps.push(InstalledApp {
                        name,
                        path: path_str,
                        category,
                        is_system,
                    });
                }
            }
        }
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
pub async fn webdav_test_connection(
    url: String,
    username: String,
    password: Option<String>,
) -> Result<(), String> {
    yntra_vault_core::vault::sync::webdav_test_connection(
        &url,
        &username,
        password.as_deref(),
    ).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn webdav_upload(
    url: String,
    username: String,
    password: Option<String>,
    db_path: String,
    if_match_etag: Option<String>,
) -> Result<Option<String>, String> {
    yntra_vault_core::vault::sync::webdav_upload(
        &url,
        &username,
        password.as_deref(),
        std::path::Path::new(&db_path),
        if_match_etag.as_deref(),
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
pub async fn webdav_sync(
    url: String,
    username: String,
    password: Option<String>,
    state: State<'_, AppState>,
) -> Result<yntra_vault_core::vault::sync::MergeStats, String> {
    const MAX_RETRIES: usize = 3;

    let (subkeys, db_path, mut current_etag) = {
        let mut vault_guard = state.vault.lock().map_err(|e| e.to_string())?;
        let mgr = vault_guard.as_mut().ok_or("Vault is locked")?;
        mgr.save().map_err(|e| e.to_string())?;
        let subkeys = (*mgr.get_subkeys().map_err(|e| e.to_string())?).clone();
        let db_path = mgr.path.clone();
        let current_etag = mgr.data.settings.webdav.last_etag.clone();
        (subkeys, db_path, current_etag)
    };

    let mut accumulated_stats = yntra_vault_core::vault::sync::MergeStats::default();

    for attempt in 0..MAX_RETRIES {
        let upload_res = yntra_vault_core::vault::sync::webdav_upload(
            &url,
            &username,
            password.as_deref(),
            &db_path,
            current_etag.as_deref(),
        ).await;

        match upload_res {
            Ok(new_etag_opt) => {
                let mut vault_guard = state.vault.lock().map_err(|e| e.to_string())?;
                if let Some(mgr) = vault_guard.as_mut() {
                    if let Some(new_etag) = new_etag_opt {
                        mgr.data.settings.webdav.last_etag = Some(new_etag);
                    }
                    mgr.data.settings.webdav.last_sync_at = Some(chrono::Utc::now());
                    let _ = mgr.save();
                }
                return Ok(accumulated_stats);
            }
            Err(e) if e.to_string().contains("412 Precondition Failed") || e.to_string().contains("modified on server") => {
                // Conflict detected!
                // 1. Fetch remote ETag first to get the latest server version
                let remote_etag = yntra_vault_core::vault::sync::webdav_get_etag(
                    &url,
                    &username,
                    password.as_deref(),
                ).await.unwrap_or(None);

                // 2. Download remote bytes into memory
                let remote_bytes = yntra_vault_core::vault::sync::webdav_download_bytes(
                    &url,
                    &username,
                    password.as_deref(),
                ).await.map_err(|err| format!("Failed downloading remote vault for merge (attempt {}): {}", attempt + 1, err))?;

                // 3. Decrypt remote payload
                let remote_data = yntra_vault_core::vault::sync::decrypt_remote_vault_bytes(
                    &remote_bytes,
                    &subkeys,
                ).map_err(|err| format!("Failed decrypting remote vault payload (attempt {}): {}", attempt + 1, err))?;

                // 4. Perform 3-way merge in memory & save local database file
                let stats = {
                    let mut vault_guard = state.vault.lock().map_err(|e| e.to_string())?;
                    let mgr = vault_guard.as_mut().ok_or("Vault is locked")?;
                    let stats = yntra_vault_core::vault::sync::merge_vault_data(&mut mgr.data, remote_data);
                    mgr.save().map_err(|e| e.to_string())?;
                    stats
                };

                accumulated_stats.entries_added += stats.entries_added;
                accumulated_stats.entries_updated += stats.entries_updated;
                accumulated_stats.entries_kept_local += stats.entries_kept_local;
                accumulated_stats.tags_merged += stats.tags_merged;
                accumulated_stats.trash_merged += stats.trash_merged;

                // Set current_etag to the acquired remote_etag so the next loop iteration attempts conditional PUT against it
                current_etag = remote_etag;
            }
            Err(e) => return Err(e.to_string()),
        }
    }

    Err("WebDAV sync failed after maximum retry attempts due to high remote contention".into())
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
pub async fn export_vault_csv(
    dest_path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let path = PathBuf::from(&dest_path);
    manager.export_csv(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_vault_json(
    dest_path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let path = PathBuf::from(&dest_path);
    manager.export_json(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_vault_path(state: State<'_, AppState>) -> Result<String, String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    Ok(manager.info().path)
}

// ─── Import Commands ─────────────────────────────────────────────────────

#[tauri::command]
pub async fn parse_import_file(
    file_path: String,
    format: Option<String>,
    state: State<'_, AppState>,
) -> Result<yntra_vault_core::vault::importer::ImportPreviewResult, String> {
    let requested_fmt = match format.as_deref() {
        Some("bitwarden_json") => yntra_vault_core::vault::importer::ImportFormat::BitwardenJson,
        Some("bitwarden_csv") => yntra_vault_core::vault::importer::ImportFormat::BitwardenCsv,
        Some("onepassword_csv") => yntra_vault_core::vault::importer::ImportFormat::OnePasswordCsv,
        Some("keepass_csv") => yntra_vault_core::vault::importer::ImportFormat::KeepassCsv,
        Some("keepass_xml") => yntra_vault_core::vault::importer::ImportFormat::KeepassXml,
        Some("chrome_csv") => yntra_vault_core::vault::importer::ImportFormat::ChromeCsv,
        Some("lastpass_csv") => yntra_vault_core::vault::importer::ImportFormat::LastPassCsv,
        Some("dashlane_csv") => yntra_vault_core::vault::importer::ImportFormat::DashlaneCsv,
        Some("protonpass_json") => yntra_vault_core::vault::importer::ImportFormat::ProtonPassJson,
        Some("protonpass_csv") => yntra_vault_core::vault::importer::ImportFormat::ProtonPassCsv,
        Some("generic_csv") => yntra_vault_core::vault::importer::ImportFormat::GenericCsv,
        _ => yntra_vault_core::vault::importer::ImportFormat::AutoDetect,
    };

    let path = PathBuf::from(&file_path);
    let mut preview = yntra_vault_core::vault::importer::Importer::parse_file(&path, requested_fmt)
        .map_err(|e| e.to_string())?;

    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    if let Some(ref manager) = *vault {
        preview.duplicates_count = manager.check_import_duplicates(&mut preview.entries);
    }

    Ok(preview)
}

#[tauri::command]
pub async fn parse_import_content(
    content: String,
    format: Option<String>,
    state: State<'_, AppState>,
) -> Result<yntra_vault_core::vault::importer::ImportPreviewResult, String> {
    let requested_fmt = match format.as_deref() {
        Some("bitwarden_json") => yntra_vault_core::vault::importer::ImportFormat::BitwardenJson,
        Some("bitwarden_csv") => yntra_vault_core::vault::importer::ImportFormat::BitwardenCsv,
        Some("onepassword_csv") => yntra_vault_core::vault::importer::ImportFormat::OnePasswordCsv,
        Some("keepass_csv") => yntra_vault_core::vault::importer::ImportFormat::KeepassCsv,
        Some("keepass_xml") => yntra_vault_core::vault::importer::ImportFormat::KeepassXml,
        Some("chrome_csv") => yntra_vault_core::vault::importer::ImportFormat::ChromeCsv,
        Some("lastpass_csv") => yntra_vault_core::vault::importer::ImportFormat::LastPassCsv,
        Some("dashlane_csv") => yntra_vault_core::vault::importer::ImportFormat::DashlaneCsv,
        Some("protonpass_json") => yntra_vault_core::vault::importer::ImportFormat::ProtonPassJson,
        Some("protonpass_csv") => yntra_vault_core::vault::importer::ImportFormat::ProtonPassCsv,
        Some("generic_csv") => yntra_vault_core::vault::importer::ImportFormat::GenericCsv,
        _ => yntra_vault_core::vault::importer::ImportFormat::AutoDetect,
    };

    let mut preview = yntra_vault_core::vault::importer::Importer::parse_str(&content, requested_fmt)
        .map_err(|e| e.to_string())?;

    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    if let Some(ref manager) = *vault {
        preview.duplicates_count = manager.check_import_duplicates(&mut preview.entries);
    }

    Ok(preview)
}

#[tauri::command]
pub async fn import_entries(
    entries: Vec<yntra_vault_core::vault::importer::ParsedImportEntry>,
    duplicate_strategy: String,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;

    let strategy = match duplicate_strategy.as_str() {
        "overwrite" => yntra_vault_core::vault::importer::DuplicateStrategy::Overwrite,
        "keep_both" => yntra_vault_core::vault::importer::DuplicateStrategy::KeepBoth,
        _ => yntra_vault_core::vault::importer::DuplicateStrategy::Skip,
    };

    manager.bulk_import_entries(entries, strategy).map_err(|e| e.to_string())
}

// ─── Clipboard Defense Commands ──────────────────────────────────────────────

#[tauri::command]
pub async fn copy_to_clipboard(
    text: String,
    is_sensitive: Option<bool>,
    clear_after_secs: Option<u64>,
) -> Result<(), String> {
    let sensitive = is_sensitive.unwrap_or(true);
    yntra_vault_core::crypto::copy_to_clipboard_defended(&text, sensitive, clear_after_secs)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_clipboard() -> Result<(), String> {
    yntra_vault_core::crypto::clear_clipboard()
        .map_err(|e| e.to_string())
}

// ─── Binary Uint8Array IPC Secret Transport Commands ───────────────────────

#[tauri::command]
pub async fn create_vault_bytes(
    name: String,
    mut password_bytes: Vec<u8>,
    path: String,
    key_file_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let password = String::from_utf8(password_bytes.clone())
        .map_err(|_| "Password not valid UTF-8".to_string())?;
    password_bytes.zeroize();

    let vault_path = PathBuf::from(&path);
    let kf_path = key_file_path.as_ref().map(PathBuf::from);
    let mut mut_pass = password;
    let res = VaultManager::create_with_keyfile(&name, &mut_pass, kf_path.as_deref(), &vault_path)
        .map_err(|e| e.to_string());
    mut_pass.zeroize();
    let manager = res?;

    let info = manager.info();
    *state.vault.lock().map_err(|e| e.to_string())? = Some(manager);
    Ok(info)
}

#[tauri::command]
pub async fn open_vault_bytes(
    path: String,
    mut password_bytes: Vec<u8>,
    key_file_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let password = String::from_utf8(password_bytes.clone())
        .map_err(|_| "Password not valid UTF-8".to_string())?;
    password_bytes.zeroize();

    let vault_path = PathBuf::from(&path);
    let kf_path = key_file_path.as_ref().map(PathBuf::from);
    let mut mut_pass = password;
    let res = VaultManager::open_with_keyfile(&vault_path, &mut_pass, kf_path.as_deref())
        .map_err(|e| e.to_string());
    mut_pass.zeroize();
    let manager = res?;

    let info = manager.info();
    *state.vault.lock().map_err(|e| e.to_string())? = Some(manager);
    Ok(info)
}

#[tauri::command]
pub async fn change_master_password_bytes(
    mut current_bytes: Vec<u8>,
    mut new_password_bytes: Vec<u8>,
    current_key_file: Option<String>,
    new_key_file: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let current = String::from_utf8(current_bytes.clone())
        .map_err(|_| "Current password not valid UTF-8".to_string())?;
    let new_password = String::from_utf8(new_password_bytes.clone())
        .map_err(|_| "New password not valid UTF-8".to_string())?;
    current_bytes.zeroize();
    new_password_bytes.zeroize();

    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    let cur_kf = current_key_file.as_ref().map(PathBuf::from);
    let new_kf = new_key_file.as_ref().map(PathBuf::from);

    let mut mut_cur = current;
    let mut mut_new = new_password;
    let res = manager
        .change_master_password_with_keyfiles(&mut_cur, cur_kf.as_deref(), &mut_new, new_kf.as_deref())
        .map_err(|e| e.to_string());
    mut_cur.zeroize();
    mut_new.zeroize();
    res
}

// ─── Zero-Disclosure Native Handle IPC Commands ─────────────────────────────

#[tauri::command]
pub async fn copy_entry_password(
    entry_id: String,
    clear_after_secs: Option<u64>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let uuid = Uuid::parse_str(&entry_id).map_err(|e| e.to_string())?;
    let mut entry = manager.get_entry(uuid).map_err(|e| e.to_string())?;
    let res = yntra_vault_core::crypto::copy_to_clipboard_defended(&entry.password, true, clear_after_secs);
    entry.password.zeroize();
    res.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn copy_entry_username(
    entry_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let uuid = Uuid::parse_str(&entry_id).map_err(|e| e.to_string())?;
    let entry = manager.get_entry(uuid).map_err(|e| e.to_string())?;
    yntra_vault_core::crypto::copy_to_clipboard_defended(&entry.username, false, None)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn copy_entry_totp(
    entry_id: String,
    clear_after_secs: Option<u64>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let uuid = Uuid::parse_str(&entry_id).map_err(|e| e.to_string())?;
    let entry = manager.get_entry(uuid).map_err(|e| e.to_string())?;
    let secret = entry.totp_secret.ok_or("No TOTP secret for this entry")?;
    let config = TotpConfig {
        secret: secret.clone(),
        ..Default::default()
    };
    let code = totp::generate_totp(&config).map_err(|e| e.to_string())?;
    let res = yntra_vault_core::crypto::copy_to_clipboard_defended(&code.code, true, clear_after_secs);
    let mut mut_secret = secret;
    mut_secret.zeroize();
    res.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn autotype_entry_password(
    entry_id: String,
    char_delay_ms: Option<u64>,
    settle_delay_ms: Option<u64>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let uuid = Uuid::parse_str(&entry_id).map_err(|e| e.to_string())?;
    let mut entry = manager.get_entry(uuid).map_err(|e| e.to_string())?;
    let res = yntra_vault_core::vault::autotype::autotype_text_with_delay(
        &entry.password,
        char_delay_ms.unwrap_or(15),
        settle_delay_ms.unwrap_or(3000),
    ).map_err(|e| e.to_string());
    entry.password.zeroize();
    res
}

#[tauri::command]
pub async fn autotype_entry_smart(
    entry_id: String,
    launch_browser: Option<bool>,
    char_delay_ms: Option<u64>,
    field_delay_ms: Option<u64>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let uuid = Uuid::parse_str(&entry_id).map_err(|e| e.to_string())?;
    let mut entry = manager.get_entry(uuid).map_err(|e| e.to_string())?;
    let totp_sec = entry.totp_secret.clone().unwrap_or_default();
    let res = yntra_vault_core::vault::autotype::run_smart_autotype_with_delays(
        entry.username.clone(),
        entry.password.clone(),
        totp_sec.clone(),
        entry.url.clone(),
        launch_browser.unwrap_or(false),
        char_delay_ms.unwrap_or(15),
        field_delay_ms.unwrap_or(300),
    ).map_err(|e| e.to_string());
    entry.password.zeroize();
    let mut mut_totp = totp_sec;
    mut_totp.zeroize();
    res
}


