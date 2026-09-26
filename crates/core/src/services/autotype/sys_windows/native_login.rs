//! Ordinary-browser Google login. No debugging port, profile replacement or clipboard.
use super::*;
use crate::smartlogin::ManualActionType;
use crate::smartlogin::native_state::{self, PageState};
use crate::smartlogin::{BrowserInfo, LoginResult, LoginState, logging::SmartLoginLogger};
use std::sync::atomic::{AtomicBool, Ordering};
use zeroize::Zeroizing;
mod observe;

enum AccountChoice {
    Selected,
    Missing(u64),
    Pending,
}

/// Activate only a unique exact account match on Google's actual account chooser.
/// Page navigation is still observed afterward; invoking a row is not login success.
fn choose_account(
    automation: &IUIAutomation,
    window: &IUIAutomationElement,
    hwnd: HWND,
    address: &str,
    identifier: &str,
    cancel: &AtomicBool,
) -> AccountChoice {
    if !native_state::account_chooser(address) {
        return AccountChoice::Pending;
    }
    let mut candidate = None;
    let mut matching_rows = 0;
    let mut account_rows = Vec::new();
    for kind in [UIA_ButtonControlTypeId, UIA_HyperlinkControlTypeId] {
        let Some(elements) = find_targeted_elements(automation, window, kind) else {
            continue;
        };
        for index in 0..unsafe { elements.Length() }.unwrap_or(0).min(120) {
            let Ok(element) = (unsafe { elements.GetElement(index) }) else {
                continue;
            };
            if unsafe { element.CurrentIsOffscreen() }.map_or(true, |v| v.as_bool())
                || !unsafe { element.CurrentIsEnabled() }.is_ok_and(|v| v.as_bool())
                || !document_field(automation, &element)
            {
                continue;
            }
            let name = unsafe { element.CurrentName() }
                .map(|v| v.to_string())
                .unwrap_or_default();
            let identity = native_state::email_control(&name, identifier);
            if identity.is_some() {
                account_rows.push(name);
            }
            if identity != Some(true) {
                continue;
            }
            matching_rows += 1;
            if matching_rows > 1 {
                return AccountChoice::Pending;
            }
            let Ok(pattern) = (unsafe { element.GetCurrentPattern(UIA_InvokePatternId) }) else {
                continue;
            };
            let Ok(invoke) = pattern.cast::<IUIAutomationInvokePattern>() else {
                continue;
            };
            if candidate.is_some() {
                return AccountChoice::Pending;
            }
            candidate = Some(invoke);
        }
    }
    if cancel.load(Ordering::Relaxed)
        || !is_target_window_active(hwnd)
        || browser_address(automation, hwnd).as_deref() != Some(address)
    {
        return AccountChoice::Pending;
    }
    if let Some(invoke) = candidate {
        return if unsafe { invoke.Invoke() }.is_ok() {
            AccountChoice::Selected
        } else {
            AccountChoice::Pending
        };
    }
    if account_rows.is_empty() || matching_rows > 0 {
        return AccountChoice::Pending;
    }
    // Keep only an ephemeral fingerprint for stability checks; never log identities.
    use std::hash::{Hash, Hasher};
    account_rows.sort();
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    account_rows.hash(&mut hash);
    AccountChoice::Missing(hash.finish())
}

#[cfg(test)]
pub(crate) fn observe_test_window(address: &str, identifier: &str) -> PageState {
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
        .ok()
        .unwrap();
    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL) }.unwrap();
    let hwnd = unsafe { GetForegroundWindow() };
    let window = unsafe { automation.ElementFromHandle(hwnd) }.unwrap();
    let result = native_state::classify(&observe::observe(
        &automation,
        &window,
        address.into(),
        identifier,
    ));
    drop(window);
    drop(automation);
    unsafe {
        windows::Win32::System::Com::CoUninitialize();
    }
    result
}

pub(super) fn document_field(automation: &IUIAutomation, element: &IUIAutomationElement) -> bool {
    unsafe {
        let Ok(walker) = automation.ControlViewWalker() else {
            return false;
        };
        let mut current = element.clone();
        for _ in 0..32 {
            let Ok(parent) = walker.GetParentElement(&current) else {
                return false;
            };
            let kind = parent.CurrentControlType().ok();
            if kind == Some(UIA_DocumentControlTypeId) {
                return true;
            }
            if kind == Some(UIA_ToolBarControlTypeId) || kind == Some(UIA_WindowControlTypeId) {
                return false;
            }
            current = parent;
        }
    }
    false
}

pub(crate) fn run_native_google_login(
    browser: &BrowserInfo,
    entry_url: &str,
    identifier: Zeroizing<String>,
    password: Zeroizing<String>,
    cancel: &AtomicBool,
    logger: &SmartLoginLogger,
) -> LoginResult {
    if cancel.load(Ordering::Relaxed) {
        return LoginResult::Cancelled;
    }
    if unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_err() {
        return LoginResult::BrowserError("Cannot initialize keyboard input".into());
    }
    struct ComGuard;
    impl Drop for ComGuard {
        fn drop(&mut self) {
            unsafe {
                windows::Win32::System::Com::CoUninitialize();
            }
        }
    }
    let _com = ComGuard;
    let automation: IUIAutomation =
        match unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL) } {
            Ok(value) => value,
            Err(_) => return LoginResult::BrowserError("Cannot inspect browser fields".into()),
        };
    logger.log(
        LoginState::LaunchingBrowser,
        "Opening Google in your ordinary browser; keep its login window focused",
    );
    // Do not terminate the user's browser or create an automation profile.
    let landing = native_state::landing_url(entry_url);
    let start_with_add_session = false;
    // Opt-in diagnostic exercises the actual blank-form branch even when the
    // test account already has a session. This override is absent from app builds.
    #[cfg(test)]
    let start_with_add_session = start_with_add_session
        || std::env::var("YNTRA_GOOGLE_TEST_BLANK_LOGIN").as_deref() == Ok("1");
    let start_url = native_state::account_navigation(entry_url, start_with_add_session);
    let mut previous_window = unsafe { GetForegroundWindow() };
    if std::process::Command::new(&browser.exe_path)
        .arg("--new-window")
        .arg(&start_url)
        .spawn()
        .is_err()
    {
        return LoginResult::BrowserError("Cannot open the selected browser".into());
    }
    let mut deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
    let mut target = HWND::default();
    let mut add_session_started = start_with_add_session;
    let mut account_selection_attempted = false;
    let mut identifier_sent = identifier.is_empty();
    let mut password_sent = false;
    let mut previous_state = PageState::Unknown;
    let mut stable_count = 0;
    let mut challenge_seen = false;
    let attempt_started = std::time::Instant::now();
    let mut chooser_wait = native_state::ChooserWait::default();
    let mut observed_address = String::new();
    while std::time::Instant::now() < deadline {
        if cancel.load(Ordering::Relaxed) {
            return LoginResult::Cancelled;
        }
        std::thread::sleep(std::time::Duration::from_millis(if password_sent {
            250
        } else {
            100
        }));
        let foreground = unsafe { GetForegroundWindow() };
        // A bounded AddSession transition opens a new ordinary window. Never
        // bind the old account window again while the new window is starting.
        if target.is_invalid() && foreground == previous_window {
            continue;
        }
        // After submission, observation can continue without sending keys while another window is active.
        let hwnd = if password_sent && !target.is_invalid() {
            target
        } else {
            foreground
        };
        if !target.is_invalid() && hwnd != target {
            return LoginResult::Cancelled;
        }
        if get_window_process_name(hwnd) != browser.process_name.to_lowercase() {
            continue;
        }
        let Some(address) = browser_address(&automation, hwnd) else {
            continue;
        };
        let auth_host = native_state::account_host(&address);
        if observed_address != address {
            chooser_wait = native_state::ChooserWait::default();
            stable_count = 0;
            previous_state = PageState::Unknown;
            observed_address.clone_from(&address);
        }
        if !auth_host && !native_state::service_host(&address) {
            // Chromium can expose a transitional document during navigation.
            // Wait within the deadline; never inspect or type into that page.
            continue;
        }
        target = hwnd;
        let Ok(window) = (unsafe { automation.ElementFromHandle(hwnd) }) else {
            continue;
        };
        let signals = observe::observe(&automation, &window, address.clone(), &identifier);
        if cancel.load(Ordering::Relaxed) {
            return LoginResult::Cancelled;
        }
        let current = native_state::classify(&signals);
        #[cfg(test)]
        if current != previous_state {
            eprintln!("Native page state: {current:?}");
        }
        if current == previous_state {
            stable_count += 1;
        } else {
            stable_count = 0;
        }
        let changed = current != previous_state;
        previous_state = current;
        let mut chooser_needs_login = false;
        if current == PageState::Unknown
            && !identifier_sent
            && !password_sent
            && native_state::account_chooser(&address)
        {
            let choice = if account_selection_attempted {
                AccountChoice::Pending
            } else {
                choose_account(&automation, &window, hwnd, &address, &identifier, cancel)
            };
            match choice {
                AccountChoice::Selected => {
                    account_selection_attempted = true;
                    chooser_wait = native_state::ChooserWait::default();
                    stable_count = 0;
                    logger.log(
                        LoginState::VerifyingResult,
                        "Checking the selected account's existing session",
                    );
                    continue;
                }
                AccountChoice::Missing(fingerprint) => {
                    chooser_needs_login = chooser_wait
                        .ready_for_new_account(attempt_started.elapsed(), Some(fingerprint));
                }
                AccountChoice::Pending => {
                    chooser_needs_login =
                        chooser_wait.ready_for_new_account(attempt_started.elapsed(), None);
                }
            }
        } else {
            chooser_wait = native_state::ChooserWait::default();
        }
        if stable_count >= 2 {
            match current {
                PageState::Authenticated => {
                    logger.log(
                        LoginState::Success,
                        if password_sent {
                            "Sign-in confirmed for the selected account"
                        } else {
                            "The selected account is already signed in"
                        },
                    );
                    // Do not emit URL query/fragment data that may contain private information.
                    return if password_sent {
                        LoginResult::Success {
                            final_url: landing.clone(),
                        }
                    } else {
                        LoginResult::AlreadySignedIn {
                            final_url: landing.clone(),
                        }
                    };
                }
                PageState::OtherAccount | PageState::Unknown | PageState::Password
                    if current == PageState::OtherAccount
                        || chooser_needs_login
                        || (current == PageState::Password
                            && !identifier_sent
                            && !native_state::password_account_verified(
                                &signals,
                                account_selection_attempted,
                            )) =>
                {
                    if native_state::can_add_account(
                        add_session_started,
                        identifier_sent || password_sent,
                    ) {
                        if cancel.load(Ordering::Relaxed) || !is_target_window_active(hwnd) {
                            return LoginResult::Cancelled;
                        }
                        logger.log(LoginState::LaunchingBrowser,
                            "Opening sign-in for the selected account without signing other accounts out");
                        let url = native_state::account_navigation(entry_url, true);
                        if std::process::Command::new(&browser.exe_path)
                            .arg("--new-window")
                            .arg(url)
                            .spawn()
                            .is_err()
                        {
                            return LoginResult::BrowserError("Cannot open account sign-in".into());
                        }
                        add_session_started = true;
                        account_selection_attempted = false;
                        previous_window = target;
                        target = HWND::default();
                        previous_state = PageState::Unknown;
                        stable_count = 0;
                        deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
                        continue;
                    }
                    return if current == PageState::OtherAccount {
                        LoginResult::DifferentAccount
                    } else {
                        LoginResult::RequiresManualAction
                    };
                }
                PageState::InvalidCredentials if identifier_sent => {
                    return LoginResult::WrongCredentials {
                        error_message: None,
                    };
                }
                PageState::Locked => {
                    return LoginResult::AccountLocked {
                        message: "Google has temporarily restricted this sign-in".into(),
                    };
                }
                PageState::Blocked => {
                    return LoginResult::BrowserError(
                        "Google rejected this browser session".into(),
                    );
                }
                _ => {}
            }
        }
        if matches!(current, PageState::Captcha | PageState::Mfa) {
            if changed {
                logger.log(
                    LoginState::RequiresManualAction(if current == PageState::Captcha {
                        ManualActionType::Captcha
                    } else {
                        ManualActionType::TwoFactorAuth
                    }),
                    if current == PageState::Captcha {
                        "Complete the robot check in the browser; waiting for the result"
                    } else {
                        "Complete two-step verification in the browser; waiting for the result"
                    },
                );
            }
            if !challenge_seen {
                deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
                challenge_seen = true;
            }
            continue;
        }
        // Never retry submitted credentials or fill a field while an error/challenge is displayed.
        if password_sent
            || !auth_host
            || !matches!(current, PageState::Identifier | PageState::Password)
        {
            continue;
        }
        // A verified chooser selection is equivalent to submitting its identifier.
        // Otherwise a direct password step needs its own matching account chip.
        // An explicit mismatch also blocks filling after our identifier step.
        if current == PageState::Password {
            if signals.other_account {
                return LoginResult::DifferentAccount;
            }
            if !native_state::password_account_verified(
                &signals,
                identifier_sent || account_selection_attempted,
            ) {
                continue;
            }
            identifier_sent = true;
        }
        let Some(edits) = find_targeted_elements(&automation, &window, UIA_EditControlTypeId)
        else {
            continue;
        };
        for index in 0..unsafe { edits.Length() }.unwrap_or(0).min(80) {
            let Ok(field) = (unsafe { edits.GetElement(index) }) else {
                continue;
            };
            if unsafe { field.CurrentIsOffscreen() }.map_or(true, |v| v.as_bool())
                || !unsafe { field.CurrentIsEnabled() }.is_ok_and(|v| v.as_bool())
                || !document_field(&automation, &field)
            {
                continue;
            }
            let protected = unsafe { field.CurrentIsPassword() }.is_ok_and(|v| v.as_bool());
            let id = unsafe { field.CurrentAutomationId() }
                .map(|v| v.to_string())
                .unwrap_or_default();
            if (!identifier_sent && (id != "identifierId" || protected))
                || (identifier_sent && !protected)
            {
                continue;
            }
            if protected && password.is_empty() {
                logger.log(
                    LoginState::WaitingForPasswordStep,
                    "Visible password field reached; no password supplied, stopping here",
                );
                return LoginResult::RequiresManualAction;
            }
            if unsafe { field.SetFocus() }.is_err() {
                continue;
            }
            let focus_deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            while check_identifier_focus(&field, hwnd).is_err()
                && std::time::Instant::now() < focus_deadline
            {
                if cancel.load(Ordering::Relaxed) || !is_target_window_active(hwnd) {
                    return LoginResult::Cancelled;
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            logger.log(
                if protected {
                    LoginState::FillingPassword
                } else {
                    LoginState::FillingIdentifier
                },
                if protected {
                    "Typing password with the system keyboard"
                } else {
                    "Typing identifier with the system keyboard"
                },
            );
            // Keep cancellation and the selected field checked for every character.
            let text = if protected { &password } else { &identifier };
            if text.chars().any(char::is_control) {
                return LoginResult::BrowserError(
                    "Control characters cannot be typed safely".into(),
                );
            }
            let result = (|| -> crate::Result<()> {
                check_identifier_focus(&field, hwnd)?;
                if browser_address(&automation, hwnd).as_deref() != Some(address.as_str()) {
                    return Err(crate::error::VaultError::AutoTypeError(
                        "Browser address changed".into(),
                    ));
                }
                if let Ok(pattern) = unsafe { field.GetCurrentPattern(UIA_ValuePatternId) }
                    && let Ok(pattern) = pattern.cast::<IUIAutomationValuePattern>()
                    && unsafe { pattern.CurrentIsReadOnly() }.map_or(true, |v| v.as_bool())
                {
                    return Err(crate::error::VaultError::AutoTypeError(
                        "Field is read-only".into(),
                    ));
                }
                check_modifier_keys_released()?;
                send_ctrl_a_backspace_guarded(hwnd)?;
                for ch in text.chars() {
                    if cancel.load(Ordering::Relaxed) {
                        return Err(crate::error::VaultError::AutoTypeError("Cancelled".into()));
                    }
                    check_identifier_focus(&field, hwnd)?;
                    if unsafe { field.CurrentIsPassword() }
                        .map(|v| v.as_bool())
                        .ok()
                        != Some(protected)
                    {
                        return Err(crate::error::VaultError::AutoTypeError(
                            "Field type changed".into(),
                        ));
                    }
                    let mut encoded = Zeroizing::new([0u8; 4]);
                    autotype_text_with_delay_guarded(
                        ch.encode_utf8(&mut encoded[..]),
                        25,
                        0,
                        hwnd,
                    )?;
                }
                safe_sleep_with_target_guard(150, hwnd)?;
                check_identifier_focus(&field, hwnd)?;
                if !protected && Zeroizing::new(get_element_value(&field)).as_str() != text.as_str()
                {
                    return Err(crate::error::VaultError::AutoTypeError(
                        "Identifier input was rejected".into(),
                    ));
                }
                if cancel.load(Ordering::Relaxed) {
                    return Err(crate::error::VaultError::AutoTypeError("Cancelled".into()));
                }
                if browser_address(&automation, hwnd).as_deref() != Some(address.as_str()) {
                    return Err(crate::error::VaultError::AutoTypeError(
                        "Browser address changed".into(),
                    ));
                }
                send_enter_guarded(hwnd)
            })();
            if result.is_err() {
                return if cancel.load(Ordering::Relaxed) {
                    LoginResult::Cancelled
                } else {
                    LoginResult::BrowserError(
                        "Typing stopped because the input or focus changed".into(),
                    )
                };
            }
            if protected {
                password_sent = true;
                logger.log(
                    LoginState::VerifyingResult,
                    "Checking the browser response and account identity",
                );
                deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
                break;
            }
            identifier_sent = true;
            logger.log(
                LoginState::WaitingForPasswordStep,
                "Waiting for the password field; complete any robot check in the browser",
            );
            break;
        }
    }
    match previous_state {
        PageState::Captcha => LoginResult::RequiresCaptcha,
        PageState::Mfa => LoginResult::RequiresMfa {
            mfa_type: "Google verification".into(),
        },
        _ => LoginResult::RequiresManualAction,
    }
}
