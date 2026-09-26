//! Native platform services. Secrets stay on the native clipboard path.
use tauri::AppHandle;
#[cfg(target_os = "android")]
use tauri::Manager;

#[tauri::command]
pub fn get_runtime_platform() -> &'static str {
    std::env::consts::OS
}

pub struct AutoLockState(std::sync::Mutex<(u64, std::time::Instant)>);
impl Default for AutoLockState {
    fn default() -> Self {
        Self(std::sync::Mutex::new((15 * 60, std::time::Instant::now())))
    }
}
impl AutoLockState {
    pub fn expired(&self) -> bool {
        self.0
            .lock()
            .map(|s| s.0 > 0 && s.1.elapsed().as_secs() >= s.0)
            .unwrap_or(true)
    }
    fn touch(&self) -> Result<(), String> {
        let mut timer = self.0.lock().map_err(|_| "Auto-lock state unavailable")?;
        if timer.0 == 0 || timer.1.elapsed().as_secs() < timer.0 {
            timer.1 = std::time::Instant::now();
        }
        Ok(())
    }
}
#[tauri::command]
pub fn configure_auto_lock(
    seconds: u64,
    state: tauri::State<'_, AutoLockState>,
) -> Result<(), String> {
    *state.0.lock().map_err(|_| "Auto-lock state unavailable")? =
        (seconds.min(86400), std::time::Instant::now());
    Ok(())
}
#[tauri::command]
pub fn record_user_activity(state: tauri::State<'_, AutoLockState>) -> Result<(), String> {
    // A queued touch after waking must never revive an already expired session.
    state.touch()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resumed_activity_cannot_revive_an_expired_session() {
        let state = AutoLockState(std::sync::Mutex::new((
            1,
            std::time::Instant::now() - std::time::Duration::from_secs(2),
        )));
        assert!(state.expired());
        state.touch().unwrap();
        assert!(state.expired());
        state.0.lock().unwrap().0 = 0;
        assert!(!state.expired());
    }
}

#[cfg(target_os = "android")]
pub struct MobileServices(pub tauri::plugin::PluginHandle<tauri::Wry>);

#[cfg(target_os = "android")]
pub fn mobile_services_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("yntra-mobile-services")
        .setup(|app, api| {
            app.manage(MobileServices(api.register_android_plugin(
                "com.yntravault.app",
                "MobileServicesPlugin",
            )?));
            Ok(())
        })
        .build()
}

pub fn copy(
    app: &AppHandle,
    text: &str,
    sensitive: bool,
    seconds: Option<u64>,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    return app.state::<MobileServices>().0.run_mobile_plugin::<serde_json::Value>("copy", serde_json::json!({
        "text": text, "sensitive": sensitive, "seconds": seconds.unwrap_or(if sensitive { 30 } else { 0 }).min(86400)
    })).map(|_| ()).map_err(|e| e.to_string());
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        yntra_vault_core::crypto::copy_to_clipboard_defended(text, sensitive, seconds)
            .map_err(|e| e.to_string())
    }
}

pub fn clear(app: &AppHandle) -> Result<(), String> {
    #[cfg(target_os = "android")]
    return app
        .state::<MobileServices>()
        .0
        .run_mobile_plugin::<serde_json::Value>("clear", ())
        .map(|_| ())
        .map_err(|e| e.to_string());
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        yntra_vault_core::crypto::clear_clipboard().map_err(|e| e.to_string())
    }
}
