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
    key_file_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let vault_path = PathBuf::from(&path);
    let kf_path = key_file_path.as_ref().map(PathBuf::from);
    let manager = VaultManager::create_with_keyfile(&name, &password, kf_path.as_deref(), &vault_path)
        .map_err(|e| e.to_string())?;

    let info = manager.info();
    *state.vault.lock().map_err(|e| e.to_string())? = Some(manager);
    Ok(info)
}

#[tauri::command]
pub async fn open_vault(
    path: String,
    password: String,
    key_file_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let vault_path = PathBuf::from(&path);
    let kf_path = key_file_path.as_ref().map(PathBuf::from);
    let manager = VaultManager::open_with_keyfile(&vault_path, &password, kf_path.as_deref())
        .map_err(|e| e.to_string())?;

    let info = manager.info();
    *state.vault.lock().map_err(|e| e.to_string())? = Some(manager);
    Ok(info)
}

#[tauri::command]
pub async fn check_biometric_available() -> Result<yntra_vault_core::crypto::biometric::BiometricInfo, String> {
    Ok(yntra_vault_core::crypto::biometric::check_biometric_availability())
}

#[tauri::command]
pub async fn is_biometric_enabled(path: String) -> Result<bool, String> {
    let vault_path = PathBuf::from(&path);
    Ok(yntra_vault_core::crypto::biometric::is_biometric_enabled(&vault_path))
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
    current_key_file: Option<String>,
    new_key_file: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    let cur_kf = current_key_file.as_ref().map(PathBuf::from);
    let new_kf = new_key_file.as_ref().map(PathBuf::from);
    manager
        .change_master_password_with_keyfiles(&current, cur_kf.as_deref(), &new_password, new_kf.as_deref())
        .map_err(|e| e.to_string())
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

