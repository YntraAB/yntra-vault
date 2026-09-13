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

#[tauri::command]
pub async fn smart_login_precheck() -> Result<PreCheckResult, String> {
    Ok(smartlogin::browser::precheck())
}

#[tauri::command]
pub async fn smart_login_close_browser(process_name: String) -> Result<(), String> {
    smartlogin::browser::close_browser(&process_name)
}

#[tauri::command]
pub async fn smart_login_start(
    entry_id: String,
    browser_index: usize,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
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
        let ident = if !entry.username.is_empty() {
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
