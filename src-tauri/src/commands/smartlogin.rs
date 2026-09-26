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

use super::AppState;

struct RunningLogin(Arc<std::sync::atomic::AtomicBool>);
impl Drop for RunningLogin {
    fn drop(&mut self) { self.0.store(false, Ordering::Release); }
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

    // Reset cancel flag
    state.smart_login_cancel.store(false, Ordering::Relaxed);
    let cancel_flag = Arc::clone(&state.smart_login_cancel);

    // Decrypt entry to get credentials — hold lock briefly
    let (url, identifier, password, totp_secret) = {
        let vault = state.vault.lock().map_err(|e| e.to_string())?;
        let manager = vault.as_ref().ok_or("Vault is locked")?;
        let uuid = Uuid::parse_str(&entry_id).map_err(|e| e.to_string())?;
        let mut entry = manager.get_entry(uuid).map_err(|e| e.to_string())?;

        let url = entry.url.clone();
        let ident = if smartlogin::engine::uses_native_browser(&entry.url) && !entry.email.is_empty() {
            entry.email.clone()
        } else if !entry.username.is_empty() {
            entry.username.clone()
        } else {
            entry.email.clone()
        };
        let pass = entry.password.clone();
        let totp = entry.totp_secret.take().map(Zeroizing::new);

        entry.password.zeroize();
        entry.username.zeroize();
        entry.email.zeroize();

        (url, Zeroizing::new(ident), Zeroizing::new(pass), totp)
    };

    if url.is_empty() {
        return Err("Entry has no URL".into());
    }

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
