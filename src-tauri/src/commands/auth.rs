use std::path::PathBuf;
use tauri::{Manager, State};
use zeroize::{Zeroize, Zeroizing};

use yntra_vault_core::vault::manager::VaultManager;
use yntra_vault_core::vault::types::VaultInfo;

use super::AppState;

fn decode_password_bytes(bytes: Zeroizing<Vec<u8>>) -> Result<Zeroizing<String>, String> {
    let password = std::str::from_utf8(&bytes).map_err(|_| "Password not valid UTF-8".to_string())?;
    Ok(Zeroizing::new(password.to_owned()))
}

pub(super) fn vault_storage_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let base = app.path().app_data_dir();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let base = app.path().document_dir().or_else(|_| app.path().app_data_dir());
    let directory = base.map_err(|e| e.to_string())?.join("YntraVault");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    Ok(directory)
}

#[tauri::command]
pub fn get_mobile_vault_path(app: tauri::AppHandle, name: String) -> Result<Option<String>, String> {
    if !cfg!(any(target_os = "android", target_os = "ios")) {
        return Ok(None);
    }
    let name = yntra_vault_core::services::sync::sanitize_vault_filename(&name);
    let path = vault_storage_dir(&app)?.join(format!("{}-{}.vdb", name, uuid::Uuid::new_v4()));
    Ok(Some(path.to_string_lossy().into_owned()))
}

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
    if let Ok(vault_lock) = state.vault.lock()
        && let Some(ref manager) = *vault_lock
            && manager.is_biometric_enabled() {
                return Ok(true);
            }
    if VaultManager::is_biometric_enabled_file(&vault_path) {
        return Ok(true);
    }
    if let Ok(canonical) = vault_path.canonicalize()
        && VaultManager::is_biometric_enabled_file(&canonical) {
            return Ok(true);
        }
    Ok(false)
}

fn get_window_hwnd(_window: &tauri::Window) -> Option<isize> {
    #[cfg(target_os = "windows")]
    {
        _window.hwnd().map(|h| h.0 as isize).ok()
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

#[tauri::command]
pub async fn unlock_vault_biometric(
    window: tauri::Window,
    path: String,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    if let Ok(visible) = window.is_visible() {
        if !visible {
            return Err("Window is hidden".to_string());
        }
    }
    let _ = window.set_focus();
    let hwnd_raw = get_window_hwnd(&window);
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
    #[cfg(desktop)]
    let _ = window.set_always_on_top(true);
    let hwnd_raw = get_window_hwnd(&window);
    let msg = prompt.unwrap_or_else(|| "Unlock Yntra Vault".to_string());
    let res = yntra_vault_core::crypto::biometric::request_user_consent_with_hwnd(&msg, hwnd_raw);
    #[cfg(desktop)]
    let _ = window.set_always_on_top(false);
    res.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn enable_biometric(
    window: tauri::Window,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let _ = window.set_focus();
    let hwnd_raw = get_window_hwnd(&window);
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
    if let Ok(vault_lock) = state.vault.lock()
        && let Some(ref manager) = *vault_lock
            && manager.is_hardware2fa_enabled() {
                return Ok(true);
            }
    if VaultManager::is_hardware2fa_enabled_file(&vault_path) {
        return Ok(true);
    }
    if let Ok(canonical) = vault_path.canonicalize()
        && VaultManager::is_hardware2fa_enabled_file(&canonical) {
            return Ok(true);
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
#[allow(clippy::too_many_arguments)]
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
    state.smart_login_cancel.store(true, std::sync::atomic::Ordering::Release);
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
pub async fn generate_emergency_kit(
    mut master_password: String,
    state: State<'_, AppState>,
) -> Result<yntra_vault_core::vault::EmergencyKit, String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    let res = manager.generate_emergency_kit(&master_password).map_err(|e| e.to_string());
    master_password.zeroize();
    res
}

#[tauri::command]
pub async fn get_emergency_kit_audit(
    state: State<'_, AppState>,
) -> Result<Option<yntra_vault_core::vault::EmergencyKitAudit>, String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    Ok(manager.get_emergency_kit_audit())
}

#[tauri::command]
pub async fn reset_emergency_kit_audit(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    manager.reset_emergency_kit_audit().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_vault_bytes(
    name: String,
    password_bytes: Vec<u8>,
    path: String,
    key_file_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let password = decode_password_bytes(Zeroizing::new(password_bytes))?;

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
    password_bytes: Vec<u8>,
    key_file_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<VaultInfo, String> {
    let password = decode_password_bytes(Zeroizing::new(password_bytes))?;

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
    current_bytes: Vec<u8>,
    new_password_bytes: Vec<u8>,
    current_key_file: Option<String>,
    new_key_file: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let current_bytes = Zeroizing::new(current_bytes);
    let new_password_bytes = Zeroizing::new(new_password_bytes);
    let current = decode_password_bytes(current_bytes)?;
    let new_password = decode_password_bytes(new_password_bytes)?;

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
