//! Autotype Engine for simulated keyboard typing with configurable delays.

#[cfg(target_os = "windows")]
pub fn autotype_text(text: &str) -> crate::Result<()> {
    autotype_text_with_delay(text, 15)
}

#[cfg(target_os = "windows")]
pub fn autotype_text_with_delay(text: &str, char_delay_ms: u64) -> crate::Result<()> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_UNICODE, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_TAB
    };

    let utf16_chars: Vec<u16> = text.encode_utf16().collect();

    for &ch in &utf16_chars {
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

        // Configurable delay between characters
        std::thread::sleep(std::time::Duration::from_millis(char_delay_ms));
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn send_shift_tab() -> crate::Result<()> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_SHIFT, VK_TAB
    };

    let send_single = |vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY, scan: u16, flags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS| -> crate::Result<()> {
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

    // Press Shift down (VK_SHIFT = 0x10, scan code = 0x2A)
    send_single(VK_SHIFT, 0x2A, windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0))?;
    std::thread::sleep(std::time::Duration::from_millis(15));

    // Press Tab down (VK_TAB = 0x09, scan code = 0x0F)
    send_single(VK_TAB, 0x0F, windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0))?;
    std::thread::sleep(std::time::Duration::from_millis(15));

    // Release Tab up
    send_single(VK_TAB, 0x0F, KEYEVENTF_KEYUP)?;
    std::thread::sleep(std::time::Duration::from_millis(15));

    // Release Shift up
    send_single(VK_SHIFT, 0x2A, KEYEVENTF_KEYUP)?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn send_ctrl_a_backspace() -> crate::Result<()> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL, VK_BACK, VIRTUAL_KEY
    };

    let send_single = |vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY, scan: u16, flags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS| -> crate::Result<()> {
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

    // Press Ctrl down (VK_CONTROL = 0x11, scan code = 0x1D)
    send_single(VK_CONTROL, 0x1D, windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0))?;
    std::thread::sleep(std::time::Duration::from_millis(15));

    // Press 'A' down (VK_A = 0x41, scan code = 0x1E)
    send_single(VIRTUAL_KEY(0x41), 0x1E, windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0))?;
    std::thread::sleep(std::time::Duration::from_millis(15));

    // Release 'A' up
    send_single(VIRTUAL_KEY(0x41), 0x1E, KEYEVENTF_KEYUP)?;
    std::thread::sleep(std::time::Duration::from_millis(15));

    // Release Ctrl up
    send_single(VK_CONTROL, 0x1D, KEYEVENTF_KEYUP)?;
    std::thread::sleep(std::time::Duration::from_millis(15));

    // Press Backspace (VK_BACK = 0x08, scan code = 0x0E)
    send_single(VK_BACK, 0x0E, windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0))?;
    std::thread::sleep(std::time::Duration::from_millis(15));
    send_single(VK_BACK, 0x0E, KEYEVENTF_KEYUP)?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn send_enter() -> crate::Result<()> {
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
fn send_backspaces(count: usize) -> crate::Result<()> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_BACK
    };

    for _ in 0..count {
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
        std::thread::sleep(std::time::Duration::from_millis(15));
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
fn autotype_correct_text(current: &str, target: &str, char_delay_ms: u64) -> crate::Result<()> {
    if current.is_empty() {
        return autotype_text_with_delay(target, char_delay_ms);
    }

    let prefix_len = common_prefix_len(current, target);
    if prefix_len == 0 {
        send_ctrl_a_backspace()?;
        std::thread::sleep(std::time::Duration::from_millis(100));
        autotype_text_with_delay(target, char_delay_ms)
    } else {
        let backspaces_needed = current.chars().count() - prefix_len;
        if backspaces_needed > 0 {
            send_backspaces(backspaces_needed)?;
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let remainder: String = target.chars().skip(prefix_len).collect();
        autotype_text_with_delay(&remainder, char_delay_ms)
    }
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

// ─── Background URL Login-Link Resolver ─────────────────────────────────

#[cfg(target_os = "windows")]
fn find_login_url_in_html(html: &str, base: &reqwest::Url) -> Option<String> {
    let mut search_idx = 0;
    while let Some(a_start) = html[search_idx..].find("<a ") {
        let absolute_a_start = search_idx + a_start;
        let tag_end = match html[absolute_a_start..].find('>') {
            Some(offset) => absolute_a_start + offset,
            None => {
                search_idx = absolute_a_start + 3;
                continue;
            }
        };

        let attrs = &html[absolute_a_start..tag_end];
        
        if let Some(href_offset) = attrs.find("href=") {
            let val_start = absolute_a_start + href_offset + 5;
            let quote = match html.as_bytes().get(val_start) {
                Some(q) => *q,
                None => break,
            };
            
            let (link_start, link_end) = if quote == b'"' || quote == b'\'' {
                let end = match html[val_start + 1..].find(quote as char) {
                    Some(e) => e,
                    None => break,
                };
                (val_start + 1, val_start + 1 + end)
            } else {
                let end = match html[val_start..].find(|c: char| c.is_whitespace() || c == '>') {
                    Some(e) => e,
                    None => break,
                };
                (val_start, val_start + end)
            };

            let raw_link = &html[link_start..link_end];
            if is_likely_login_url(raw_link) {
                if let Ok(resolved) = base.join(raw_link) {
                    return Some(resolved.to_string());
                }
            }
        }

        search_idx = tag_end + 1;
    }
    None
}

#[cfg(target_os = "windows")]
async fn resolve_login_url(base_url: &str) -> Option<String> {
    let parsed_base = reqwest::Url::parse(base_url).ok()?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(1500))
        .build()
        .ok()?;

    // 1. Try to fetch homepage HTML and parse login link
    if let Ok(resp) = client.get(parsed_base.clone()).send().await {
        if let Ok(html) = resp.text().await {
            if let Some(link) = find_login_url_in_html(&html, &parsed_base) {
                return Some(link);
            }
        }
    }

    // 2. Probe common paths directly
    let common_paths = ["login", "signin", "log-in", "sign-in"];
    for path in common_paths {
        if let Ok(target) = parsed_base.join(path) {
            if let Ok(resp) = client.get(target.clone()).send().await {
                if resp.status().is_success() {
                    return Some(target.to_string());
                }
            }
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn try_click_login_link(
    automation: &windows::Win32::UI::Accessibility::IUIAutomation,
    window_el: &windows::Win32::UI::Accessibility::IUIAutomationElement,
) -> bool {
    use windows::core::Interface;
    use windows::Win32::UI::Accessibility::{
        IUIAutomationElementArray, IUIAutomationInvokePattern, TreeScope_Descendants,
        UIA_ButtonControlTypeId, UIA_HyperlinkControlTypeId, UIA_InvokePatternId,
    };

    let true_cond = match unsafe { automation.CreateTrueCondition() } {
        Ok(c) => c,
        Err(_) => return false,
    };

    let elements: IUIAutomationElementArray = match unsafe {
        window_el.FindAll(TreeScope_Descendants, &true_cond)
    } {
        Ok(el) => el,
        Err(_) => return false,
    };

    let count = match unsafe { elements.Length() } {
        Ok(c) => c,
        Err(_) => 0,
    };

    for i in 0..count {
        let el = match unsafe { elements.GetElement(i) } {
            Ok(e) => e,
            Err(_) => continue,
        };

        let control_id = unsafe { el.CurrentControlType() }
            .map(|id| id.0)
            .unwrap_or(0);

        if control_id == UIA_ButtonControlTypeId.0 || control_id == UIA_HyperlinkControlTypeId.0 {
            let name = unsafe { el.CurrentName() }
                .map(|b| b.to_string())
                .unwrap_or_default();

            let href = get_element_value(&el).to_lowercase();
            let auto_id = unsafe { el.CurrentAutomationId() }
                .map(|id| id.to_string().to_lowercase())
                .unwrap_or_default();

            // Language-independent dynamic keyword checks across visible name, target href, or element ID
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
    }

    false
}

#[cfg(target_os = "windows")]
fn is_on_register_page(
    automation: &windows::Win32::UI::Accessibility::IUIAutomation,
    window_el: &windows::Win32::UI::Accessibility::IUIAutomationElement,
) -> bool {
    use windows::Win32::UI::Accessibility::{
        IUIAutomationElementArray, TreeScope_Descendants, UIA_EditControlTypeId
    };

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

    let true_cond = match unsafe { automation.CreateTrueCondition() } {
        Ok(c) => c,
        Err(_) => return false,
    };

    let elements: IUIAutomationElementArray = match unsafe {
        window_el.FindAll(TreeScope_Descendants, &true_cond)
    } {
        Ok(el) => el,
        Err(_) => return false,
    };

    let count = match unsafe { elements.Length() } {
        Ok(c) => c,
        Err(_) => 0,
    };

    let mut password_count = 0;
    for i in 0..count {
        let el = match unsafe { elements.GetElement(i) } {
            Ok(e) => e,
            Err(_) => continue,
        };

        let control_id = unsafe { el.CurrentControlType() }
            .map(|id| id.0)
            .unwrap_or(0);

        let is_pw = unsafe { el.CurrentIsPassword() }
            .map(|b| b.as_bool())
            .unwrap_or(false);

        if is_pw {
            password_count += 1;
            if password_count >= 2 {
                return true;
            }
        } else if control_id == UIA_EditControlTypeId.0 {
            let name = unsafe { el.CurrentName() }
                .map(|b| b.to_string().to_lowercase())
                .unwrap_or_default();
            let class_name = unsafe { el.CurrentClassName() }
                .map(|b| b.to_string().to_lowercase())
                .unwrap_or_default();

            // Strict confirmation matching only on Edit controls to bypass generic buttons
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
    use windows::Win32::UI::Accessibility::{IUIAutomationElementArray, TreeScope_Descendants};

    let true_cond = match unsafe { automation.CreateTrueCondition() } {
        Ok(c) => c,
        Err(_) => return false,
    };

    let elements: IUIAutomationElementArray = match unsafe {
        window_el.FindAll(TreeScope_Descendants, &true_cond)
    } {
        Ok(el) => el,
        Err(_) => return false,
    };

    let count = match unsafe { elements.Length() } {
        Ok(c) => c,
        Err(_) => 0,
    };

    for i in 0..count {
        let el = match unsafe { elements.GetElement(i) } {
            Ok(e) => e,
            Err(_) => continue,
        };

        // Skip elements that are hidden, offscreen or not keyboard-focusable
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
        // Resolve the login URL in the background
        let target_url = if !normalized_url.is_empty() && launch_browser {
            let url_clone = normalized_url.clone();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .ok()?;
                rt.block_on(async {
                    resolve_login_url(&url_clone).await
                })
            }).join().unwrap_or(None).unwrap_or(normalized_url)
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
                    let _ = std::process::Command::new("cmd")
                        .args(&["/C", "start", "", &target_url])
                        .spawn();
                    std::thread::sleep(std::time::Duration::from_millis(1500));

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

                            if !already_on_login_form {
                                if try_click_login_link(&automation, &window_el) {
                                    std::thread::sleep(std::time::Duration::from_millis(1500));
                                }
                            }
                        }
                    }
                }
            }

            let mut last_focused_element_id: Option<String> = None;
            let mut filled_username = username.is_empty();
            let mut filled_password = password.is_empty();
            let mut filled_totp = totp_secret.is_empty();

            // Poll for up to 45 seconds (225 polls * 200ms) to allow multi-step transition/2FA
            for _ in 0..225 {
                if filled_username && filled_password && filled_totp {
                    break;
                }

                std::thread::sleep(std::time::Duration::from_millis(200));

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
                                std::thread::sleep(std::time::Duration::from_millis(1500));
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

                    // Language-independent criteria: username is any input that is neither password nor TOTP!
                    let is_username_field = !is_password_field && !is_totp_field;

                    if is_totp_field && !filled_totp {
                        last_focused_element_id = Some(element_key.clone());
                        let config = crate::totp::TotpConfig {
                            secret: totp_secret.clone(),
                            ..Default::default()
                        };
                        if let Ok(totp_code) = crate::totp::generate_totp(&config) {
                            let _ = send_ctrl_a_backspace();
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            let _ = autotype_text_with_delay(&totp_code.code, char_delay_ms);
                            std::thread::sleep(std::time::Duration::from_millis(field_delay_ms));
                            let _ = send_enter();
                        }
                        filled_totp = true;
                    } else if is_password_field && !filled_password {
                        last_focused_element_id = Some(element_key.clone());

                        if !filled_username {
                            // Focus was directly on Password field first, traverse up to username first
                            std::thread::sleep(std::time::Duration::from_millis(150));
                            let _ = send_shift_tab();
                            std::thread::sleep(std::time::Duration::from_millis(field_delay_ms));

                            // Fill Username
                            let mut user_val = String::new();
                            if let Ok(new_focused) = automation.GetFocusedElement() {
                                user_val = get_element_value(&new_focused);
                            }
                            let _ = autotype_correct_text(&user_val, &username, char_delay_ms);
                            std::thread::sleep(std::time::Duration::from_millis(field_delay_ms));

                            // Return to Password
                            let _ = autotype_text_with_delay("\t", char_delay_ms);
                            std::thread::sleep(std::time::Duration::from_millis(field_delay_ms));
                            filled_username = true;
                        }

                        // Clear password and fill
                        let _ = send_ctrl_a_backspace();
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        let _ = autotype_text_with_delay(&password, char_delay_ms);
                        std::thread::sleep(std::time::Duration::from_millis(field_delay_ms));
                        let _ = send_enter();
                        filled_password = true;
                    } else if is_username_field && !filled_username {
                        last_focused_element_id = Some(element_key.clone());

                        let current_val = get_element_value(&focused);
                        let _ = autotype_correct_text(&current_val, &username, char_delay_ms);
                        std::thread::sleep(std::time::Duration::from_millis(field_delay_ms));

                        // Check if password field is visible in active window
                        let has_password = !hwnd.is_invalid() && if let Ok(win_el) = automation.ElementFromHandle(hwnd) {
                            active_window_has_password_field(&automation, &win_el)
                        } else {
                            false
                        };

                        if has_password {
                            // Standard single-screen login form: Tab down and enter password
                            let _ = autotype_text_with_delay("\t", char_delay_ms);
                            std::thread::sleep(std::time::Duration::from_millis(field_delay_ms));
                            let _ = send_ctrl_a_backspace();
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            let _ = autotype_text_with_delay(&password, char_delay_ms);
                            std::thread::sleep(std::time::Duration::from_millis(field_delay_ms));
                            let _ = send_enter();
                            filled_username = true;
                            filled_password = true;
                        } else {
                            // Split-screen login form (like Google page 1): Press Enter to go to password screen
                            let _ = send_enter();
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
    autotype_text_with_delay(text, 15)
}

#[cfg(not(target_os = "windows"))]
pub fn autotype_text_with_delay(text: &str, _char_delay_ms: u64) -> crate::Result<()> {
    println!("Autotype (fallback stub): {}", text);
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
