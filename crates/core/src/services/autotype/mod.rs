//! Autotype Engine for simulated keyboard typing with configurable delays.
//!
//! Callers must wrap sensitive strings (username, password) in
//! `zeroize::Zeroizing<String>` to ensure they are wiped from memory after use.

#[cfg(target_os = "windows")]
mod sys_windows;
#[cfg(target_os = "windows")]
pub(crate) use sys_windows::{foreground_browser_token, send_enter_guarded, type_browser_field};
#[cfg(target_os = "windows")]
pub(crate) use sys_windows::verify_browser_submit;
#[cfg(target_os = "windows")]
pub(crate) use sys_windows::run_native_google_login;
#[cfg(all(test, target_os = "windows"))]
pub(crate) use sys_windows::observe_test_window;
#[cfg(target_os = "windows")]
use sys_windows::WindowsAutotypeDriver as PlatformDriver;

#[cfg(any(not(target_os = "windows"), test))]
mod sys_stub;
#[cfg(not(target_os = "windows"))]
use sys_stub::StubAutotypeDriver as PlatformDriver;

/// Guard holding sensitive autotype credentials wrapped in Zeroizing.
pub(crate) struct AutotypeGuard {
    pub(crate) username: zeroize::Zeroizing<String>,
    pub(crate) password: zeroize::Zeroizing<String>,
    pub(crate) totp_secret: zeroize::Zeroizing<String>,
}

impl AutotypeGuard {
    pub(crate) fn new(username: String, password: String, totp_secret: String) -> Self {
        Self {
            username: zeroize::Zeroizing::new(username),
            password: zeroize::Zeroizing::new(password),
            totp_secret: zeroize::Zeroizing::new(totp_secret),
        }
    }
}

/// Internal driver trait for OS-specific autotype implementations.
pub(crate) trait AutotypeDriver: Send + Sync {
    /// Send keystrokes for text with specified character and settling delays.
    fn autotype_text_with_delay(
        &self,
        text: &str,
        char_delay_ms: u64,
        settle_delay_ms: u64,
    ) -> crate::Result<()>;

    /// Run smart autotype targeting focused or detected application credential fields.
    fn run_smart_autotype(
        &self,
        guard: AutotypeGuard,
        url: &str,
        launch_browser: bool,
        char_delay_ms: u64,
        field_delay_ms: u64,
    ) -> crate::Result<()>;
}

fn driver() -> &'static PlatformDriver {
    static DRIVER: PlatformDriver = PlatformDriver::new();
    &DRIVER
}

// ─── Public API ─────────────────────────────────────────────────────────────

pub fn autotype_text(text: &str) -> crate::Result<()> {
    autotype_text_with_delay(text, 15, 0)
}

pub fn autotype_text_with_delay(text: &str, char_delay_ms: u64, settle_delay_ms: u64) -> crate::Result<()> {
    driver().autotype_text_with_delay(text, char_delay_ms, settle_delay_ms)
}

pub fn run_smart_autotype(username: String, password: String) -> crate::Result<()> {
    run_smart_autotype_with_delays(username, password, String::new(), String::new(), true, 15, 300)
}

pub fn run_smart_autotype_with_delays(
    username: String,
    password: String,
    totp_secret: String,
    url: String,
    launch_browser: bool,
    char_delay_ms: u64,
    field_delay_ms: u64,
) -> crate::Result<()> {
    let guard = AutotypeGuard::new(username, password, totp_secret);
    driver().run_smart_autotype(guard, &url, launch_browser, char_delay_ms, field_delay_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stub_driver() {
        let driver = sys_stub::StubAutotypeDriver::new();
        assert!(driver.autotype_text_with_delay("test", 0, 0).is_ok());
        let guard = AutotypeGuard::new("user".into(), "pass".into(), "totp".into());
        assert!(driver.run_smart_autotype(guard, "", false, 0, 0).is_ok());
    }
}
