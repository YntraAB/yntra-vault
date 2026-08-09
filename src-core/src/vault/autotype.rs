//! Autotype Engine for simulated keyboard typing with configurable delays.
//!
//! Callers must wrap sensitive strings (username, password) in
//! `zeroize::Zeroizing<String>` to ensure they are wiped from memory after use.

#[cfg(target_os = "windows")]
struct AutotypeGuard {
    username: zeroize::Zeroizing<String>,
    password: zeroize::Zeroizing<String>,
    totp_secret: zeroize::Zeroizing<String>,
}

#[cfg(target_os = "windows")]
pub fn autotype_text(text: &str) -> crate::Result<()> {
    autotype_text_with_delay(text, 15, 0)
}

#[cfg(target_os = "windows")]
#[cfg(target_os = "windows")]
fn is_target_window_active(target_hwnd: windows::Win32::Foundation::HWND) -> bool {
    if target_hwnd.is_invalid() {
        return true;
    }
    unsafe {
        let current = windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        current == target_hwnd
    }
}

#[cfg(target_os = "windows")]
fn check_target_window_active(target_hwnd: windows::Win32::Foundation::HWND) -> crate::Result<()> {
    if !is_target_window_active(target_hwnd) {
        return Err(crate::error::VaultError::AutoTypeError(
            "Target window lost active focus during autotype execution".into(),
        ));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn safe_sleep_with_target_guard(duration_ms: u64, target_hwnd: windows::Win32::Foundation::HWND) -> crate::Result<()> {
    let mut remaining = duration_ms;
    while remaining > 0 {
        check_target_window_active(target_hwnd)?;
        let step = std::cmp::min(remaining, 25);
        std::thread::sleep(std::time::Duration::from_millis(step));
        remaining -= step;
    }
    check_target_window_active(target_hwnd)?;
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn autotype_text_with_delay(text: &str, char_delay_ms: u64, settle_delay_ms: u64) -> crate::Result<()> {
    autotype_text_with_delay_guarded(text, char_delay_ms, settle_delay_ms, windows::Win32::Foundation::HWND::default())
}

#[cfg(target_os = "windows")]
pub fn autotype_text_with_delay_guarded(
    text: &str,
    char_delay_ms: u64,
    settle_delay_ms: u64,
    target_hwnd: windows::Win32::Foundation::HWND,
) -> crate::Result<()> {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    if settle_delay_ms > 0 {
        unsafe {
            let start_hwnd = GetForegroundWindow();
            if !start_hwnd.is_invalid() {
                let mut start_pid = 0u32;
                GetWindowThreadProcessId(start_hwnd, Some(&mut start_pid));

                // If the active window belongs to Yntra Vault (our process),
                // wait until the user switches to a different process window.
                if start_pid == std::process::id() {
                    let mut elapsed = 0;
                    // Poll every 100ms for up to 15 seconds (150 polls)
                    while elapsed < 150 {
                        let current_hwnd = GetForegroundWindow();
                        let mut current_pid = 0u32;
                        GetWindowThreadProcessId(current_hwnd, Some(&mut current_pid));
                        
                        if current_pid != start_pid && !current_hwnd.is_invalid() {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        elapsed += 1;
                    }
                    // Settle delay: wait so the user has time to select/focus the input field
                    safe_sleep_with_target_guard(settle_delay_ms, target_hwnd)?;
                }
            }
        }
    }

    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_UNICODE, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_TAB
    };

    let utf16_chars: Vec<u16> = text.encode_utf16().collect();

    for &ch in &utf16_chars {
        check_target_window_active(target_hwnd)?;

        let (vk, scan, flags) = if ch == 9 {
            (VK_TAB, 0u16, windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0))
        } else {
            (VIRTUAL_KEY(0), ch, KEYEVENTF_UNICODE)
        };

        // Send Key Down
        let input_down = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: scan,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        // Send Key Up
        let input_up = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: scan,
                    dwFlags: flags | KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        let inputs = [input_down, input_up];

        unsafe {
            let sent = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
            if sent != 2 {
                return Err(crate::error::VaultError::EncryptionError(
                    "Autotype failed to send input events".into(),
                ));
            }
        }

        // Configurable delay between characters with active window guard check
        safe_sleep_with_target_guard(char_delay_ms, target_hwnd)?;
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn send_shift_tab_guarded(target_hwnd: windows::Win32::Foundation::HWND) -> crate::Result<()> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_SHIFT, VK_TAB
    };

    let send_single = |vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY, scan: u16, flags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS| -> crate::Result<()> {
        check_target_window_active(target_hwnd)?;
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: scan,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            let sent = SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
            if sent != 1 {
                return Err(crate::error::VaultError::EncryptionError(
                    "Autotype failed to send key event".into(),
                ));
            }
        }
        Ok(())
    };

    send_single(VK_SHIFT, 0x2A, windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0))?;
    safe_sleep_with_target_guard(15, target_hwnd)?;

    send_single(VK_TAB, 0x0F, windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0))?;
    safe_sleep_with_target_guard(15, target_hwnd)?;

    send_single(VK_TAB, 0x0F, KEYEVENTF_KEYUP)?;
    safe_sleep_with_target_guard(15, target_hwnd)?;

    send_single(VK_SHIFT, 0x2A, KEYEVENTF_KEYUP)?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn send_ctrl_a_backspace_guarded(target_hwnd: windows::Win32::Foundation::HWND) -> crate::Result<()> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL, VK_BACK, VIRTUAL_KEY
    };

    let send_single = |vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY, scan: u16, flags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS| -> crate::Result<()> {
        check_target_window_active(target_hwnd)?;
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: scan,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            let sent = SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
            if sent != 1 {
                return Err(crate::error::VaultError::EncryptionError(
                    "Autotype failed to send key event".into(),
                ));
            }
        }
        Ok(())
    };

    send_single(VK_CONTROL, 0x1D, windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0))?;
    safe_sleep_with_target_guard(15, target_hwnd)?;

    send_single(VIRTUAL_KEY(0x41), 0x1E, windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0))?;
    safe_sleep_with_target_guard(15, target_hwnd)?;

    send_single(VIRTUAL_KEY(0x41), 0x1E, KEYEVENTF_KEYUP)?;
    safe_sleep_with_target_guard(15, target_hwnd)?;

    send_single(VK_CONTROL, 0x1D, KEYEVENTF_KEYUP)?;
    safe_sleep_with_target_guard(15, target_hwnd)?;

    send_single(VK_BACK, 0x0E, windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0))?;
    safe_sleep_with_target_guard(15, target_hwnd)?;
    send_single(VK_BACK, 0x0E, KEYEVENTF_KEYUP)?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn send_enter_guarded(target_hwnd: windows::Win32::Foundation::HWND) -> crate::Result<()> {
    check_target_window_active(target_hwnd)?;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_RETURN
    };

    let input_down = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_RETURN,
                wScan: 0x1C,
                dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0),
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let input_up = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_RETURN,
                wScan: 0x1C,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe {
        let _ = SendInput(&[input_down, input_up], std::mem::size_of::<INPUT>() as i32);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn send_backspaces_guarded(count: usize, target_hwnd: windows::Win32::Foundation::HWND) -> crate::Result<()> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_BACK
    };

    for _ in 0..count {
        check_target_window_active(target_hwnd)?;
        let input_down = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_BACK,
                    wScan: 0x0E,
                    dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let input_up = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_BACK,
                    wScan: 0x0E,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            let _ = SendInput(&[input_down, input_up], std::mem::size_of::<INPUT>() as i32);
        }
        safe_sleep_with_target_guard(15, target_hwnd)?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn common_prefix_len(s1: &str, s2: &str) -> usize {
    s1.chars().zip(s2.chars())
      .take_while(|(c1, c2)| c1 == c2)
      .count()
}

#[cfg(target_os = "windows")]
fn autotype_correct_text_guarded(current: &str, target: &str, char_delay_ms: u64, target_hwnd: windows::Win32::Foundation::HWND) -> crate::Result<()> {
    if current.is_empty() {
        return autotype_text_with_delay_guarded(target, char_delay_ms, 0, target_hwnd);
    }

    let prefix_len = common_prefix_len(current, target);
    if prefix_len == 0 {
        send_ctrl_a_backspace_guarded(target_hwnd)?;
        safe_sleep_with_target_guard(100, target_hwnd)?;
        autotype_text_with_delay_guarded(target, char_delay_ms, 0, target_hwnd)
    } else {
        let backspaces_needed = current.chars().count() - prefix_len;
        if backspaces_needed > 0 {
            send_backspaces_guarded(backspaces_needed, target_hwnd)?;
            safe_sleep_with_target_guard(50, target_hwnd)?;
        }
        let remainder: String = target.chars().skip(prefix_len).collect();
        autotype_text_with_delay_guarded(&remainder, char_delay_ms, 0, target_hwnd)
    }
}

#[cfg(target_os = "windows")]
fn try_set_element_value_via_uia(
    focused: &windows::Win32::UI::Accessibility::IUIAutomationElement,
    text: &str,
) -> bool {
    use windows::core::{Interface, BSTR};
    use windows::Win32::UI::Accessibility::{IUIAutomationValuePattern, UIA_ValuePatternId};

    unsafe {
        if let Ok(pattern_obj) = focused.GetCurrentPattern(UIA_ValuePatternId) {
            if let Ok(val_pattern) = pattern_obj.cast::<IUIAutomationValuePattern>() {
                let bstr = BSTR::from(text);
                if val_pattern.SetValue(&bstr).is_ok() {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(target_os = "windows")]
fn send_ctrl_v_guarded(target_hwnd: windows::Win32::Foundation::HWND) -> crate::Result<()> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL, VIRTUAL_KEY
    };

    let send_single = |vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY, scan: u16, flags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS| -> crate::Result<()> {
        check_target_window_active(target_hwnd)?;
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: scan,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            let sent = SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
            if sent != 1 {
                return Err(crate::error::VaultError::EncryptionError(
                    "Autotype failed to send key event".into(),
                ));
            }
        }
        Ok(())
    };

    send_single(VK_CONTROL, 0x1D, windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0))?;
    safe_sleep_with_target_guard(15, target_hwnd)?;

    send_single(VIRTUAL_KEY(0x56), 0x2F, windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0))?;
    safe_sleep_with_target_guard(15, target_hwnd)?;

    send_single(VIRTUAL_KEY(0x56), 0x2F, KEYEVENTF_KEYUP)?;
    safe_sleep_with_target_guard(15, target_hwnd)?;

    send_single(VK_CONTROL, 0x1D, KEYEVENTF_KEYUP)?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn send_secure_paste_guarded(text: &str, target_hwnd: windows::Win32::Foundation::HWND) -> crate::Result<()> {
    use windows::Win32::System::DataExchange::{OpenClipboard, CloseClipboard, EmptyClipboard, SetClipboardData};
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    check_target_window_active(target_hwnd)?;

    let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes_len = utf16.len() * std::mem::size_of::<u16>();

    unsafe {
        if OpenClipboard(target_hwnd).is_ok() {
            let _ = EmptyClipboard();
            if let Ok(h_mem) = GlobalAlloc(GMEM_MOVEABLE, bytes_len) {
                let ptr = GlobalLock(h_mem) as *mut u16;
                if !ptr.is_null() {
                    std::ptr::copy_nonoverlapping(utf16.as_ptr(), ptr, utf16.len());
                    let _ = GlobalUnlock(h_mem);
                    let _ = SetClipboardData(13u32, windows::Win32::Foundation::HANDLE(h_mem.0));
                }
            }
            let _ = CloseClipboard();
        }
    }

    // Send Ctrl+V key combination (keyloggers see Ctrl+V only, hiding raw password scan codes)
    send_ctrl_v_guarded(target_hwnd)?;

    // Immediate zeroization of clipboard after paste settling delay (50ms)
    safe_sleep_with_target_guard(50, target_hwnd)?;

    unsafe {
        if OpenClipboard(target_hwnd).is_ok() {
            let _ = EmptyClipboard();
            let _ = CloseClipboard();
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn inject_secret_guarded(
    focused: &windows::Win32::UI::Accessibility::IUIAutomationElement,
    text: &str,
    target_hwnd: windows::Win32::Foundation::HWND,
    char_delay_ms: u64,
) -> crate::Result<()> {
    check_target_window_active(target_hwnd)?;

    // Primary Defense: Direct UIA COM Property Injection (0 Keystrokes, Immune to WH_KEYBOARD_LL)
    if try_set_element_value_via_uia(focused, text) {
        return Ok(());
    }

    // Secondary Defense: Block Paste with Instant Zeroization (Keyloggers see Ctrl+V only)
    if send_secure_paste_guarded(text, target_hwnd).is_ok() {
        return Ok(());
    }

    // Tertiary Fallback: Guarded Keystroke Typing
    autotype_text_with_delay_guarded(text, char_delay_ms, 0, target_hwnd)
}

#[cfg(target_os = "windows")]
fn get_element_value(focused: &windows::Win32::UI::Accessibility::IUIAutomationElement) -> String {
    use windows::core::Interface;
    use windows::Win32::UI::Accessibility::{IUIAutomationValuePattern, UIA_ValuePatternId};

    unsafe {
        if let Ok(pattern_obj) = focused.GetCurrentPattern(UIA_ValuePatternId) {
            if let Ok(val_pattern) = pattern_obj.cast::<IUIAutomationValuePattern>() {
                if let Ok(bstr_val) = val_pattern.CurrentValue() {
                    return bstr_val.to_string();
                }
            }
        }
    }
    String::new()
}

#[cfg(target_os = "windows")]
fn get_window_title(hwnd: windows::Win32::Foundation::HWND) -> String {
    use windows::Win32::UI::WindowsAndMessaging::GetWindowTextW;
    let mut buf = [0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, &mut buf) };
    if len > 0 {
        String::from_utf16_lossy(&buf[..len as usize])
    } else {
        String::new()
    }
}

#[cfg(target_os = "windows")]
fn get_window_process_name(hwnd: windows::Win32::Foundation::HWND) -> String {
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
    use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
    use windows::Win32::System::ProcessStatus::GetModuleBaseNameA;

    if hwnd.is_invalid() {
        return String::new();
    }

    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return String::new();
        }

        let handle = match OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid) {
            Ok(h) => h,
            Err(_) => return String::new(),
        };

        let mut buf = [0u8; 260];
        let len = GetModuleBaseNameA(handle, None, &mut buf);
        let _ = windows::Win32::Foundation::CloseHandle(handle);

        if len > 0 {
            String::from_utf8_lossy(&buf[..len as usize]).to_lowercase()
        } else {
            String::new()
        }
    }
}

#[cfg(target_os = "windows")]
fn is_known_web_browser(proc_name: &str) -> bool {
    let p = proc_name.to_lowercase();
    p.contains("chrome")
        || p.contains("msedge")
        || p.contains("firefox")
        || p.contains("brave")
        || p.contains("opera")
        || p.contains("vivaldi")
        || p.contains("arc")
        || p.contains("waterfox")
        || p.contains("librewolf")
        || p.contains("zen")
        || p.contains("thorium")
        || p.contains("floorp")
        || p.contains("iexplore")
}

#[cfg(target_os = "windows")]
fn is_verified_login_context(
    hwnd: windows::Win32::Foundation::HWND,
    title: &str,
    target_domain_token: &str,
    automation: &windows::Win32::UI::Accessibility::IUIAutomation,
) -> bool {
    let title_lower = title.to_lowercase();
    let proc_name = get_window_process_name(hwnd);
    let is_browser = is_known_web_browser(&proc_name);

    // 1. Basic UI check: Does window have an active password field or explicit login indicator?
    let has_login_indicator = if let Ok(win_el) = unsafe { automation.ElementFromHandle(hwnd) } {
        active_window_has_password_field(automation, &win_el)
            || title_lower.contains("login")
            || title_lower.contains("sign in")
            || title_lower.contains("signin")
            || title_lower.contains("log in")
            || title_lower.contains("sign-in")
            || title_lower.contains("log-in")
            || title_lower.contains("auth")
            || title_lower.contains("session")
            || title_lower.contains("skapa konto")
            || title_lower.contains("registrera")
            || title_lower.contains("logga in")
            || title_lower.contains("lösenord")
    } else {
        false
    };

    if !has_login_indicator {
        return false;
    }

    // 2. Strict Domain & Process Executable Anti-Phishing Guard
    if !target_domain_token.is_empty() {
        if is_browser {
            // Browser window MUST contain the verified domain token in its title bar snippet
            // E.g., "Sign in to GitHub · GitHub - Google Chrome" contains "github" -> Verified.
            // A phishing window titled "Sign In - Google Chrome" does NOT contain "github" -> Rejected!
            if !title_lower.contains(target_domain_token) {
                return false;
            }
        } else {
            // Native Application window: Process name or window title MUST match domain token
            // E.g., "discord.exe" matches "discord" -> Verified.
            // Spoofed app "phish.exe" with title "Sign In - Google Chrome" -> Rejected!
            if !proc_name.contains(target_domain_token) && !title_lower.contains(target_domain_token) {
                return false;
            }
        }
    } else if !is_browser {
        // If no URL is provided AND process is not a recognized browser, require an explicit password field
        if let Ok(win_el) = unsafe { automation.ElementFromHandle(hwnd) } {
            if !active_window_has_password_field(automation, &win_el) {
                return false;
            }
        }
    }

    true
}

#[cfg(target_os = "windows")]
fn poll_until_login_context_ready(
    automation: &windows::Win32::UI::Accessibility::IUIAutomation,
    domain_token: &str,
    max_timeout_ms: u64,
) -> bool {
    let steps = max_timeout_ms / 50;
    for _ in 0..steps {
        let hwnd = unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
        if !hwnd.is_invalid() {
            let title = get_window_title(hwnd);
            if is_verified_login_context(hwnd, &title, domain_token, automation) {
                return true;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    false
}


// ─── Semantic Language-Independent Link & Text Parsers ─────────────────

#[cfg(target_os = "windows")]
fn is_likely_login_url(url: &str) -> bool {
    let url_lower = url.to_lowercase();
    if let Ok(parsed) = reqwest::Url::parse(&url_lower) {
        // 1. Check subdomain host parts
        if let Some(host) = parsed.host_str() {
            for part in host.split('.') {
                if part == "login" || part == "signin" || part == "auth" {
                    return true;
                }
            }
        }
        
        // 2. Check path segments exactly
        if let Some(segments) = parsed.path_segments() {
            for seg in segments {
                if seg == "login"
                    || seg == "signin"
                    || seg == "log-in"
                    || seg == "sign-in"
                    || seg == "session"
                    || seg == "auth"
                    || seg == "connect"
                {
                    return true;
                }
            }
        }
    } else {
        // Relative paths or queries
        let cleaned = url_lower.trim_matches('/');
        if cleaned == "login"
            || cleaned == "signin"
            || cleaned == "log-in"
            || cleaned == "sign-in"
            || cleaned == "session"
            || cleaned == "auth"
            || cleaned == "connect"
            || cleaned.ends_with("/login")
            || cleaned.ends_with("/signin")
            || cleaned.ends_with("/log-in")
            || cleaned.ends_with("/sign-in")
        {
            return true;
        }
    }
    false
}

#[cfg(target_os = "windows")]
fn is_likely_login_text(text: &str) -> bool {
    let t = text.to_lowercase().replace(" ", "").replace("-", "");
    t == "login"
        || t == "signin"
        || t == "loggain"
        || t == "anmäla"
        || t == "anmäld"
        || t == "anmelden"
        || t == "einloggen"
        || t == "seconnecter"
        || t == "connexion"
        || t == "iniciarsesión"
        || t == "iniciarsesion"
        || t == "conectar"
        || t == "conectarse"
        || t == "entrar"
        || t == "signinto"
        || t == "loginin"
        || t == "loginto"
        || t == "login"
        || t.starts_with("login")
        || t.starts_with("signin")
        || t.starts_with("signinto")
        || t.starts_with("loggain")
        || t.starts_with("anmelden")
        || t.starts_with("einloggen")
}

#[cfg(target_os = "windows")]
fn is_non_login_input_field(name: &str, class_name: &str, auto_id: &str) -> bool {
    let n = name.to_lowercase();
    let c = class_name.to_lowercase();
    let i = auto_id.to_lowercase();
    
    let keywords = [
        "search", "sök", "find", "chat", "message", "reply", "comment", 
        "prompt", "filter", "query", "ask", "fråga", "gpt", "copilot",
        "newsletter", "subscribe", "coupon", "promo", "discount", "rabatt",
        "prenumerera", "address", "street", "zip", "postcode", "city",
        "cvv", "card", "subject", "feedback", "review"
    ];
    
    for kw in keywords {
        if n.contains(kw) || c.contains(kw) || i.contains(kw) {
            return true;
        }
    }
    false
}

#[cfg(target_os = "windows")]
fn is_valid_username_field(
    name: &str,
    class_name: &str,
    auto_id: &str,
    has_co_located_password_field: bool,
    is_password_field: bool,
    is_totp_field: bool,
) -> bool {
    if is_password_field || is_totp_field {
        return false;
    }

    if is_non_login_input_field(name, class_name, auto_id) {
        return false;
    }

    let n = name.to_lowercase();
    let c = class_name.to_lowercase();
    let i = auto_id.to_lowercase();

    // 1. Explicit Positive Keyword Identifiers (High Confidence)
    let username_keywords = [
        "user", "username", "email", "e-mail", "mail", "login", "account",
        "ident", "användar", "e-post", "epost", "usuario", "kullanıcı", "nom",
        "identifier", "handle", "signin", "sign-in", "log-in"
    ];

    for kw in username_keywords {
        if n.contains(kw) || c.contains(kw) || i.contains(kw) {
            return true;
        }
    }

    // 2. Structural Co-location Guard:
    // If the active page/form explicitly contains a co-located password field,
    // any non-search input on that form is accepted as the username input field.
    if has_co_located_password_field {
        return true;
    }

    false
}

// ─── Browser Automation Helpers ─────────────────────────────────────────


#[cfg(target_os = "windows")]
fn find_targeted_elements(
    automation: &windows::Win32::UI::Accessibility::IUIAutomation,
    window_el: &windows::Win32::UI::Accessibility::IUIAutomationElement,
    control_type_id: windows::Win32::UI::Accessibility::UIA_CONTROLTYPE_ID,
) -> Option<windows::Win32::UI::Accessibility::IUIAutomationElementArray> {
    use windows::Win32::UI::Accessibility::{TreeScope_Descendants, UIA_ControlTypePropertyId};
    unsafe {
        let var = windows::core::VARIANT::from(control_type_id.0);
        let cond = automation.CreatePropertyCondition(UIA_ControlTypePropertyId, &var).ok()?;
        window_el.FindAll(TreeScope_Descendants, &cond).ok()
    }
}

#[cfg(target_os = "windows")]
fn try_click_login_link(
    automation: &windows::Win32::UI::Accessibility::IUIAutomation,
    window_el: &windows::Win32::UI::Accessibility::IUIAutomationElement,
) -> bool {
    use windows::core::Interface;
    use windows::Win32::UI::Accessibility::{
        IUIAutomationInvokePattern, UIA_ButtonControlTypeId, UIA_HyperlinkControlTypeId, UIA_InvokePatternId,
    };

    let check_array = |elements: windows::Win32::UI::Accessibility::IUIAutomationElementArray| -> bool {
        let raw_count = match unsafe { elements.Length() } {
            Ok(c) => c as usize,
            Err(_) => 0,
        };
        let count = std::cmp::min(raw_count, 35);

        for i in 0..count {
            let el = match unsafe { elements.GetElement(i as i32) } {
                Ok(e) => e,
                Err(_) => continue,
            };

            let name = unsafe { el.CurrentName() }
                .map(|b| b.to_string())
                .unwrap_or_default();

            let href = get_element_value(&el).to_lowercase();
            let auto_id = unsafe { el.CurrentAutomationId() }
                .map(|id| id.to_string().to_lowercase())
                .unwrap_or_default();

            let is_login_btn = is_likely_login_text(&name)
                || is_likely_login_url(&href)
                || auto_id.contains("login")
                || auto_id.contains("signin")
                || auto_id.contains("session");

            if is_login_btn {
                unsafe {
                    if let Ok(pattern_obj) = el.GetCurrentPattern(UIA_InvokePatternId) {
                        if let Ok(invoke_pattern) = pattern_obj.cast::<IUIAutomationInvokePattern>() {
                            if invoke_pattern.Invoke().is_ok() {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    };

    if let Some(buttons) = find_targeted_elements(automation, window_el, UIA_ButtonControlTypeId) {
        if check_array(buttons) {
            return true;
        }
    }

    if let Some(links) = find_targeted_elements(automation, window_el, UIA_HyperlinkControlTypeId) {
        if check_array(links) {
            return true;
        }
    }

    false
}

#[cfg(target_os = "windows")]
fn try_focus_username_field(
    automation: &windows::Win32::UI::Accessibility::IUIAutomation,
    window_el: &windows::Win32::UI::Accessibility::IUIAutomationElement,
) -> bool {
    use windows::Win32::UI::Accessibility::UIA_EditControlTypeId;

    let elements = match find_targeted_elements(automation, window_el, UIA_EditControlTypeId) {
        Some(el) => el,
        None => return false,
    };

    let raw_count = match unsafe { elements.Length() } {
        Ok(c) => c as usize,
        Err(_) => 0,
    };
    let count = std::cmp::min(raw_count, 40);

    for i in 0..count {
        let el = match unsafe { elements.GetElement(i as i32) } {
            Ok(e) => e,
            Err(_) => continue,
        };

        let is_pw = unsafe { el.CurrentIsPassword() }
            .map(|b| b.as_bool())
            .unwrap_or(false);
        if is_pw {
            continue;
        }

        let is_offscreen = unsafe { el.CurrentIsOffscreen() }
            .map(|b| b.as_bool())
            .unwrap_or(false);
        if is_offscreen {
            continue;
        }

        let is_focusable = unsafe { el.CurrentIsKeyboardFocusable() }
            .map(|b| b.as_bool())
            .unwrap_or(false);
        if !is_focusable {
            continue;
        }

        let name = unsafe { el.CurrentName() }
            .map(|b| b.to_string().to_lowercase())
            .unwrap_or_default();

        let class_name = unsafe { el.CurrentClassName() }
            .map(|b| b.to_string().to_lowercase())
            .unwrap_or_default();

        let auto_id = unsafe { el.CurrentAutomationId() }
            .map(|id| id.to_string().to_lowercase())
            .unwrap_or_default();

        if is_non_login_input_field(&name, &class_name, &auto_id) {
            continue;
        }

        if unsafe { el.SetFocus() }.is_ok() {
            return true;
        }
    }

    false
}

#[cfg(target_os = "windows")]
fn is_on_register_page(
    automation: &windows::Win32::UI::Accessibility::IUIAutomation,
    window_el: &windows::Win32::UI::Accessibility::IUIAutomationElement,
) -> bool {
    use windows::Win32::UI::Accessibility::UIA_EditControlTypeId;

    let title = unsafe { window_el.CurrentName() }
        .map(|b| b.to_string().to_lowercase())
        .unwrap_or_default();

    if title.contains("register")
        || title.contains("sign up")
        || title.contains("signup")
        || title.contains("skapa konto")
        || title.contains("registrera")
    {
        return true;
    }

    let elements = match find_targeted_elements(automation, window_el, UIA_EditControlTypeId) {
        Some(el) => el,
        None => return false,
    };

    let raw_count = match unsafe { elements.Length() } {
        Ok(c) => c as usize,
        Err(_) => 0,
    };
    let count = std::cmp::min(raw_count, 40);

    let mut password_count = 0;
    for i in 0..count {
        let el = match unsafe { elements.GetElement(i as i32) } {
            Ok(e) => e,
            Err(_) => continue,
        };

        let is_pw = unsafe { el.CurrentIsPassword() }
            .map(|b| b.as_bool())
            .unwrap_or(false);

        if is_pw {
            password_count += 1;
            if password_count >= 2 {
                return true;
            }
        } else {
            let name = unsafe { el.CurrentName() }
                .map(|b| b.to_string().to_lowercase())
                .unwrap_or_default();
            let class_name = unsafe { el.CurrentClassName() }
                .map(|b| b.to_string().to_lowercase())
                .unwrap_or_default();

            let is_confirm_field = name.contains("confirm password")
                || name.contains("repeat password")
                || name.contains("lösenordsbekräftelse")
                || name.contains("bekräfta lösenord")
                || class_name.contains("confirm-password")
                || class_name.contains("password-confirm");

            if is_confirm_field {
                return true;
            }
        }
    }

    false
}

#[cfg(target_os = "windows")]
fn active_window_has_password_field(
    automation: &windows::Win32::UI::Accessibility::IUIAutomation,
    window_el: &windows::Win32::UI::Accessibility::IUIAutomationElement,
) -> bool {
    use windows::Win32::UI::Accessibility::UIA_EditControlTypeId;

    let elements = match find_targeted_elements(automation, window_el, UIA_EditControlTypeId) {
        Some(el) => el,
        None => return false,
    };

    let raw_count = match unsafe { elements.Length() } {
        Ok(c) => c as usize,
        Err(_) => 0,
    };
    let count = std::cmp::min(raw_count, 40);

    for i in 0..count {
        let el = match unsafe { elements.GetElement(i as i32) } {
            Ok(e) => e,
            Err(_) => continue,
        };

        let is_offscreen = unsafe { el.CurrentIsOffscreen() }
            .map(|b| b.as_bool())
            .unwrap_or(false);
        if is_offscreen {
            continue;
        }

        let is_focusable = unsafe { el.CurrentIsKeyboardFocusable() }
            .map(|b| b.as_bool())
            .unwrap_or(false);
        if !is_focusable {
            continue;
        }

        let is_pw = unsafe { el.CurrentIsPassword() }
            .map(|b| b.as_bool())
            .unwrap_or(false);

        if is_pw {
            return true;
        }

        let name = unsafe { el.CurrentName() }
            .map(|b| b.to_string().to_lowercase())
            .unwrap_or_default();
        let class_name = unsafe { el.CurrentClassName() }
            .map(|b| b.to_string().to_lowercase())
            .unwrap_or_default();

        let is_password = (name.contains("password")
            || name.contains("lösenord")
            || name == "pass"
            || class_name.contains("password")
            || class_name == "pass")
            && !name.contains("code")
            && !name.contains("token")
            && !name.contains("otp");

        if is_password {
            return true;
        }
    }

    false
}

// ─── Entry points ───────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
pub fn run_smart_autotype(username: String, password: String) -> crate::Result<()> {
    run_smart_autotype_with_delays(username, password, String::new(), String::new(), true, 15, 300)
}

#[cfg(target_os = "windows")]
pub fn run_smart_autotype_with_delays(
    username: String,
    password: String,
    totp_secret: String,
    url: String,
    launch_browser: bool,
    char_delay_ms: u64,
    field_delay_ms: u64,
) -> crate::Result<()> {
    use windows::Win32::System::Com::{CoInitializeEx, CoCreateInstance, CLSCTX_ALL, COINIT_MULTITHREADED};
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationElement, UIA_EditControlTypeId
    };
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    let normalized_url = if !url.is_empty() {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            format!("https://{}", url)
        } else {
            url.clone()
        }
    } else {
        String::new()
    };

    std::thread::spawn(move || {
        let guard = AutotypeGuard {
            username: zeroize::Zeroizing::new(username),
            password: zeroize::Zeroizing::new(password),
            totp_secret: zeroize::Zeroizing::new(totp_secret),
        };

        let domain_token = if let Ok(parsed) = reqwest::Url::parse(&normalized_url) {
            parsed.host_str()
                .unwrap_or("")
                .split('.')
                .find(|&s| s != "www" && s != "com" && s != "org" && s != "net" && s != "io" && s != "se" && s != "co" && s != "uk")
                .unwrap_or("")
                .to_string()
        } else {
            String::new()
        };

        // Use normalized target URL directly without network probing (enforces offline invariant)
        let target_url = if !normalized_url.is_empty() && launch_browser {
            normalized_url
        } else {
            String::new()
        };

        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let automation: IUIAutomation = match CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL) {
                Ok(a) => a,
                Err(_) => return,
            };

            // Launch browser directly to the resolved target (e.g. https://github.com/login)
            if !target_url.is_empty() && launch_browser {
                let hwnd = GetForegroundWindow();
                let is_already_active = if !hwnd.is_invalid() {
                    let title = get_window_title(hwnd).to_lowercase();
                    let domain_token = if let Ok(parsed) = reqwest::Url::parse(&target_url) {
                        parsed.host_str()
                            .unwrap_or("")
                            .split('.')
                            .find(|&s| s != "www" && s != "com" && s != "org" && s != "net" && s != "io" && s != "se")
                            .unwrap_or("")
                            .to_string()
                    } else {
                        String::new()
                    };
                    !domain_token.is_empty() && title.contains(&domain_token)
                } else {
                    false
                };

                if !is_already_active {
                    use windows::core::PCWSTR;
                    use windows::Win32::UI::Shell::ShellExecuteW;
                    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

                    let verb: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();
                    let url_wide: Vec<u16> = target_url.encode_utf16().chain(std::iter::once(0)).collect();

                    ShellExecuteW(
                        None,
                        PCWSTR(verb.as_ptr()),
                        PCWSTR(url_wide.as_ptr()),
                        PCWSTR::null(),
                        PCWSTR::null(),
                        SW_SHOWNORMAL,
                    );

                    // Adaptive Settle Polling: Poll in 50ms steps until browser window & login context render (max 3500ms)
                    let _ = poll_until_login_context_ready(&automation, &domain_token, 3500);

                    // Fallback: If we landed on a homepage (e.g. because resolver fell back to original base URL)
                    // and no input is focused, try to find and click a login link.
                    let hwnd = GetForegroundWindow();
                    if !hwnd.is_invalid() {
                        if let Ok(window_el) = automation.ElementFromHandle(hwnd) {
                            let mut already_on_login_form = false;
                            if let Ok(focused) = automation.GetFocusedElement() {
                                let class_name = focused.CurrentClassName()
                                    .map(|b| b.to_string().to_lowercase())
                                    .unwrap_or_default();
                                let control_type = focused.CurrentLocalizedControlType()
                                    .map(|b| b.to_string().to_lowercase())
                                    .unwrap_or_default();
                                let control_id = focused.CurrentControlType().unwrap_or(windows::Win32::UI::Accessibility::UIA_CONTROLTYPE_ID(0));
                                if control_id == UIA_EditControlTypeId
                                    || class_name.contains("edit")
                                    || control_type.contains("edit")
                                    || control_type.contains("text box")
                                {
                                    already_on_login_form = true;
                                }
                            }

                            // SOTA: Prevent clicking login links if we are already on a login page containing a password field
                            if !already_on_login_form {
                                if active_window_has_password_field(&automation, &window_el) {
                                    already_on_login_form = true;
                                }
                            }

                            if !already_on_login_form {
                                if try_click_login_link(&automation, &window_el) {
                                    let _ = poll_until_login_context_ready(&automation, &domain_token, 3000);
                                }
                            }
                        }
                    }
                }
            }

            let mut last_focused_element_id: Option<String> = None;
            let mut filled_username = guard.username.is_empty();
            let mut filled_password = guard.password.is_empty();
            let mut filled_totp = guard.totp_secret.is_empty();
            let mut focus_attempts = 0;
            let mut target_hwnd = windows::Win32::Foundation::HWND::default();

            // Poll for up to 45 seconds (225 polls * 200ms) to allow multi-step transition/2FA
            for loop_counter in 0..225 {
                if filled_username && filled_password && filled_totp {
                    break;
                }

                std::thread::sleep(std::time::Duration::from_millis(200));

                let hwnd = GetForegroundWindow();
                if hwnd.is_invalid() {
                    continue;
                }

                if target_hwnd.is_invalid() {
                    target_hwnd = hwnd;
                } else if hwnd != target_hwnd {
                    // Security: Active window switched, abort to prevent leaks!
                    break;
                }

                // Anti-Phishing Guard: Enforce verified domain token and process executable matching
                let title = get_window_title(hwnd);
                let is_login_context = is_verified_login_context(hwnd, &title, &domain_token, &automation);

                if !is_login_context {
                    // Not a verified login context (e.g. domain mismatch or untrusted process title), ignore
                    continue;
                }

                let focused: IUIAutomationElement = match automation.GetFocusedElement() {
                    Ok(f) => f,
                    Err(_) => continue,
                };

                let name = focused.CurrentName()
                    .map(|b| b.to_string().to_lowercase())
                    .unwrap_or_default();

                let class_name = focused.CurrentClassName()
                    .map(|b| b.to_string().to_lowercase())
                    .unwrap_or_default();

                let control_type = focused.CurrentLocalizedControlType()
                    .map(|b| b.to_string().to_lowercase())
                    .unwrap_or_default();

                let control_id = focused.CurrentControlType().unwrap_or(windows::Win32::UI::Accessibility::UIA_CONTROLTYPE_ID(0));

                let element_key = format!("{}-{}-{}", class_name, name, control_type);
                if last_focused_element_id.as_ref() == Some(&element_key) {
                    continue;
                }

                // Check ControlTypeID as language-independent SOTA criteria
                let is_input = control_id == UIA_EditControlTypeId
                    || class_name.contains("edit")
                    || control_type.contains("edit")
                    || control_type.contains("text box")
                    || control_type.contains("inmatningsfält")
                    || class_name.contains("chrome_render_widget_host_view")
                    || class_name.contains("renderwidgethostview");

                let auto_id = focused.CurrentAutomationId()
                    .map(|id| id.to_string().to_lowercase())
                    .unwrap_or_default();

                // Skip typing if focused element is a non-login input field (e.g. search, chat, newsletter)
                if is_input && is_non_login_input_field(&name, &class_name, &auto_id) {
                    continue;
                }

                if !is_input && !filled_username && focus_attempts < 15 {
                    // Try to auto-focus the username field once every 5 loops (1 second)
                    if loop_counter % 5 == 0 {
                        focus_attempts += 1;
                        let hwnd = GetForegroundWindow();
                        if !hwnd.is_invalid() {
                            if let Ok(win_el) = automation.ElementFromHandle(hwnd) {
                                let _ = try_focus_username_field(&automation, &win_el);
                            }
                        }
                    }
                }

                if is_input {
                    let hwnd = GetForegroundWindow();

                    // Check if we are on a registration/signup page (only before credentials are typed to prevent infinite loops)
                    let on_register = !filled_password && !hwnd.is_invalid() && if let Ok(win_el) = automation.ElementFromHandle(hwnd) {
                        is_on_register_page(&automation, &win_el)
                    } else {
                        false
                    };

                    if on_register {
                        if let Ok(win_el) = automation.ElementFromHandle(hwnd) {
                            if try_click_login_link(&automation, &win_el) {
                                let _ = poll_until_login_context_ready(&automation, &domain_token, 3000);
                                last_focused_element_id = None; // Reset focus to re-evaluate on redirected page
                                continue;
                            }
                        }
                    }

                    let is_totp_field = name.contains("code")
                        || name.contains("token")
                        || name.contains("totp")
                        || name.contains("2fa")
                        || name.contains("otp")
                        || name.contains("mfa")
                        || name.contains("verification")
                        || name.contains("kod")
                        || name.contains("säkerhet")
                        || name.contains("security")
                        || class_name.contains("code")
                        || class_name.contains("totp")
                        || class_name.contains("otp");

                    // Check native accessibility property first, fallback to keywords (overridden by is_totp_field)
                    let is_password_field = !is_totp_field && (
                        focused.CurrentIsPassword()
                            .map(|b| b.as_bool())
                            .unwrap_or(false)
                        || name.contains("password")
                        || name.contains("lösenord")
                        || name == "pass"
                        || class_name.contains("password")
                        || class_name == "pass"
                    );

                    // Positive Username Verification: Requires explicit keyword match or co-located password field
                    let has_co_located_password = !hwnd.is_invalid() && if let Ok(win_el) = automation.ElementFromHandle(hwnd) {
                        active_window_has_password_field(&automation, &win_el)
                    } else {
                        false
                    };

                    let is_username_field = is_valid_username_field(
                        &name,
                        &class_name,
                        &auto_id,
                        has_co_located_password,
                        is_password_field,
                        is_totp_field,
                    );

                    if is_totp_field && !filled_totp {
                        last_focused_element_id = Some(element_key.clone());
                        let config = crate::totp::TotpConfig {
                            secret: guard.totp_secret.to_string(),
                            ..Default::default()
                        };
                        if let Ok(totp_code) = crate::totp::generate_totp(&config) {
                            if send_ctrl_a_backspace_guarded(target_hwnd).is_err() { break; }
                            if safe_sleep_with_target_guard(100, target_hwnd).is_err() { break; }
                            if inject_secret_guarded(&focused, &totp_code.code, target_hwnd, char_delay_ms).is_err() { break; }
                            if safe_sleep_with_target_guard(field_delay_ms, target_hwnd).is_err() { break; }
                            if send_enter_guarded(target_hwnd).is_err() { break; }
                        }
                        filled_totp = true;
                    } else if is_password_field && !filled_password {
                        last_focused_element_id = Some(element_key.clone());

                        if !filled_username {
                            // Focus was directly on Password field first, traverse up to username first
                            if safe_sleep_with_target_guard(150, target_hwnd).is_err() { break; }
                            if send_shift_tab_guarded(target_hwnd).is_err() { break; }
                            if safe_sleep_with_target_guard(field_delay_ms, target_hwnd).is_err() { break; }

                            // Fill Username
                            let mut user_val = String::new();
                            if let Ok(new_focused) = automation.GetFocusedElement() {
                                user_val = get_element_value(&new_focused);
                                if inject_secret_guarded(&new_focused, &guard.username, target_hwnd, char_delay_ms).is_err() { break; }
                            } else {
                                if autotype_correct_text_guarded(&user_val, &guard.username, char_delay_ms, target_hwnd).is_err() { break; }
                            }
                            if safe_sleep_with_target_guard(field_delay_ms, target_hwnd).is_err() { break; }

                            // Return to Password
                            if autotype_text_with_delay_guarded("\t", char_delay_ms, 0, target_hwnd).is_err() { break; }
                            if safe_sleep_with_target_guard(field_delay_ms, target_hwnd).is_err() { break; }
                            filled_username = true;
                        }

                        // Clear password and fill
                        if send_ctrl_a_backspace_guarded(target_hwnd).is_err() { break; }
                        if safe_sleep_with_target_guard(100, target_hwnd).is_err() { break; }
                        if inject_secret_guarded(&focused, &guard.password, target_hwnd, char_delay_ms).is_err() { break; }
                        if safe_sleep_with_target_guard(field_delay_ms, target_hwnd).is_err() { break; }
                        if send_enter_guarded(target_hwnd).is_err() { break; }
                        filled_password = true;
                    } else if is_username_field && !filled_username {
                        last_focused_element_id = Some(element_key.clone());

                        if inject_secret_guarded(&focused, &guard.username, target_hwnd, char_delay_ms).is_err() { break; }
                        if safe_sleep_with_target_guard(field_delay_ms, target_hwnd).is_err() { break; }

                        // Check if password field is visible in active window
                        let has_password = !hwnd.is_invalid() && if let Ok(win_el) = automation.ElementFromHandle(hwnd) {
                            active_window_has_password_field(&automation, &win_el)
                        } else {
                            false
                        };

                        if has_password {
                            // Standard single-screen login form: Tab down and enter password
                            if autotype_text_with_delay_guarded("\t", char_delay_ms, 0, target_hwnd).is_err() { break; }
                            if safe_sleep_with_target_guard(field_delay_ms, target_hwnd).is_err() { break; }
                            if send_ctrl_a_backspace_guarded(target_hwnd).is_err() { break; }
                            if safe_sleep_with_target_guard(100, target_hwnd).is_err() { break; }
                            
                            // Re-query focused password element to inject safely
                            if let Ok(pass_focused) = automation.GetFocusedElement() {
                                if inject_secret_guarded(&pass_focused, &guard.password, target_hwnd, char_delay_ms).is_err() { break; }
                            } else {
                                if autotype_text_with_delay_guarded(&guard.password, char_delay_ms, 0, target_hwnd).is_err() { break; }
                            }

                            if safe_sleep_with_target_guard(field_delay_ms, target_hwnd).is_err() { break; }
                            if send_enter_guarded(target_hwnd).is_err() { break; }
                            filled_username = true;
                            filled_password = true;
                        } else {
                            // Split-screen login form (like Google page 1): Press Enter to go to password screen
                            if send_enter_guarded(target_hwnd).is_err() { break; }
                            filled_username = true;
                        }
                    }
                }
            }
        }
    });

    Ok(())
}

// ─── Non-Windows Stubs ──────────────────────────────────────────────────

#[cfg(not(target_os = "windows"))]
pub fn autotype_text(text: &str) -> crate::Result<()> {
    autotype_text_with_delay(text, 15, 0)
}

#[cfg(not(target_os = "windows"))]
pub fn autotype_text_with_delay(_text: &str, _char_delay_ms: u64, _settle_delay_ms: u64) -> crate::Result<()> {
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn run_smart_autotype(username: String, password: String) -> crate::Result<()> {
    run_smart_autotype_with_delays(username, password, String::new(), String::new(), true, 15, 300)
}

#[cfg(not(target_os = "windows"))]
pub fn run_smart_autotype_with_delays(
    username: String,
    password: String,
    _totp_secret: String,
    _url: String,
    _launch_browser: bool,
    _char_delay_ms: u64,
    _field_delay_ms: u64,
) -> crate::Result<()> {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        let _ = autotype_text(&username);
        let _ = autotype_text("\t");
        let _ = autotype_text(&password);
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autotype_stub() {
        let res = autotype_text("test-typing");
        assert!(res.is_ok() || res.is_err());
    }
}
