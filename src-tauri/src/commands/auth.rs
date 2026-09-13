use std::path::PathBuf;
use tauri::State;
use zeroize::Zeroize;

use yntra_vault_core::vault::manager::VaultManager;
use yntra_vault_core::vault::types::VaultInfo;

use super::AppState;

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
    if VaultManager::is_biometric_enabled_file(&vault_path) {
        return Ok(true);
    }
    if let Ok(canonical) = vault_path.canonicalize() {
        if VaultManager::is_biometric_enabled_file(&canonical) {
            return Ok(true);
        }
    }
    Ok(false)
}

#[tauri::command]
pub async fn unlock_vault_biometric(
    window: tauri::Window,
    path: String,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let _ = window.set_focus();
    let hwnd_raw = window.hwnd().map(|h| h.0 as isize).ok();
    let vault_path = PathBuf::from(&path);
    let manager = VaultManager::open_with_biometric_with_hwnd(&vault_path, hwnd_raw)
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
pub async fn enable_biometric(
    window: tauri::Window,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let _ = window.set_focus();
    let hwnd_raw = window.hwnd().map(|h| h.0 as isize).ok();
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    manager.enable_biometric_with_hwnd(hwnd_raw).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn disable_biometric(state: State<'_, AppState>) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    manager.disable_biometric().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_hardware2fa_available() -> Result<yntra_vault_core::crypto::hardware2fa::Hardware2FaInfo, String> {
    Ok(yntra_vault_core::crypto::hardware2fa::check_hardware2fa_availability())
}

#[tauri::command]
pub async fn list_hardware_keys() -> Result<Vec<yntra_vault_core::crypto::hardware2fa::HardwareKeyInfo>, String> {
    Ok(yntra_vault_core::crypto::hardware2fa::list_hardware_keys())
}

#[tauri::command]
pub async fn is_hardware2fa_enabled(path: String, state: State<'_, AppState>) -> Result<bool, String> {
    let vault_path = PathBuf::from(&path);
    if let Ok(vault_lock) = state.vault.lock() {
        if let Some(ref manager) = *vault_lock {
            if manager.is_hardware2fa_enabled() {
                return Ok(true);
            }
        }
    }
    if VaultManager::is_hardware2fa_enabled_file(&vault_path) {
        return Ok(true);
    }
    if let Ok(canonical) = vault_path.canonicalize() {
        if VaultManager::is_hardware2fa_enabled_file(&canonical) {
            return Ok(true);
        }
    }
    Ok(false)
}

#[tauri::command]
pub async fn get_hardware2fa_challenge(path: String) -> Result<Option<yntra_vault_core::vault::auth::Hardware2FaChallengeInfo>, String> {
    let vault_path = PathBuf::from(&path);
    VaultManager::get_hardware2fa_challenge_info(&vault_path).map_err(|e| e.to_string())
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
    credential_id: Option<Vec<u8>>,
) -> Result<Vec<u8>, String> {
    let proto = match protocol.as_str() {
        "Fido2Ctap2HmacSecret" | "fido2" => yntra_vault_core::crypto::hardware2fa::Hardware2FaProtocol::Fido2Ctap2HmacSecret,
        _ => yntra_vault_core::crypto::hardware2fa::Hardware2FaProtocol::YubiKeyChallengeResponse,
    };
    let chall = challenge.unwrap_or_else(|| b"yntra-vault-hardware2fa-default-challenge".to_vec());
    yntra_vault_core::crypto::hardware2fa::perform_hardware2fa_challenge_with_cred(
        proto,
        &chall,
        credential_id.as_deref(),
    ).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn enable_hardware2fa(
    mut password: String,
    key_file_path: Option<String>,
    protocol: String,
    key_name: String,
    challenge_salt: Option<Vec<u8>>,
    credential_id: Option<Vec<u8>>,
    hardware_response: Vec<u8>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    let proto = match protocol.as_str() {
        "Fido2Ctap2HmacSecret" | "fido2" => yntra_vault_core::crypto::hardware2fa::Hardware2FaProtocol::Fido2Ctap2HmacSecret,
        _ => yntra_vault_core::crypto::hardware2fa::Hardware2FaProtocol::YubiKeyChallengeResponse,
    };
    let kf_path = key_file_path.as_ref().map(PathBuf::from);

    let salt_bytes: [u8; 32] = match challenge_salt {
        Some(s) if s.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&s);
            arr
        }
        _ => yntra_crypto::kdf::generate_salt(),
    };

    let res = manager.enable_hardware2fa_with_password(
        &password,
        kf_path.as_deref(),
        proto,
        &key_name,
        salt_bytes,
        credential_id.unwrap_or_default(),
        &hardware_response,
    ).map_err(|e| e.to_string());
    password.zeroize();
    res
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

#[tauri::command]
pub async fn split_master_password(password: String) -> Result<Vec<String>, String> {
    yntra_vault_core::crypto::split_password(&password).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reconstruct_master_password(share_a: String, share_b: String) -> Result<String, String> {
    yntra_vault_core::crypto::reconstruct_password(&share_a, &share_b).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reconstruct_master_password_hash(share_a: String, share_b: String) -> Result<String, String> {
    yntra_vault_core::crypto::reconstruct_password_to_hex(&share_a, &share_b).map_err(|e| e.to_string())
}

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
