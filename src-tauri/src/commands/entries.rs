use tauri::State;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

use yntra_vault_core::totp::{self, TotpConfig};
use yntra_vault_core::vault::entry::TrashedEntryPreview;
use yntra_vault_core::vault::history::DecryptedHistoryItem;
use yntra_vault_core::vault::manager::{DecryptedEntry, NewEntry, UpdateEntry};
use yntra_vault_core::vault::types::*;

use super::AppState;

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
pub async fn update_entry_breach_status(
    id: String,
    breach_status: BreachStatus,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    manager.update_entry_breach_status(uuid, breach_status).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_vault(state: State<'_, AppState>) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    manager.save_vault().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reload_vault(state: State<'_, AppState>) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    manager.reload().map_err(|e| e.to_string())
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
pub async fn get_vault_path(state: State<'_, AppState>) -> Result<String, String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    Ok(manager.info().path)
}

#[tauri::command]
pub async fn query_mobile_autofill_status(
    state: State<'_, AppState>,
) -> Result<yntra_vault_core::services::autofill::MobileAutofillStatus, String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let entries_count = manager.list_entries().map_err(|e| e.to_string())?.len();

    Ok(yntra_vault_core::services::autofill::MobileAutofillStatus {
        supported: true,
        enabled: true,
        active_provider: "Android AutofillService / iOS CredentialProvider".into(),
        mapped_packages_count: entries_count,
        strict_domain_matching: true,
        asset_links_enforced: true,
        webview_origin_protected: true,
        biometric_stepup_required: true,
    })
}

#[tauri::command]
pub async fn get_autofill_credentials_for_package(
    package_name: String,
    web_domain: Option<String>,
    state: State<'_, AppState>,
) -> Result<yntra_vault_core::services::autofill::AutofillDatasetPayload, String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let items = manager.find_entries_for_mobile_context(&package_name, web_domain.as_deref()).map_err(|e| e.to_string())?;
    let asset_links_valid = items.iter().any(|item| {
        yntra_vault_core::services::autofill::verify_digital_asset_links(&item.domain, &package_name, None)
    });

    Ok(yntra_vault_core::services::autofill::AutofillDatasetPayload {
        package_name,
        web_domain,
        matched_credentials: items,
        asset_links_verified: asset_links_valid,
    })
}

pub fn extract_entry_password(
    entry_id: &str,
    state: &AppState,
) -> Result<Zeroizing<String>, String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let uuid = Uuid::parse_str(entry_id).map_err(|e| e.to_string())?;
    let mut entry = manager.get_entry(uuid).map_err(|e| e.to_string())?;
    let secret = Zeroizing::new(std::mem::take(&mut entry.password));
    if let Some(mut totp) = entry.totp_secret.take() {
        totp.zeroize();
    }
    for field in &mut entry.custom_fields {
        if field.sensitive {
            field.value.zeroize();
        }
    }
    Ok(secret)
}

pub fn extract_entry_autotype_smart(
    entry_id: &str,
    state: &AppState,
) -> Result<(String, String, String, String), String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let uuid = Uuid::parse_str(entry_id).map_err(|e| e.to_string())?;
    let mut entry = manager.get_entry(uuid).map_err(|e| e.to_string())?;
    let password = std::mem::take(&mut entry.password);
    let totp_sec = entry.totp_secret.take().unwrap_or_default();
    for field in &mut entry.custom_fields {
        if field.sensitive {
            field.value.zeroize();
        }
    }
    Ok((entry.username, password, totp_sec, entry.url))
}

#[tauri::command]
pub async fn copy_entry_password(
    entry_id: String,
    clear_after_secs: Option<u64>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let password = extract_entry_password(&entry_id, &state)?;
    yntra_vault_core::crypto::copy_to_clipboard_defended(&password, true, clear_after_secs)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn copy_entry_username(
    entry_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let username = {
        let vault = state.vault.lock().map_err(|e| e.to_string())?;
        let manager = vault.as_ref().ok_or("Vault is locked")?;
        let uuid = Uuid::parse_str(&entry_id).map_err(|e| e.to_string())?;
        let mut entry = manager.get_entry(uuid).map_err(|e| e.to_string())?;
        entry.password.zeroize();
        if let Some(mut totp) = entry.totp_secret.take() {
            totp.zeroize();
        }
        for field in &mut entry.custom_fields {
            if field.sensitive {
                field.value.zeroize();
            }
        }
        entry.username
    };
    yntra_vault_core::crypto::copy_to_clipboard_defended(&username, false, None)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn copy_entry_totp(
    entry_id: String,
    clear_after_secs: Option<u64>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let code = {
        let vault = state.vault.lock().map_err(|e| e.to_string())?;
        let manager = vault.as_ref().ok_or("Vault is locked")?;
        let uuid = Uuid::parse_str(&entry_id).map_err(|e| e.to_string())?;
        let mut entry = manager.get_entry(uuid).map_err(|e| e.to_string())?;
        entry.password.zeroize();
        for field in &mut entry.custom_fields {
            if field.sensitive {
                field.value.zeroize();
            }
        }
        let secret = entry.totp_secret.ok_or("No TOTP secret for this entry")?;
        let config = TotpConfig {
            secret: secret.clone(),
            ..Default::default()
        };
        let code = totp::generate_totp(&config).map_err(|e| e.to_string())?;
        let mut mut_secret = secret;
        mut_secret.zeroize();
        code
    };
    yntra_vault_core::crypto::copy_to_clipboard_defended(&code.code, true, clear_after_secs)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn autotype_entry_password(
    entry_id: String,
    char_delay_ms: Option<u64>,
    settle_delay_ms: Option<u64>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Release vault lock before starting autotype delays to avoid deadlocking with focus-loss auto-lock.
    let password = extract_entry_password(&entry_id, &state)?;
    let char_delay = char_delay_ms.unwrap_or(15);
    let settle_delay = settle_delay_ms.unwrap_or(3000);

    tokio::task::spawn_blocking(move || {
        yntra_vault_core::services::autotype::autotype_text_with_delay(
            &password,
            char_delay,
            settle_delay,
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn autotype_entry_smart(
    entry_id: String,
    launch_browser: Option<bool>,
    char_delay_ms: Option<u64>,
    field_delay_ms: Option<u64>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Release vault lock before initiating autotype execution.
    let (username, password, totp_sec, url) = extract_entry_autotype_smart(&entry_id, &state)?;

    yntra_vault_core::services::autotype::run_smart_autotype_with_delays(
        username,
        password,
        totp_sec,
        url,
        launch_browser.unwrap_or(false),
        char_delay_ms.unwrap_or(15),
        field_delay_ms.unwrap_or(300),
    ).map_err(|e| e.to_string())
}
