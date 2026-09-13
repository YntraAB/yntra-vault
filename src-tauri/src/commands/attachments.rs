use tauri::State;
use uuid::Uuid;

use yntra_vault_core::vault::types::AttachmentInfo;

use super::AppState;

#[tauri::command]
pub async fn get_attachment_data(
    entry_id: String,
    attachment_id: String,
    state: State<'_, AppState>,
) -> Result<tauri::ipc::Response, String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let e_uuid = Uuid::parse_str(&entry_id).map_err(|e| e.to_string())?;
    let a_uuid = Uuid::parse_str(&attachment_id).map_err(|e| e.to_string())?;
    let data = manager.get_attachment_data(e_uuid, a_uuid).map_err(|e| e.to_string())?;
    Ok(tauri::ipc::Response::new(data))
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
