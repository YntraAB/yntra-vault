//! Windows UI Automation and Win32 SendInput autotype driver implementation.

use super::{AutotypeDriver, AutotypeGuard};
mod native_login;
pub(crate) use native_login::run_native_google_login;
#[cfg(test)]
pub(crate) use native_login::observe_test_window;
use windows::core::{Interface, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};
use windows::Win32::System::ProcessStatus::GetModuleBaseNameA;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationElementArray,
    IUIAutomationInvokePattern, IUIAutomationValuePattern, TreeScope_Descendants,
    UIA_ButtonControlTypeId, UIA_ControlTypePropertyId, UIA_EditControlTypeId,
    UIA_HyperlinkControlTypeId, UIA_InvokePatternId, UIA_ValuePatternId, UIA_CONTROLTYPE_ID,
    UIA_DocumentControlTypeId, UIA_ToolBarControlTypeId, UIA_WindowControlTypeId,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, VIRTUAL_KEY, VK_BACK, VK_CONTROL, VK_SHIFT, VK_TAB,
    GetKeyboardLayout, GetKeyState, GetAsyncKeyState, VkKeyScanExW, MapVirtualKeyExW, MAPVK_VK_TO_VSC_EX,
    KEYEVENTF_SCANCODE, KEYEVENTF_EXTENDEDKEY, VK_MENU, VK_CAPITAL,
    ToUnicodeEx, HKL, VK_LCONTROL, VK_LSHIFT, VK_RMENU, VK_LWIN, VK_RWIN, MAPVK_VSC_TO_VK_EX,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId, SW_SHOWNORMAL,
};

fn is_target_window_active(target_hwnd: HWND) -> bool {
    if target_hwnd.is_invalid() {
        return true;
    }
    unsafe {
        let current = GetForegroundWindow();
        current == target_hwnd
    }
}

fn check_target_window_active(target_hwnd: HWND) -> crate::Result<()> {
    if !is_target_window_active(target_hwnd) {
        return Err(crate::error::VaultError::AutoTypeError(
            "Target window lost active focus during autotype execution".into(),
        ));
    }
    Ok(())
}

fn safe_sleep_with_target_guard(duration_ms: u64, target_hwnd: HWND) -> crate::Result<()> {
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

const KEY_HOLD_MS: u64 = 8;

/// Keep key-down and key-up as separate OS events with a nonzero hold interval.
/// Releases are attempted even when focus is lost or only part of a batch succeeds.
fn send_character_chord(inputs: &[INPUT], target_hwnd: HWND) -> crate::Result<()> {
    check_target_window_active(target_hwnd)?;
    let (downs, ups) = inputs.split_at(inputs.len() / 2);
    let sent = unsafe { SendInput(downs, std::mem::size_of::<INPUT>() as i32) };
    let held = if sent == downs.len() as u32 {
        safe_sleep_with_target_guard(KEY_HOLD_MS, target_hwnd)
    } else {
        Err(crate::error::VaultError::AutoTypeError("Autotype input was interrupted".into()))
    };
    let released = unsafe { SendInput(ups, std::mem::size_of::<INPUT>() as i32) };
    if released != ups.len() as u32 {
        unsafe { SendInput(ups, std::mem::size_of::<INPUT>() as i32); }
        return Err(crate::error::VaultError::AutoTypeError("Could not release autotype keys".into()));
    }
    held
}

fn autotype_text_with_delay_guarded(
    text: &str,
    char_delay_ms: u64,
    settle_delay_ms: u64,
    target_hwnd: HWND,
) -> crate::Result<()> {
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

    let target_hwnd = if target_hwnd.is_invalid() {
        let hwnd = unsafe { GetForegroundWindow() };
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)); }
        if hwnd.is_invalid() || pid == std::process::id() {
            return Err(crate::error::VaultError::AutoTypeError("Focus a target window before typing".into()));
        }
        hwnd
    } else {
        target_hwnd
    };

    for ch in text.encode_utf16() {
        check_target_window_active(target_hwnd)?;
        check_modifier_keys_released()?;

        // Subtle randomized timing variance (+/- 3ms) to prevent synthetic rhythm detection by anti-bot scripts
        let jitter: i64 = if char_delay_ms > 10 {
            use rand::Rng;
            rand::rng().random_range(-3..=3)
        } else {
            0
        };
        let delay = (char_delay_ms as i64 + jitter).max(KEY_HOLD_MS as i64 + 1) as u64;

        if ch != 9 && send_layout_character(ch, target_hwnd)? {
            safe_sleep_with_target_guard(delay.saturating_sub(KEY_HOLD_MS), target_hwnd)?;
            continue;
        }

        let (vk, scan, flags) = if ch == 9 {
            (VK_TAB, 0u16, KEYBD_EVENT_FLAGS(0))
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

        send_character_chord(&inputs, target_hwnd)?;

        // Configurable delay between characters with active window guard check
        safe_sleep_with_target_guard(delay.saturating_sub(KEY_HOLD_MS), target_hwnd)?;
    }

    Ok(())
}

fn scan_input(scan: u16, key_up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT {
            wVk: VIRTUAL_KEY(0),
            wScan: scan & 0xff,
            dwFlags: KEYEVENTF_SCANCODE
                | if scan & 0xff00 == 0xe000 { KEYEVENTF_EXTENDEDKEY } else { KEYBD_EVENT_FLAGS(0) }
                | if key_up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
            time: 0,
            dwExtraInfo: 0,
        } },
    }
}

fn check_modifier_keys_released() -> crate::Result<()> {
    if [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN]
        .iter()
        .any(|key| unsafe { GetAsyncKeyState(key.0 as i32) } < 0)
    {
        return Err(crate::error::VaultError::AutoTypeError(
            "Release modifier keys before autotype".into(),
        ));
    }
    Ok(())
}

/// Build events without injecting them. Only use a layout mapping that Windows
/// confirms produces exactly the requested character; dead keys use Unicode.
fn layout_character_inputs(ch: u16, layout: HKL, caps_lock: bool) -> Option<([INPUT; 8], usize)> {
    if char::from_u32(ch as u32).is_none_or(char::is_control) { return None; }
    let mapped = unsafe { VkKeyScanExW(ch, layout) };
    if mapped == -1 || ((mapped as u16 >> 8) & !7) != 0 {
        return None;
    }
    let translates_exactly = |vk: u16, modifiers: u16| -> Option<u16> {
        // Numpad digits/decimal share scan codes with navigation keys. Avoid
        // relying on Num Lock or changing the user's toggle state.
        if (0x60..=0x6f).contains(&vk) { return None; }
        let scan = unsafe { MapVirtualKeyExW(vk as u32, MAPVK_VK_TO_VSC_EX, layout) } as u16;
        if scan == 0 { return None; }
        let physical_vk = unsafe { MapVirtualKeyExW(scan as u32, MAPVK_VSC_TO_VK_EX, layout) };
        if physical_vk == 0 { return None; }
        let mut state = [0u8; 256];
        state[VK_CAPITAL.0 as usize] = u8::from(caps_lock);
        for (mask, key) in [(1, VK_SHIFT), (2, VK_CONTROL), (4, VK_MENU)] {
            if modifiers & mask != 0 { state[key.0 as usize] = 0x80; }
        }
        let mut output = zeroize::Zeroizing::new([0u16; 8]);
        // Bit 2 prevents changing the thread's dead-key state (Windows 10 1607+).
        let count = unsafe { ToUnicodeEx(physical_vk, scan as u32, &state, &mut output[..], 4, layout) };
        (count == 1 && output[0] == ch).then_some(scan)
    };
    let vk = mapped as u16 & 0xff;
    let proposed = (mapped as u16 >> 8) & 7;
    // Try the OS suggestion, then its Caps Lock variant. Some layouts return a
    // numpad suggestion despite having a regular punctuation key (e.g. Italian .).
    let mapping = [proposed, proposed ^ 1].into_iter()
        .find_map(|modifiers| translates_exactly(vk, modifiers).map(|scan| (scan, modifiers)))
        .or_else(|| (0x30..=0xfe).find_map(|vk| {
            [0, 1, 6, 7].into_iter().find_map(|modifiers|
                translates_exactly(vk, modifiers).map(|scan| (scan, modifiers)))
        }));
    let (scan, modifiers) = mapping?;
    // VkKeyScanEx reports AltGr as Ctrl+Alt. Use the extended RIGHT Alt key,
    // not left Alt, so browsers receive the layout's actual AltGraph chord.
    let alt = if modifiers & 6 == 6 { VK_RMENU } else { VK_MENU };
    let modifier_keys = [(2, VK_LCONTROL), (4, alt), (1, VK_LSHIFT)];
    let mut inputs = [INPUT::default(); 8];
    let mut count = 0;
    for (mask, key) in modifier_keys {
        if modifiers & mask != 0 {
            let code = unsafe { MapVirtualKeyExW(key.0 as u32, MAPVK_VK_TO_VSC_EX, layout) } as u16;
            if code == 0 { return None; }
            inputs[count] = scan_input(code, false);
            count += 1;
        }
    }
    inputs[count] = scan_input(scan, false);
    inputs[count + 1] = scan_input(scan, true);
    count += 2;
    for (mask, key) in modifier_keys.into_iter().rev() {
        if modifiers & mask != 0 {
            let code = unsafe { MapVirtualKeyExW(key.0 as u32, MAPVK_VK_TO_VSC_EX, layout) } as u16;
            inputs[count] = scan_input(code, true);
            count += 1;
        }
    }
    Some((inputs, count))
}

fn send_layout_character(ch: u16, target_hwnd: HWND) -> crate::Result<bool> {
    let layout = unsafe { GetKeyboardLayout(GetWindowThreadProcessId(target_hwnd, None)) };
    let caps_lock = unsafe { GetKeyState(VK_CAPITAL.0 as i32) } & 1 != 0;
    let Some((inputs, count)) = layout_character_inputs(ch, layout, caps_lock) else {
        return Ok(false);
    };
    send_character_chord(&inputs[..count], target_hwnd)?;
    Ok(true)
}

fn inject_identifier_guarded(focused: &IUIAutomationElement, text: &str, target_hwnd: HWND, char_delay_ms: u64) -> crate::Result<()> {
    if is_known_web_browser(&get_window_process_name(target_hwnd)) {
        if text.chars().any(char::is_control) {
            return Err(crate::error::VaultError::AutoTypeError("Username contains control characters".into()));
        }
        check_identifier_focus(focused, target_hwnd)?;
        let pattern: IUIAutomationValuePattern = unsafe {
            focused.GetCurrentPattern(UIA_ValuePatternId)
                .and_then(|pattern| pattern.cast())
        }.map_err(|_| crate::error::VaultError::AutoTypeError("Cannot verify the username field".into()))?;
        if unsafe { pattern.CurrentIsReadOnly() }.map_or(true, |value| value.as_bool()) {
            return Err(crate::error::VaultError::AutoTypeError("Username field is read-only".into()));
        }
        check_modifier_keys_released()?;
        send_ctrl_a_backspace_guarded(target_hwnd)?;
        for ch in text.chars() {
            check_identifier_focus(focused, target_hwnd)?;
            let mut encoded = zeroize::Zeroizing::new([0u8; 4]);
            autotype_text_with_delay_guarded(ch.encode_utf8(&mut encoded[..]), char_delay_ms, 0, target_hwnd)?;
        }
        // Let the browser process its input queue, but never submit a partial,
        // rejected or changed value, and never retry credentials automatically.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        loop {
            check_identifier_focus(focused, target_hwnd)?;
            let matches = unsafe { pattern.CurrentValue() }
                .map(|value| zeroize::Zeroizing::new(value.to_string()).as_str() == text)
                .unwrap_or(false);
            if matches { return Ok(()); }
            if std::time::Instant::now() >= deadline {
                return Err(crate::error::VaultError::AutoTypeError("Username field did not accept the input".into()));
            }
            safe_sleep_with_target_guard(25, target_hwnd)?;
        }
    } else {
        inject_secret_guarded(focused, text, target_hwnd, char_delay_ms)
    }
}

fn check_identifier_focus(focused: &IUIAutomationElement, target_hwnd: HWND) -> crate::Result<()> {
    check_target_window_active(target_hwnd)?;
    if !unsafe { focused.CurrentHasKeyboardFocus() }.is_ok_and(|value| value.as_bool()) {
        return Err(crate::error::VaultError::AutoTypeError("Username field lost keyboard focus".into()));
    }
    Ok(())
}

/// Capture the foreground window before CDP verifies that its page has focus.
pub(crate) fn foreground_browser_token() -> crate::Result<usize> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() || !is_known_web_browser(&get_window_process_name(hwnd)) {
        return Err(crate::error::VaultError::AutoTypeError("Focus the browser login page".into()));
    }
    Ok(hwnd.0 as usize)
}

fn inject_password_guarded(
    focused: &IUIAutomationElement,
    text: &str,
    target_hwnd: HWND,
    char_delay_ms: u64,
) -> crate::Result<()> {
    if text.chars().any(char::is_control) {
        return Err(crate::error::VaultError::AutoTypeError("Password contains control characters".into()));
    }
    check_identifier_focus(focused, target_hwnd)?;
    if unsafe { focused.CurrentIsPassword() }.map(|v| v.as_bool()).ok() != Some(true) {
        return Err(crate::error::VaultError::AutoTypeError("Expected a protected password field".into()));
    }
    if let Ok(pattern) = unsafe { focused.GetCurrentPattern(UIA_ValuePatternId) }
        && let Ok(val_pattern) = pattern.cast::<IUIAutomationValuePattern>()
        && unsafe { val_pattern.CurrentIsReadOnly() }.map_or(false, |v| v.as_bool()) {
            return Err(crate::error::VaultError::AutoTypeError("Password field is read-only".into()));
        }
    check_modifier_keys_released()?;
    send_ctrl_a_backspace_guarded(target_hwnd)?;
    safe_sleep_with_target_guard(50, target_hwnd)?;

    for ch in text.chars() {
        check_identifier_focus(focused, target_hwnd)?;
        if unsafe { focused.CurrentIsPassword() }.map(|v| v.as_bool()).ok() != Some(true) {
            return Err(crate::error::VaultError::AutoTypeError("Password field changed type".into()));
        }
        let mut encoded = zeroize::Zeroizing::new([0u8; 4]);
        autotype_text_with_delay_guarded(ch.encode_utf8(&mut encoded[..]), char_delay_ms, 0, target_hwnd)?;
    }
    safe_sleep_with_target_guard(50, target_hwnd)?;
    check_identifier_focus(focused, target_hwnd)?;
    Ok(())
}

/// Type into a focused browser field (identifier or password) using OS physical scan codes.
pub(crate) fn type_browser_field(
    text: &str,
    window_token: usize,
    field_id: &str,
    is_password: bool,
    char_delay_ms: u64,
    expected_url: &str,
) -> crate::Result<()> {
    use windows::Win32::System::Com::CoUninitialize;
    let hwnd = HWND(window_token as *mut _);
    check_target_window_active(hwnd)?;
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok()
        .map_err(|_| crate::error::VaultError::AutoTypeError("Cannot initialize browser input".into()))?;
    struct ComGuard;
    impl Drop for ComGuard { fn drop(&mut self) { unsafe { CoUninitialize(); } } }
    let _com = ComGuard;
    let automation: IUIAutomation = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL) }
        .map_err(|_| crate::error::VaultError::AutoTypeError("Cannot inspect browser input".into()))?;
    let address_deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let actual = loop {
        check_target_window_active(hwnd)?;
        if let Some(actual) = browser_address(&automation, hwnd) { break actual; }
        if std::time::Instant::now() >= address_deadline {
            return Err(crate::error::VaultError::AutoTypeError("Cannot inspect the active browser address".into()));
        }
        safe_sleep_with_target_guard(50, hwnd)?;
    };
    if !same_browser_document(expected_url, &actual) {
        return Err(crate::error::VaultError::AutoTypeError("The selected browser page is not the active window".into()));
    }
    if let Ok(window) = unsafe { automation.ElementFromHandle(hwnd) } {
        let _ = find_targeted_elements(&automation, &window, UIA_EditControlTypeId);
    }
    // Chromium enables its accessibility tree lazily on the first UIA request.
    // Poll readiness without moving focus or sending input to the container.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let focused = loop {
        check_target_window_active(hwnd)?;
        if let Ok(element) = unsafe { automation.GetFocusedElement() } {
            if unsafe { element.CurrentControlType() }.ok() == Some(UIA_EditControlTypeId) {
                break element;
            }
        }
        if std::time::Instant::now() >= deadline {
            return Err(crate::error::VaultError::AutoTypeError("Cannot find focused browser input".into()));
        }
        if let Ok(window) = unsafe { automation.ElementFromHandle(hwnd) } {
            let _ = find_targeted_elements(&automation, &window, UIA_EditControlTypeId);
        }
        safe_sleep_with_target_guard(25, hwnd)?;
    };
    if unsafe { focused.CurrentControlType() }.ok() != Some(UIA_EditControlTypeId) {
        return Err(crate::error::VaultError::AutoTypeError("Browser input focus changed".into()));
    }
    if unsafe { focused.CurrentIsPassword() }.map(|value| value.as_bool()).ok() != Some(is_password) {
        return Err(crate::error::VaultError::AutoTypeError("Browser input focus changed".into()));
    }
    if !field_id.is_empty() {
        let auto_id = unsafe { focused.CurrentAutomationId() }.map(|id| id.to_string()).unwrap_or_default();
        if auto_id != field_id {
            return Err(crate::error::VaultError::AutoTypeError("Browser input identity changed".into()));
        }
    }
    if is_password {
        inject_password_guarded(&focused, text, hwnd, char_delay_ms)
    } else {
        inject_identifier_guarded(&focused, text, hwnd, char_delay_ms)
    }
}

fn send_shift_tab_guarded(target_hwnd: HWND) -> crate::Result<()> {
    let send_single = |vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS| -> crate::Result<()> {
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

    send_single(VK_SHIFT, 0x2A, KEYBD_EVENT_FLAGS(0))?;
    safe_sleep_with_target_guard(15, target_hwnd)?;

    send_single(VK_TAB, 0x0F, KEYBD_EVENT_FLAGS(0))?;
    safe_sleep_with_target_guard(15, target_hwnd)?;

    send_single(VK_TAB, 0x0F, KEYEVENTF_KEYUP)?;
    safe_sleep_with_target_guard(15, target_hwnd)?;

    send_single(VK_SHIFT, 0x2A, KEYEVENTF_KEYUP)?;

    Ok(())
}

fn send_ctrl_a_backspace_guarded(target_hwnd: HWND) -> crate::Result<()> {
    check_target_window_active(target_hwnd)?;
    check_modifier_keys_released()?;
    let layout = unsafe { GetKeyboardLayout(GetWindowThreadProcessId(target_hwnd, None)) };
    let a_scan = unsafe { MapVirtualKeyExW(0x41, MAPVK_VK_TO_VSC_EX, layout) } as u16;
    if a_scan == 0 {
        return Err(crate::error::VaultError::AutoTypeError("Cannot map Select All shortcut".into()));
    }
    send_character_chord(&[scan_input(0x1d, false), scan_input(a_scan, false), scan_input(a_scan, true), scan_input(0x1d, true)], target_hwnd)?;
    send_character_chord(&[scan_input(0x0e, false), scan_input(0x0e, true)], target_hwnd)
}
pub(crate) fn send_enter_guarded(target_hwnd: HWND) -> crate::Result<()> {
    check_target_window_active(target_hwnd)?;
    check_modifier_keys_released()?;

    let input_down = scan_input(0x1C, false);
    let input_up = scan_input(0x1C, true);
    send_character_chord(&[input_down, input_up], target_hwnd)
}

fn send_backspaces_guarded(count: usize, target_hwnd: HWND) -> crate::Result<()> {
    for _ in 0..count {
        check_target_window_active(target_hwnd)?;
        let input_down = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_BACK,
                    wScan: 0x0E,
                    dwFlags: KEYBD_EVENT_FLAGS(0),
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

fn common_prefix_len(s1: &str, s2: &str) -> usize {
    s1.chars().zip(s2.chars())
      .take_while(|(c1, c2)| c1 == c2)
      .count()
}

fn autotype_correct_text_guarded(current: &str, target: &str, char_delay_ms: u64, target_hwnd: HWND) -> crate::Result<()> {
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

fn inject_secret_guarded(
    focused: &IUIAutomationElement,
    text: &str,
    target_hwnd: HWND,
    char_delay_ms: u64,
) -> crate::Result<()> {
    check_target_window_active(target_hwnd)?;

    // If target is a web browser, use pure native keyboard typing (no clipboard paste / no Ctrl+V)
    if is_known_web_browser(&get_window_process_name(target_hwnd)) {
        return if unsafe { focused.CurrentIsPassword() }.is_ok_and(|v| v.as_bool()) {
            inject_password_guarded(focused, text, target_hwnd, char_delay_ms)
        } else {
            inject_identifier_guarded(focused, text, target_hwnd, char_delay_ms)
        };
    }

    // Direct keystroke typing for native desktop applications as well (no clipboard paste)
    autotype_text_with_delay_guarded(text, char_delay_ms, 0, target_hwnd)
}

fn get_element_value(focused: &IUIAutomationElement) -> String {
    unsafe {
        if let Ok(pattern_obj) = focused.GetCurrentPattern(UIA_ValuePatternId)
            && let Ok(val_pattern) = pattern_obj.cast::<IUIAutomationValuePattern>()
                && let Ok(bstr_val) = val_pattern.CurrentValue() {
                    return bstr_val.to_string();
                }
    }
    String::new()
}

fn get_window_title(hwnd: HWND) -> String {
    let mut buf = [0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, &mut buf) };
    if len > 0 {
        String::from_utf16_lossy(&buf[..len as usize])
    } else {
        String::new()
    }
}

fn get_window_process_name(hwnd: HWND) -> String {
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

fn extract_domain_token(url: &str) -> String {
    let parsed = match reqwest::Url::parse(url) {
        Ok(p) => p,
        Err(_) => return String::new(),
    };
    let host = match parsed.host_str() {
        Some(h) => h.to_lowercase(),
        None => return String::new(),
    };
    let parts: Vec<&str> = host.split('.').collect();
    if parts.is_empty() {
        return String::new();
    }
    if parts.len() == 1 {
        return parts[0].to_string();
    }

    // Check multi-part second-level domain (e.g. .co.uk, .com.au, .co.jp)
    if parts.len() >= 3 {
        let last_two = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
        let is_multipart = matches!(
            last_two.as_str(),
            "co.uk" | "gov.uk" | "ac.uk" | "org.uk" | "net.uk" |
            "com.au" | "net.au" | "org.au" | "edu.au" | "gov.au" |
            "co.jp" | "ne.jp" | "or.jp" | "go.jp" | "ac.jp" |
            "com.br" | "net.br" | "org.br" | "gov.br" |
            "co.nz" | "net.nz" | "org.nz" | "gov.nz" |
            "com.tr" | "org.tr" | "net.tr" | "gov.tr" |
            "com.sg" | "edu.sg" | "gov.sg" |
            "com.mx" | "org.mx" | "gob.mx"
        ) || (parts[parts.len() - 1].len() == 2 && parts[parts.len() - 2].len() <= 3);

        if is_multipart && parts.len() >= 3 {
            return parts[parts.len() - 3].to_string();
        }
    }

    // Standard single-part TLD (e.g. .com, .org, .net, .io, .se, .dev)
    if parts.len() >= 2 {
        return parts[parts.len() - 2].to_string();
    }

    parts[0].to_string()
}

fn verified_browser_url(expected: &str, actual: &str) -> bool {
    let Ok(actual) = reqwest::Url::parse(actual) else { return false; };
    actual.scheme() == "https" && actual.username().is_empty() && actual.password().is_none()
        && crate::smartlogin::discovery::is_allowed_auth_domain(expected, actual.as_str())
}

fn same_browser_document(expected: &str, actual: &str) -> bool {
    let (Ok(mut expected), Ok(mut actual)) = (reqwest::Url::parse(expected), reqwest::Url::parse(actual)) else { return false; };
    if !matches!(expected.scheme(), "https" | "http") { return false; }
    // Chromium also elides HTTP on loopback; local development pages remain verifiable by origin/path.
    if expected.scheme() == "http" && matches!(expected.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))
        && actual.scheme() == "https" { let _ = actual.set_scheme("http"); }
    expected.set_fragment(None);
    actual.set_fragment(None);
    expected == actual
}

/// Before native Enter, bind the selected CDP page to a focused document input.
pub(crate) fn verify_browser_submit(window_token: usize, expected_url: &str) -> bool {
    let hwnd = HWND(window_token as *mut _);
    if !is_target_window_active(hwnd) { return false; }
    if unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_err() { return false; }
    let valid = (|| {
        let automation: IUIAutomation = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL) }.ok()?;
        let actual = browser_address(&automation, hwnd)?;
        let focused = unsafe { automation.GetFocusedElement() }.ok()?;
        Some(same_browser_document(expected_url, &actual)
            && unsafe { focused.CurrentControlType() }.ok() == Some(UIA_EditControlTypeId)
            && native_login::document_field(&automation, &focused)
            && check_identifier_focus(&focused, hwnd).is_ok())
    })().unwrap_or(false);
    unsafe { windows::Win32::System::Com::CoUninitialize(); }
    valid
}

/// Chromium elides the outer scheme while redirect queries can still contain
/// an inner https:// URL. Only a scheme at the start belongs to this document.
fn normalize_browser_address(address: &str) -> Option<String> {
    let address = address.trim();
    if address.is_empty() || address.contains(char::is_whitespace) { return None; }
    let normalized = if address.to_ascii_lowercase().starts_with("https://")
        || address.to_ascii_lowercase().starts_with("http://") {
        address.to_owned()
    } else {
        format!("https://{address}")
    };
    let url = reqwest::Url::parse(&normalized).ok()?;
    if !url.username().is_empty() || url.password().is_some()
        || !url.host_str().is_some_and(|host| host.contains('.') || host == "localhost" || host == "[::1]") {
        return None;
    }
    Some(normalized)
}

/// Read only browser chrome: an edit beneath a native toolbar, never a web document.
fn browser_address(automation: &IUIAutomation, hwnd: HWND) -> Option<String> {
    unsafe {
        let window = automation.ElementFromHandle(hwnd).ok()?;
        let edits = find_targeted_elements(automation, &window, UIA_EditControlTypeId)?;
        let walker = automation.ControlViewWalker().ok()?;
        for index in 0..edits.Length().ok()?.min(100) {
            let Ok(edit) = edits.GetElement(index) else { continue; };
            if edit.CurrentIsOffscreen().map_or(true, |v| v.as_bool()) { continue; }
            let mut ancestor = edit.clone();
            let mut toolbar = false;
            let mut chrome = false;
            for _ in 0..24 {
                let Ok(parent) = walker.GetParentElement(&ancestor) else { break; };
                let kind = parent.CurrentControlType().ok();
                if kind == Some(UIA_DocumentControlTypeId) { break; }
                toolbar |= kind == Some(UIA_ToolBarControlTypeId);
                if kind == Some(UIA_WindowControlTypeId) {
                    chrome = toolbar && automation.CompareElements(&parent, &window).is_ok_and(|v| v.as_bool());
                    break;
                }
                ancestor = parent;
            }
            if !chrome { continue; }
            let Ok(pattern) = edit.GetCurrentPattern(UIA_ValuePatternId) else { continue; };
            let Ok(value) = pattern.cast::<IUIAutomationValuePattern>() else { continue; };
            let Ok(value) = value.CurrentValue() else { continue; };
            let address = value.to_string();
            // Chromium's unfocused omnibox elides the scheme. This verifies the
            // hostname, not the TLS state; never accept an explicitly insecure URL.
            if let Some(address) = normalize_browser_address(&address) { return Some(address); }
        }
    }
    None
}

fn is_verified_login_context(
    hwnd: HWND,
    title: &str,
    expected_url: &str,
    automation: &IUIAutomation,
) -> bool {
    let title_lower = title.to_lowercase();
    let proc_name = get_window_process_name(hwnd);
    let is_browser = is_known_web_browser(&proc_name);
    if is_browser && !expected_url.is_empty() {
        return browser_address(automation, hwnd)
            .is_some_and(|actual| verified_browser_url(expected_url, &actual));
    }
    let domain_token = extract_domain_token(expected_url);
    let target_domain_token = domain_token.as_str();

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
        let is_token_match = |text: &str| -> bool {
            if text.contains(target_domain_token) {
                return true;
            }
            if target_domain_token == "steampowered" && text.contains("steam") {
                return true;
            }
            false
        };

        if is_browser {
            // Browser window MUST contain the verified domain token in its title bar snippet
            // E.g., "Sign in to GitHub · GitHub - Google Chrome" contains "github" -> Verified.
            if !is_token_match(&title_lower) {
                return false;
            }
        } else {
            // Native Application window: Process name or window title MUST match domain token
            // E.g., "discord.exe" matches "discord" -> Verified.
            // Spoofed app "phish.exe" with title "Sign In - Google Chrome" -> Rejected!
            if !is_token_match(&proc_name) && !is_token_match(&title_lower) {
                return false;
            }
        }
    } else if !is_browser {
        // If no URL is provided AND process is not a recognized browser, require an explicit password field
        if let Ok(win_el) = unsafe { automation.ElementFromHandle(hwnd) }
            && !active_window_has_password_field(automation, &win_el) {
                return false;
            }
    }

    true
}

fn poll_until_login_context_ready(
    automation: &IUIAutomation,
    domain_token: &str,
    max_timeout_ms: u64,
) -> bool {
    let steps = max_timeout_ms / 50;
    for _ in 0..steps {
        let hwnd = unsafe { GetForegroundWindow() };
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

fn is_likely_login_text(text: &str) -> bool {
    let t = text.to_lowercase().replace([' ', '-'], "");
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

fn is_non_login_input_field(name: &str, class_name: &str, auto_id: &str) -> bool {
    let n = name.to_lowercase();
    let c = class_name.to_lowercase();
    let i = auto_id.to_lowercase();

    let keywords = [
        "search", "sök", "find", "chat", "message", "reply", "comment",
        "prompt", "filter", "query", "ask", "fråga", "gpt", "copilot",
        "newsletter", "subscribe", "coupon", "promo", "discount", "rabatt",
        "prenumerera", "address", "street", "zip", "postcode", "city",
        "cvv", "card", "subject", "feedback", "review",
    ];

    for kw in keywords {
        if n.contains(kw) || c.contains(kw) || i.contains(kw) {
            return true;
        }
    }
    false
}

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
        "identifier", "handle", "signin", "sign-in", "log-in",
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

fn find_targeted_elements(
    automation: &IUIAutomation,
    window_el: &IUIAutomationElement,
    control_type_id: UIA_CONTROLTYPE_ID,
) -> Option<IUIAutomationElementArray> {
    unsafe {
        let var = windows::core::VARIANT::from(control_type_id.0);
        let cond = automation.CreatePropertyCondition(UIA_ControlTypePropertyId, &var).ok()?;
        window_el.FindAll(TreeScope_Descendants, &cond).ok()
    }
}

fn try_click_login_link(
    automation: &IUIAutomation,
    window_el: &IUIAutomationElement,
) -> bool {
    let check_array = |elements: IUIAutomationElementArray| -> bool {
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
                    if let Ok(pattern_obj) = el.GetCurrentPattern(UIA_InvokePatternId)
                        && let Ok(invoke_pattern) = pattern_obj.cast::<IUIAutomationInvokePattern>()
                            && invoke_pattern.Invoke().is_ok() {
                                return true;
                            }
                }
            }
        }
        false
    };

    if let Some(buttons) = find_targeted_elements(automation, window_el, UIA_ButtonControlTypeId)
        && check_array(buttons) {
            return true;
        }

    if let Some(links) = find_targeted_elements(automation, window_el, UIA_HyperlinkControlTypeId)
        && check_array(links) {
            return true;
        }

    false
}

fn try_focus_username_field(
    automation: &IUIAutomation,
    window_el: &IUIAutomationElement,
) -> bool {
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

fn is_on_register_page(
    automation: &IUIAutomation,
    window_el: &IUIAutomationElement,
) -> bool {
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

fn active_window_has_password_field(
    automation: &IUIAutomation,
    window_el: &IUIAutomationElement,
) -> bool {
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

// ─── Windows Driver Implementation ──────────────────────────────────────

pub(crate) struct WindowsAutotypeDriver;

impl WindowsAutotypeDriver {
    pub(crate) const fn new() -> Self {
        Self
    }
}

impl AutotypeDriver for WindowsAutotypeDriver {
    fn autotype_text_with_delay(
        &self,
        text: &str,
        char_delay_ms: u64,
        settle_delay_ms: u64,
    ) -> crate::Result<()> {
        autotype_text_with_delay_guarded(text, char_delay_ms, settle_delay_ms, HWND::default())
    }

    fn run_smart_autotype(
        &self,
        guard: AutotypeGuard,
        url: &str,
        launch_browser: bool,
        char_delay_ms: u64,
        field_delay_ms: u64,
    ) -> crate::Result<()> {
        let normalized_url = if !url.is_empty() {
            if !url.starts_with("http://") && !url.starts_with("https://") {
                format!("https://{}", url)
            } else {
                url.to_string()
            }
        } else {
            String::new()
        };

        std::thread::spawn(move || {
            let expected_url = normalized_url.clone();

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
                        let active_token = extract_domain_token(&target_url);
                        !active_token.is_empty() && title.contains(&active_token)
                    } else {
                        false
                    };

                    if !is_already_active {
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
                        let _ = poll_until_login_context_ready(&automation, &expected_url, 3500);

                        // Fallback: If we landed on a homepage (e.g. because resolver fell back to original base URL)
                        // and no input is focused, try to find and click a login link.
                        let hwnd = GetForegroundWindow();
                        if !hwnd.is_invalid()
                            && let Ok(window_el) = automation.ElementFromHandle(hwnd) {
                                let mut already_on_login_form = false;
                                if let Ok(focused) = automation.GetFocusedElement() {
                                    let class_name = focused.CurrentClassName()
                                        .map(|b| b.to_string().to_lowercase())
                                        .unwrap_or_default();
                                    let control_type = focused.CurrentLocalizedControlType()
                                        .map(|b| b.to_string().to_lowercase())
                                        .unwrap_or_default();
                                    let control_id = focused.CurrentControlType().unwrap_or(UIA_CONTROLTYPE_ID(0));
                                    if control_id == UIA_EditControlTypeId
                                        || class_name.contains("edit")
                                        || control_type.contains("edit")
                                        || control_type.contains("text box")
                                    {
                                        already_on_login_form = true;
                                    }
                                }

                                // SOTA: Prevent clicking login links if we are already on a login page containing a password field
                                if !already_on_login_form
                                    && active_window_has_password_field(&automation, &window_el) {
                                        already_on_login_form = true;
                                    }

                                if !already_on_login_form
                                    && try_click_login_link(&automation, &window_el) {
                                        let _ = poll_until_login_context_ready(&automation, &expected_url, 3000);
                                    }
                            }
                    }
                }

                let mut last_focused_element_id: Option<String> = None;
                let mut filled_username = guard.username.is_empty();
                let mut filled_password = guard.password.is_empty();
                let mut filled_totp = guard.totp_secret.is_empty();
                let mut focus_attempts = 0;
                let mut target_hwnd = HWND::default();

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
                    let is_login_context = is_verified_login_context(hwnd, &title, &expected_url, &automation);

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

                    let control_id = focused.CurrentControlType().unwrap_or(UIA_CONTROLTYPE_ID(0));

                    let element_key = format!("{}-{}-{}", class_name, name, control_type);
                    if last_focused_element_id.as_ref() == Some(&element_key) {
                        continue;
                    }

                    // Check ControlTypeID as language-independent criteria
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
                            if !hwnd.is_invalid()
                                && let Ok(win_el) = automation.ElementFromHandle(hwnd) {
                                    let _ = try_focus_username_field(&automation, &win_el);
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

                        if on_register
                            && let Ok(win_el) = automation.ElementFromHandle(hwnd)
                                && try_click_login_link(&automation, &win_el) {
                                    let _ = poll_until_login_context_ready(&automation, &expected_url, 3000);
                                    last_focused_element_id = None; // Reset focus to re-evaluate on redirected page
                                    continue;
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
                                    if inject_identifier_guarded(&new_focused, &guard.username, target_hwnd, char_delay_ms).is_err() { break; }
                                } else if autotype_correct_text_guarded(&user_val, &guard.username, char_delay_ms, target_hwnd).is_err() {
                                    break;
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
                            if inject_password_guarded(&focused, &guard.password, target_hwnd, char_delay_ms).is_err() { break; }
                            if safe_sleep_with_target_guard(field_delay_ms, target_hwnd).is_err() { break; }
                            if send_enter_guarded(target_hwnd).is_err() { break; }
                            filled_password = true;
                        } else if is_username_field && !filled_username {
                            last_focused_element_id = Some(element_key.clone());

                            if inject_identifier_guarded(&focused, &guard.username, target_hwnd, char_delay_ms).is_err() { break; }
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
                                    if inject_password_guarded(&pass_focused, &guard.password, target_hwnd, char_delay_ms).is_err() { break; }
                                } else {
                                    break;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn browser_url_requires_verified_https_origin() {
        assert!(verified_browser_url("https://gmail.com", "https://accounts.google.com/signin"));
        for target in ["http://accounts.google.com", "https://accounts.google.com.evil.test", "https://accounts.google.com@evil.test", "https://evil.test/login/google", "accounts.google.com"] {
            assert!(!verified_browser_url("https://gmail.com", target));
        }
    }

    #[test]
    fn browser_target_binding_rejects_other_documents_and_error_pages() {
        assert!(same_browser_document("https://example.test/login#one", "https://example.test/login#two"));
        assert!(!same_browser_document("https://example.test/login", "https://other.test/login"));
        assert!(!same_browser_document("https://example.test/login", "https://example.test/other"));
        assert!(!same_browser_document("https://example.test/login?a=1", "https://example.test/login?a=2"));
        assert!(!same_browser_document("chrome-error://chromewebdata/", "https://example.test/"));
        assert!(!same_browser_document("https://example.test/login", ""));
        assert!(same_browser_document("http://127.0.0.1:1234/", "https://127.0.0.1:1234/"));
    }

    #[test]
    fn elided_browser_scheme_is_not_confused_by_redirect_urls_or_email_queries() {
        for address in [
            "accounts.google.com/v3/signin/identifier?continue=https://mail.google.com/mail/",
            "accounts.google.com/v3/signin/challenge/pwd?Email=demo@example.test&continue=https://mail.google.com/",
        ] {
            let actual = normalize_browser_address(address).unwrap();
            assert_eq!(reqwest::Url::parse(&actual).unwrap().host_str(), Some("accounts.google.com"));
        }
        assert_eq!(normalize_browser_address("http://example.test/").as_deref(), Some("http://example.test/"));
        for address in ["chrome://newtab/", "about:blank", "user@accounts.google.com", "javascript:alert(1)", "not a url"] {
            assert!(normalize_browser_address(address).is_none(), "{address}");
        }
        let malicious = normalize_browser_address("evil.test/?continue=https://accounts.google.com").unwrap();
        assert!(!crate::smartlogin::native_state::account_host(&malicious));
    }

    #[test]
    #[ignore = "opens a disposable normal Brave window to verify native address-bar inspection"]
    fn native_browser_address_smoke() {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok().unwrap();
        let automation: IUIAutomation = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL) }.unwrap();
        let profile = tempfile::tempdir().unwrap();
        let mut child = std::process::Command::new(r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe")
            .arg(format!("--user-data-dir={}", profile.path().display())).arg("--no-first-run")
            .arg("https://example.test/native-address-check").spawn().unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let mut found = None;
        while std::time::Instant::now() < deadline {
            let hwnd = unsafe { GetForegroundWindow() };
            let mut pid = 0;
            unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)); }
            if pid == child.id() {
                found = browser_address(&automation, hwnd);
                if found.is_some() { break; }

            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        let _ = child.kill();
        let _ = child.wait();
        drop(automation);
        unsafe { windows::Win32::System::Com::CoUninitialize(); }
        assert_eq!(found.as_deref(), Some("https://example.test/native-address-check"));
    }
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyboardLayoutList, LoadKeyboardLayoutW, UnloadKeyboardLayout, ACTIVATE_KEYBOARD_LAYOUT_FLAGS,
    };

    #[test]
    #[ignore = "opens normal Brave and tests Google's identifier step with an explicitly supplied test address"]
    fn google_native_identifier_diagnostic() {
        use windows::Win32::System::Com::CoUninitialize;
        use windows::Win32::UI::Accessibility::UIA_TextControlTypeId;
        let email = zeroize::Zeroizing::new(std::env::var("YNTRA_GOOGLE_TEST_EMAIL").expect("explicit test identifier required"));
        let profile = tempfile::tempdir().unwrap();
        let child = std::process::Command::new(r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe")
            .arg(format!("--user-data-dir={}", profile.path().display()))
            .arg("--no-first-run").arg("https://accounts.google.com/AddSession?service=mail")
            .spawn().unwrap();
        struct TestBrowser(std::process::Child);
        impl Drop for TestBrowser { fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); } }
        let child = TestBrowser(child);
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok().unwrap();
        struct ComGuard;
        impl Drop for ComGuard { fn drop(&mut self) { unsafe { CoUninitialize(); } } }
        let _com = ComGuard;
        let automation: IUIAutomation = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL) }.unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(40);
        let (hwnd, field) = loop {
            let hwnd = unsafe { GetForegroundWindow() };
            let mut pid = 0;
            unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)); }
            if pid == child.0.id() {
                if let Ok(window) = unsafe { automation.ElementFromHandle(hwnd) } {
                    if let Some(elements) = find_targeted_elements(&automation, &window, UIA_EditControlTypeId) {
                        let mut found = None;
                        for i in 0..unsafe { elements.Length() }.unwrap_or(0) {
                            let element = unsafe { elements.GetElement(i) }.unwrap();
                            if unsafe { element.CurrentAutomationId() }.is_ok_and(|id| id.to_string() == "identifierId")
                                && !unsafe { element.CurrentIsOffscreen() }.map_or(true, |b| b.as_bool()) {
                                found = Some(element); break;
                            }
                        }
                        if let Some(field) = found { break (hwnd, field); }
                    }
                }
            }
            assert!(std::time::Instant::now() < deadline, "Normal Brave identifier input was not found in foreground");
            std::thread::sleep(std::time::Duration::from_millis(200));
        };
        eprintln!("Normal Brave ready; title={}, native Auto-type starts now", get_window_title(hwnd));
        unsafe { field.SetFocus() }.unwrap();
        let focus_deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let field = loop {
            check_target_window_active(hwnd).unwrap();
            if let Ok(current) = unsafe { automation.GetFocusedElement() } {
                if unsafe { current.CurrentAutomationId() }.is_ok_and(|id| id.to_string() == "identifierId")
                    && check_identifier_focus(&current, hwnd).is_ok() { break current; }
            }
            assert!(std::time::Instant::now() < focus_deadline, "Identifier focus did not settle");
            std::thread::sleep(std::time::Duration::from_millis(25));
        };
        inject_identifier_guarded(&field, &email, hwnd, 15).unwrap();
        safe_sleep_with_target_guard(300, hwnd).unwrap();
        send_enter_guarded(hwnd).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            check_target_window_active(hwnd).unwrap();
            let window = unsafe { automation.ElementFromHandle(hwnd) }.unwrap();
            let password = active_window_has_password_field(&automation, &window);
            let mut text = String::new();
            if let Some(elements) = find_targeted_elements(&automation, &window, UIA_TextControlTypeId) {
                for i in 0..unsafe { elements.Length() }.unwrap_or(0).min(80) {
                    let element = unsafe { elements.GetElement(i) }.unwrap();
                    if !unsafe { element.CurrentIsOffscreen() }.map_or(true, |b| b.as_bool()) {
                        if let Ok(name) = unsafe { element.CurrentName() } { text.push_str(&name.to_string()); text.push('\n'); }
                    }
                }
            }
            let rejected = text.to_lowercase().contains("may not be secure") || text.to_lowercase().contains("kanske inte är säker");
            if password || rejected || std::time::Instant::now() >= deadline {
                eprintln!("Normal Brave outcome: password_visible={password}, rejected={rejected}, text={text}");
                // Leave the result visible long enough for the operator to inspect it.
                std::thread::sleep(std::time::Duration::from_secs(20));
                assert!(password || rejected, "No conclusive Google identifier result");
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }

    static TEST_LAYOUT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    struct TestLayout {
        handle: HKL,
        unload: bool,
        _lock: std::sync::MutexGuard<'static, ()>,
    }
    impl Drop for TestLayout {
        fn drop(&mut self) {
            if self.unload { unsafe { let _ = UnloadKeyboardLayout(self.handle); } }
        }
    }

    fn test_layout(id: &str) -> TestLayout {
        let lock = TEST_LAYOUT_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut previous = vec![HKL::default(); unsafe { GetKeyboardLayoutList(None) } as usize];
        unsafe { GetKeyboardLayoutList(Some(&mut previous)); }
        let id: Vec<u16> = id.encode_utf16().chain(Some(0)).collect();
        // No KLF_ACTIVATE: tests neither change the active layout nor send input.
        let handle = unsafe { LoadKeyboardLayoutW(PCWSTR(id.as_ptr()), ACTIVATE_KEYBOARD_LAYOUT_FLAGS(0)) }
            .expect("standard Windows keyboard layout");
        TestLayout { handle, unload: !previous.contains(&handle), _lock: lock }
    }

    fn down_scans(inputs: &[INPUT]) -> Vec<u16> {
        inputs.iter().filter_map(|input| {
            let key = unsafe { input.Anonymous.ki };
            assert_eq!(key.wVk, VIRTUAL_KEY(0));
            assert_ne!(key.dwFlags & KEYEVENTF_SCANCODE, KEYBD_EVENT_FLAGS(0));
            if key.dwFlags & KEYEVENTF_KEYUP != KEYBD_EVENT_FLAGS(0) { return None; }
            Some(key.wScan | if key.dwFlags & KEYEVENTF_EXTENDEDKEY != KEYBD_EVENT_FLAGS(0) { 0xe000 } else { 0 })
        }).collect()
    }

    #[test]
    fn keyboard_layout_email_symbols_use_the_expected_physical_keys() {
        for (layout_id, ch, expected) in [
            ("00000409", '@', vec![0x2a, 0x03]), // US Shift+2
            ("0000041d", '@', vec![0x1d, 0xe038, 0x03]), // Swedish AltGr+2
            ("00000407", '@', vec![0x1d, 0xe038, 0x10]), // German AltGr+Q
            ("0000041d", '€', vec![0x1d, 0xe038, 0x06]), // Swedish AltGr+5
            ("0000041d", 'å', vec![0x1a]),
            ("00000410", '.', vec![0x34]), // Italian: regular period, not Num Lock-dependent decimal
        ] {
            let layout = test_layout(layout_id);
            let (inputs, count) = layout_character_inputs(ch as u16, layout.handle, false).unwrap();
            assert_eq!(down_scans(&inputs[..count]), expected, "{layout_id} {ch}");
            // Every key down has a corresponding key up in reverse chord order.
            assert_eq!(count, expected.len() * 2);
            for (down, up) in inputs[..count / 2].iter().zip(inputs[count / 2..count].iter().rev()) {
                let (down, up) = unsafe { (down.Anonymous.ki, up.Anonymous.ki) };
                assert_eq!(down.wScan, up.wScan);
                assert_eq!(down.dwFlags | KEYEVENTF_KEYUP, up.dwFlags);
            }
        }
    }

    #[test]
    fn keyboard_layout_caps_lock_preserves_case_and_symbols() {
        let layout = test_layout("0000041d");
        for (ch, expected) in [('a', vec![0x2a, 0x1e]), ('A', vec![0x1e]), ('å', vec![0x2a, 0x1a]), ('Å', vec![0x1a]), ('@', vec![0x1d, 0xe038, 0x03])] {
            let (inputs, count) = layout_character_inputs(ch as u16, layout.handle, true).unwrap();
            assert_eq!(down_scans(&inputs[..count]), expected, "{ch}");
        }
    }

    #[test]
    fn keyboard_layout_preserves_unicode_fallback_for_dead_and_unmapped_keys() {
        let layout = test_layout("0000041d");
        for ch in ['^' as u16, '漢' as u16, 0xd83d, 0xde00, 0, 9, 10, 13] {
            assert!(layout_character_inputs(ch, layout.handle, false).is_none(), "{ch:x}");
        }
    }

    #[test]
    fn keyboard_layout_matrix_round_trips_emitted_scan_codes() {
        use windows::Win32::UI::Input::KeyboardAndMouse::VK_RCONTROL;
        // Regional punctuation, AZERTY/QWERTZ, Dvorak, AltGr and non-Latin layouts.
        let layouts = [
            ("00000409", "US"), ("00000809", "UK"), ("00020409", "US International"),
            ("00010409", "US Dvorak"), ("0000041d", "Swedish"), ("00000414", "Norwegian"),
            ("00000406", "Danish"), ("0000040b", "Finnish"), ("00000407", "German"),
            ("00000807", "Swiss German"), ("0000040c", "French"), ("0000080c", "Belgian French"),
            ("0000040a", "Spanish"), ("0000080a", "Latin American"), ("00000410", "Italian"),
            ("00000816", "Portuguese"), ("00010416", "Brazilian ABNT2"),
            ("00000415", "Polish programmer"), ("00000405", "Czech"), ("0000040e", "Hungarian"),
            ("0000041f", "Turkish Q"), ("0001041f", "Turkish F"),
            ("00000419", "Russian"), ("00000422", "Ukrainian"), ("00000408", "Greek"),
            ("00000401", "Arabic 101"), ("0000040d", "Hebrew"),
        ];
        for (id, name) in layouts {
            let loaded = test_layout(id);
            let layout = loaded.handle;
            if !matches!(name, "Russian" | "Ukrainian" | "Greek" | "Arabic 101" | "Hebrew") {
                assert!(layout_character_inputs('@' as u16, layout, false).is_some(), "{name}: @ should have a physical mapping");
            }
            for caps_lock in [false, true] {
                let samples = (32u16..=126).chain("åÅäÄöÖéÉèÈçÇñÑßẞøØæÆ€£¥ıİğĞşŞąĄłŁžŽčČěěйЙяЯїЇαΑωΩشא漢".encode_utf16());
                let mut mapped_count = 0;
                for ch in samples {
                    let Some((inputs, count)) = layout_character_inputs(ch, layout, caps_lock) else { continue; };
                    mapped_count += 1;
                    // Decode the EMITTED physical scan codes back to virtual keys,
                    // independently of VkKeyScanEx's suggested mapping.
                    let scans = down_scans(&inputs[..count]);
                    let mut state = [0u8; 256];
                    state[VK_CAPITAL.0 as usize] = u8::from(caps_lock);
                    let mut main_vk = 0;
                    for scan in &scans {
                        let vk = unsafe { MapVirtualKeyExW(*scan as u32, MAPVK_VSC_TO_VK_EX, layout) };
                        assert!(vk > 0 && vk < 256, "{name}: unmapped scan code");
                        state[vk as usize] = 0x80;
                        let key = VIRTUAL_KEY(vk as u16);
                        if key == VK_LCONTROL || key == VK_RCONTROL { state[VK_CONTROL.0 as usize] = 0x80; }
                        if key == VK_MENU || key == VK_RMENU { state[VK_MENU.0 as usize] = 0x80; }
                        if key == VK_LSHIFT { state[VK_SHIFT.0 as usize] = 0x80; }
                        main_vk = vk;
                    }
                    let mut output = [0u16; 8];
                    let result = unsafe { ToUnicodeEx(main_vk, *scans.last().unwrap() as u32, &state, &mut output, 4, layout) };
                    assert_eq!(result, 1, "{name} caps={caps_lock} U+{ch:04X}");
                    assert_eq!(output[0], ch, "{name} caps={caps_lock}");
                }
                assert!(mapped_count >= 10, "{name}: layout did not provide usable mappings");
            }
        }
    }

    #[test]
    fn test_extract_domain_token_subdomains() {
        assert_eq!(extract_domain_token("https://store.steampowered.com/"), "steampowered");
        assert_eq!(extract_domain_token("https://steamcommunity.com/login"), "steamcommunity");
        assert_eq!(extract_domain_token("https://login.microsoftonline.com/"), "microsoftonline");
        assert_eq!(extract_domain_token("https://accounts.google.com/signin"), "google");
        assert_eq!(extract_domain_token("https://auth.bank.co.uk/"), "bank");
        assert_eq!(extract_domain_token("https://portal.service.com.au/app"), "service");
        assert_eq!(extract_domain_token("https://github.com/login"), "github");
        assert_eq!(extract_domain_token("https://sub.domain.se"), "domain");
    }
}

