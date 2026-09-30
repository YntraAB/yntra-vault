//! Autotype Engine for simulated keyboard typing with configurable delays.
//!
//! Callers must wrap sensitive strings (username, password) in
//! `zeroize::Zeroizing<String>` to ensure they are wiped from memory after use.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

static AUTOTYPE_CANCEL: AtomicBool = AtomicBool::new(false);
static AUTOTYPE_GENERATION: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static AUTOTYPE_SESSION: Cell<u64> = const { Cell::new(0) };
}

/// Start a new foreground autotype operation. The native lock path calls
/// `cancel_autotype` so detached Windows input cannot continue after locking.
pub fn begin_autotype() -> u64 {
    let generation = AUTOTYPE_GENERATION.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
    AUTOTYPE_CANCEL.store(false, Ordering::Release);
    generation
}

pub fn cancel_autotype() {
    AUTOTYPE_CANCEL.store(true, Ordering::Release);
    AUTOTYPE_GENERATION.fetch_add(1, Ordering::AcqRel);
}

pub(crate) fn is_cancelled() -> bool {
    let current = AUTOTYPE_GENERATION.load(Ordering::Acquire);
    AUTOTYPE_CANCEL.load(Ordering::Acquire)
        || AUTOTYPE_SESSION.with(|session| {
            let bound = session.get();
            bound != 0 && bound != current
        })
}

pub(crate) fn bind_autotype_session(generation: u64) {
    AUTOTYPE_SESSION.with(|session| session.set(generation));
}

/// Shared by native input guards and callers checking whether input may start.
pub fn ensure_input_allowed() -> crate::Result<()> {
    if is_cancelled() {
        return Err(crate::error::VaultError::AutoTypeError("Autotype cancelled by vault lock".into()));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
mod sys_windows;
#[cfg(target_os = "windows")]
pub(crate) use sys_windows::{activate_browser_window, foreground_browser_token, send_enter_guarded, type_browser_field};
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
    pub(crate) generation: u64,
    pub(crate) username: zeroize::Zeroizing<String>,
    pub(crate) password: zeroize::Zeroizing<String>,
    pub(crate) totp_secret: zeroize::Zeroizing<String>,
}

impl AutotypeGuard {
    pub(crate) fn new(generation: u64, username: String, password: String, totp_secret: String) -> Self {
        Self {
            generation,
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
    let generation = begin_autotype();
    autotype_text_with_delay_for_session(generation, text, char_delay_ms, settle_delay_ms)
}

pub fn autotype_text_with_delay_for_session(
    generation: u64,
    text: &str,
    char_delay_ms: u64,
    settle_delay_ms: u64,
) -> crate::Result<()> {
    bind_autotype_session(generation);
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
    let generation = begin_autotype();
    run_smart_autotype_with_delays_for_session(
        generation,
        username,
        password,
        totp_secret,
        url,
        launch_browser,
        char_delay_ms,
        field_delay_ms,
    )
}

pub fn run_smart_autotype_with_delays_for_session(
    generation: u64,
    username: String,
    password: String,
    totp_secret: String,
    url: String,
    launch_browser: bool,
    char_delay_ms: u64,
    field_delay_ms: u64,
) -> crate::Result<()> {
    let guard = AutotypeGuard::new(generation, username, password, totp_secret);
    driver().run_smart_autotype(guard, &url, launch_browser, char_delay_ms, field_delay_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stub_driver() {
        begin_autotype();
        let driver = sys_stub::StubAutotypeDriver::new();
        assert!(driver.autotype_text_with_delay("test", 0, 0).is_ok());
        let generation = begin_autotype();
        let guard = AutotypeGuard::new(generation, "user".into(), "pass".into(), "totp".into());
        assert!(driver.run_smart_autotype(guard, "", false, 0, 0).is_ok());
    }

    #[test]
    fn lock_cancellation_stops_the_next_input_check() {
        begin_autotype();
        assert!(!is_cancelled());
        cancel_autotype();
        assert!(is_cancelled());
        begin_autotype();
        assert!(!is_cancelled());
    }

    #[test]
    fn an_old_worker_stays_cancelled_after_a_new_session_starts() {
        let old = begin_autotype();
        bind_autotype_session(old);
        cancel_autotype();
        let current = begin_autotype();
        assert_ne!(old, current);
        assert!(is_cancelled(), "the old worker must not be revived by a new session");

        bind_autotype_session(current);
        assert!(!is_cancelled());
    }
}
