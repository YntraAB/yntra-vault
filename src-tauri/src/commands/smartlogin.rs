//! Smart Login Commands — Chrome DevTools Protocol (CDP) automated browser login.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

use yntra_vault_core::smartlogin::{
    self, PreCheckResult, SmartLoginConfig, SmartLoginEngine, SmartLoginEvent,
    logging::{EventCallback, SmartLoginLogger},
};
use yntra_vault_core::vault::types::FieldType;

use super::AppState;

struct RunningLogin(Arc<std::sync::atomic::AtomicBool>);
impl Drop for RunningLogin {
    fn drop(&mut self) { self.0.store(false, Ordering::Release); }
}

struct LoginCredentials {
    url: String,
    identifier: Zeroizing<String>,
    password: Zeroizing<String>,
    totp_secret: Option<Zeroizing<String>>,
}

fn prepare_login(state: &AppState, entry_id: &str) -> Result<LoginCredentials, String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let uuid = Uuid::parse_str(entry_id).map_err(|e| e.to_string())?;
    let mut entry = manager.get_entry(uuid).map_err(|e| e.to_string())?;
    let native_browser = smartlogin::engine::uses_native_browser(&entry.url);
    let phone = entry.custom_fields.iter_mut()
        .find(|field| field.field_type == FieldType::Phone && !field.value.is_empty())
        .map(|field| std::mem::take(&mut field.value))
        .unwrap_or_default();
    let credentials = LoginCredentials {
        url: std::mem::take(&mut entry.url),
        identifier: Zeroizing::new(if native_browser && !entry.email.is_empty() {
            std::mem::take(&mut entry.email)
        } else if !entry.username.is_empty() {
            std::mem::take(&mut entry.username)
        } else {
            let email = std::mem::take(&mut entry.email);
            if email.is_empty() { phone } else { email }
        }),
        password: Zeroizing::new(std::mem::take(&mut entry.password)),
        totp_secret: entry.totp_secret.take().map(Zeroizing::new),
    };
    entry.username.zeroize();
    entry.email.zeroize();
    if credentials.url.is_empty() {
        return Err("Entry has no URL".into());
    }

    // Hold the same vault mutex as clear_session: a later lock must win.
    state.smart_login_cancel.store(false, Ordering::Release);
    yntra_vault_core::services::autotype::begin_autotype();
    Ok(credentials)
}

#[tauri::command]
pub async fn smart_login_precheck(entry_id: Option<String>, state: State<'_, AppState>) -> Result<PreCheckResult, String> {
    let mut result = smartlogin::browser::precheck();
    if let Some(entry_id) = entry_id {
        let vault = state.vault.lock().map_err(|e| e.to_string())?;
        let manager = vault.as_ref().ok_or("Vault is locked")?;
        let uuid = Uuid::parse_str(&entry_id).map_err(|e| e.to_string())?;
        let entry = manager.list_entries().map_err(|e| e.to_string())?.into_iter()
            .find(|entry| entry.id == uuid).ok_or("Entry not found")?;
        if smartlogin::engine::uses_native_browser(&entry.url) { result.needs_close = false; }
    }
    Ok(result)
}

#[tauri::command]
pub async fn smart_login_close_browser(process_name: String) -> Result<(), String> {
    let clean_name = process_name.trim();
    if clean_name.is_empty() || clean_name.len() > 64 {
        return Err("Invalid process name length".into());
    }
    smartlogin::browser::close_browser(clean_name)
}

#[tauri::command]
pub async fn smart_login_start(
    entry_id: String,
    browser_index: usize,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    state.smart_login_running.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| "A Smart Login attempt is already running")?;
    let running = RunningLogin(Arc::clone(&state.smart_login_running));
    // Discover browsers and pick the selected one
    let browsers = smartlogin::browser::discover_browsers();
    let browser_info = browsers
        .get(browser_index)
        .ok_or("Selected browser not found")?
        .clone();

    let cancel_flag = Arc::clone(&state.smart_login_cancel);

    // Decrypt entry to get credentials — hold lock briefly
    let LoginCredentials { url, identifier, password, totp_secret } = prepare_login(&state, &entry_id)?;

    // Set up event callback that emits to the frontend
    let app_handle = app.clone();
    let callback: EventCallback = Box::new(move |event: SmartLoginEvent| {
        let _ = app_handle.emit("smart-login-progress", &event);
    });

    let config = SmartLoginConfig::default();
    let logger = SmartLoginLogger::new(callback);
    let engine = SmartLoginEngine::new(config, logger, cancel_flag);

    // Spawn the login flow as a background task
    let app_handle = app.clone();
    tokio::spawn(async move {
        let _running = running;
        let result = engine.execute(&url, identifier, password, totp_secret, &browser_info).await;
        let _ = app_handle.emit("smart-login-result", &result);
    });

    Ok(())
}

#[tauri::command]
pub async fn smart_login_cancel(state: State<'_, AppState>) -> Result<(), String> {
    state.smart_login_cancel.store(true, Ordering::Relaxed);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, atomic::AtomicBool};
    use yntra_vault_core::{services::autotype, vault::{VaultManager, entry::NewEntry}};

    #[test]
    fn smart_login_after_unlock_rearms_input_but_a_later_lock_still_cancels() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("smart-login-fixture.vdb");
        let master = "Synthetic-Master-Password!";
        let mut manager = VaultManager::create("Login fixture", master, &path).unwrap();
        let id = manager.add_entry(NewEntry {
            title: "Synthetic Google entry".into(),
            username: "fixture-user".into(), email: "fixture@example.test".into(),
            password: "Synthetic-Entry-Password!".into(), url: "https://gmail.com".into(),
            notes: String::new(), tags: vec![], totp_secret: None, custom_fields: vec![],
            entry_type: None, generate_passkey: None, attachments: None,
        }).unwrap().to_string();
        let state = AppState {
            vault: Mutex::new(Some(manager)),
            minimize_to_tray: AtomicBool::new(false),
            lock_on_focus_loss: AtomicBool::new(false),
            lock_on_system_lock: AtomicBool::new(false),
            smart_login_cancel: Default::default(), smart_login_running: Default::default(),
            pairing_operation: Default::default(), sync_listener_operation: Default::default(),
            sync_client_operation: Default::default(), qr_pairing_session: Mutex::new(None),
            pending_adopted_vault: Mutex::new(None),
        };

        state.clear_session().unwrap();
        assert!(autotype::ensure_input_allowed().is_err());
        assert!(prepare_login(&state, &id).is_err());
        assert!(autotype::ensure_input_allowed().is_err());

        *state.vault.lock().unwrap() = Some(VaultManager::open(&path, master).unwrap());
        assert!(prepare_login(&state, &Uuid::new_v4().to_string()).is_err());
        assert!(autotype::ensure_input_allowed().is_err());
        let credentials = prepare_login(&state, &id).unwrap();
        assert_eq!(credentials.url, "https://gmail.com");
        assert_eq!(credentials.identifier.as_str(), if cfg!(windows) { "fixture@example.test" } else { "fixture-user" });
        assert_eq!(credentials.password.as_str(), "Synthetic-Entry-Password!");
        // This is the exact cancellation gate used before Windows focus checks and SendInput.
        assert!(autotype::ensure_input_allowed().is_ok());
        assert!(!state.smart_login_cancel.load(Ordering::Acquire));

        // A lock between preparation and the background worker cannot be undone by that worker.
        state.clear_session().unwrap();
        assert!(autotype::ensure_input_allowed().is_err());
        assert!(state.smart_login_cancel.load(Ordering::Acquire));
        assert!(prepare_login(&state, &id).is_err());
        assert!(autotype::ensure_input_allowed().is_err());
    }
}
