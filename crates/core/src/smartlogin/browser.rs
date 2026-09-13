//! Browser Management — dynamic Chromium browser detection, profile resolution,
//! process management, and CDP-enabled launch.
//!
//! Discovery strategy (no hardcoding):
//! 1. Scan Windows registry `StartMenuInternet` for installed browsers
//! 2. Read `UserChoice` ProgId for the user's default
//! 3. Resolve executable paths from registry entries
//! 4. Scan common installation paths as fallback
//! 5. Detect each browser's user profile directory
//! 6. Check if the browser process is currently running

use chromiumoxide::browser::Browser;
use std::path::PathBuf;
use std::sync::Arc;

use crate::smartlogin::types::*;
use crate::smartlogin::logging::SmartLoginLogger;

/// Chromium-based browser info discovered on the system.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BrowserInfo {
    /// Display name (e.g., "Brave Browser", "Google Chrome")
    pub name: String,
    /// Executable path
    pub exe_path: PathBuf,
    /// User data directory (contains the default profile)
    pub profile_dir: PathBuf,
    /// Process name for detection (e.g., "brave.exe")
    pub process_name: String,
    /// Whether this is the system default browser
    pub is_default: bool,
    /// Whether the browser is currently running
    pub is_running: bool,
}

/// Result of pre-launch checks.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PreCheckResult {
    /// All detected Chromium browsers
    pub browsers: Vec<BrowserInfo>,
    /// Index of the recommended browser (default or first available)
    pub recommended_index: Option<usize>,
    /// Whether the recommended browser is currently running
    pub needs_close: bool,
    /// Error message if no browsers found
    pub error: Option<String>,
}

/// Session handle returned after successful browser connection.
pub struct BrowserSession {
    pub browser: Arc<Browser>,
    _handler_task: tokio::task::JoinHandle<()>,
}

impl BrowserSession {
    /// Disconnect CDP cleanly — aborts the event handler, closes the WebSocket,
    /// and lets the browser resume normal operation.
    pub fn disconnect(self) {
        self._handler_task.abort();
        // Arc<Browser> drops here — WebSocket connection closes
    }
}

impl Drop for BrowserSession {
    fn drop(&mut self) {
        // Abort the handler to prevent orphaned CDP connections
        self._handler_task.abort();
    }
}

// ─── Discovery ──────────────────────────────────────────────────────────

/// Discover all installed Chromium-based browsers on the system.
pub fn discover_browsers() -> Vec<BrowserInfo> {
    let mut browsers = Vec::new();
    let default_progid = get_default_progid();

    // Strategy 1: Scan registry
    #[cfg(windows)]
    {
        scan_registry_browsers(&mut browsers, &default_progid);
    }

    // Strategy 2: Scan common paths as fallback
    scan_common_paths(&mut browsers, &default_progid);

    // Deduplicate by executable path (normalized)
    browsers.sort_by(|a, b| a.exe_path.cmp(&b.exe_path));
    browsers.dedup_by(|a, b| normalize_path(&a.exe_path) == normalize_path(&b.exe_path));

    // Check which browsers are currently running using a single tasklist call
    let running_processes = get_running_process_list();
    for browser in &mut browsers {
        let p_name = browser.process_name.to_lowercase();
        browser.is_running = running_processes.iter().any(|p| p.to_lowercase() == p_name);
    }

    browsers
}

/// Run pre-launch checks: find browsers, pick the best one.
pub fn precheck() -> PreCheckResult {
    let browsers = discover_browsers();

    if browsers.is_empty() {
        return PreCheckResult {
            browsers,
            recommended_index: None,
            needs_close: false,
            error: Some("No Chromium-based browser found on this system".into()),
        };
    }

    // Pick the best browser with the following priority:
    // 1. Non-Edge browser marked as default (rare but correct)
    // 2. Non-Edge browser (any) — Windows often falsely reports Edge as default
    // 3. Edge if it's the only option
    let recommended_index = browsers
        .iter()
        .position(|b| b.is_default && !is_edge(b))
        .or_else(|| browsers.iter().position(|b| !is_edge(b)))
        .or(Some(0));

    let needs_close = recommended_index
        .map(|i| browsers[i].is_running)
        .unwrap_or(false);

    PreCheckResult {
        browsers,
        recommended_index,
        needs_close,
        error: None,
    }
}

/// Check if a BrowserInfo is Microsoft Edge.
fn is_edge(b: &BrowserInfo) -> bool {
    let name = b.name.to_lowercase();
    let exe = b.process_name.to_lowercase();
    name.contains("edge") || exe.contains("msedge")
}

// ─── Browser Launch ─────────────────────────────────────────────────────

/// Launch a browser with CDP and connect to it.
/// If the browser is already running with CDP enabled, opens a new tab instead.
pub async fn launch_and_connect(
    url: &str,
    browser_info: &BrowserInfo,
    config: &SmartLoginConfig,
    logger: &SmartLoginLogger,
) -> crate::Result<(BrowserSession, chromiumoxide::Page)> {
    let port = config.cdp_port;

    // Try connecting to an already-running browser with CDP enabled
    if browser_info.is_running {
        if let Ok(result) = try_connect_existing(url, port, logger).await {
            return Ok(result);
        }
        // CDP not available — close and relaunch
        logger.log(LoginState::LaunchingBrowser, format!(
            "Closing {}...", browser_info.name
        ));
        let _ = close_browser(&browser_info.process_name);
    }

    // Fresh launch
    logger.log(LoginState::LaunchingBrowser, format!(
        "Launching {} with CDP...", browser_info.name
    ));

    // Clean up stale lock files if left over from taskkill
    for lock_name in &["SingletonLock", "lockfile", "SingletonCookie", "SingletonSocket"] {
        let lock_path = browser_info.profile_dir.join(lock_name);
        if lock_path.exists() {
            let _ = std::fs::remove_file(&lock_path);
        }
    }

    let exe = browser_info.exe_path.to_string_lossy().to_string();
    let profile = browser_info.profile_dir.to_string_lossy().to_string();

    let mut args = vec![
        format!("--remote-debugging-port={port}"),
        format!("--user-data-dir={profile}"),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
        "--disable-background-networking".to_string(),
        "--disable-extensions".to_string(),
        "--disable-component-update".to_string(),
    ];

    args.push(url.to_string());

    let mut child = spawn_browser_process(&exe, &args, browser_info, logger)?;

    let ws_url = wait_for_cdp_endpoint(&mut child, port, config.page_load_timeout_secs * 1000).await?;
    connect_and_get_page(url, &ws_url, config, logger).await
}

/// Try connecting to an already-running browser via its CDP endpoint.
/// Opens a new tab for the URL if successful.
async fn try_connect_existing(
    url: &str,
    port: u16,
    logger: &SmartLoginLogger,
) -> crate::Result<(BrowserSession, chromiumoxide::Page)> {
    logger.log(LoginState::LaunchingBrowser, "Browser already running — trying to connect...");

    // Check if CDP endpoint is available
    let version_url = format!("http://127.0.0.1:{port}/json/version");
    let response = reqwest::get(&version_url).await.map_err(|e| {
        crate::error::VaultError::SmartLoginError(format!("No CDP endpoint: {e}"))
    })?;

    let text = response.text().await.map_err(|e| {
        crate::error::VaultError::SmartLoginError(format!("Invalid CDP response: {e}"))
    })?;

    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        crate::error::VaultError::SmartLoginError(format!("Invalid CDP JSON: {e}"))
    })?;

    let ws_url = json["webSocketDebuggerUrl"]
        .as_str()
        .ok_or_else(|| crate::error::VaultError::SmartLoginError(
            "No WebSocket URL in CDP response".into()
        ))?
        .to_string();

    logger.log(LoginState::LaunchingBrowser, "Connecting to existing browser via CDP...");

    let (browser, mut handler) = Browser::connect(&ws_url).await.map_err(|e| {
        crate::error::VaultError::SmartLoginError(format!("CDP connect failed: {e}"))
    })?;

    let handler_task = tokio::spawn(async move {
        use futures::StreamExt;
        while let Some(event) = handler.next().await {
            if event.is_err() { break; }
        }
    });

    let browser = Arc::new(browser);

    // Reuse a blank tab if available, otherwise open a new tab
    let pages = browser.pages().await.unwrap_or_default();
    let mut page_opt = None;
    for p in &pages {
        let p_url = p.url().await.ok().flatten().unwrap_or_default();
        if p_url == "about:blank" || p_url == "chrome://newtab/" || p_url.is_empty() {
            page_opt = Some(p.clone());
            break;
        }
    }

    let page = match page_opt {
        Some(p) => {
            let _ = p.goto(url).await;
            p
        }
        None => browser.new_page(url).await.map_err(|e| {
            crate::error::VaultError::SmartLoginError(format!("Failed to open new tab: {e}"))
        })?,
    };

    logger.log(LoginState::NavigatingToUrl, format!("Connected (existing browser). Page: {url}"));

    let session = BrowserSession {
        browser,
        _handler_task: handler_task,
    };

    Ok((session, page))
}

/// Connect to a CDP WebSocket URL, get or create the target page.
async fn connect_and_get_page(
    url: &str,
    ws_url: &str,
    config: &SmartLoginConfig,
    logger: &SmartLoginLogger,
) -> crate::Result<(BrowserSession, chromiumoxide::Page)> {
    logger.log(LoginState::LaunchingBrowser, "Connecting to CDP...");

    let (browser, mut handler) = Browser::connect(ws_url).await.map_err(|e| {
        crate::error::VaultError::SmartLoginError(format!("CDP connection failed: {e}"))
    })?;

    let handler_task = tokio::spawn(async move {
        use futures::StreamExt;
        while let Some(event) = handler.next().await {
            if event.is_err() { break; }
        }
    });

    let browser = Arc::new(browser);

    // Brief wait for the initial page to register
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let pages = browser.pages().await.map_err(|e| {
        crate::error::VaultError::SmartLoginError(format!("Failed to get pages: {e}"))
    })?;

    // Close any extra blank tabs to leave only 1 clean tab
    if pages.len() > 1 {
        for p in &pages[..pages.len() - 1] {
            let p_url = p.url().await.ok().flatten().unwrap_or_default();
            if p_url == "about:blank" || p_url == "chrome://newtab/" || p_url.is_empty() {
                let _ = p.clone().close().await;
            }
        }
    }

    // Pick the primary tab
    let page = if let Some(p) = pages.into_iter().last() {
        p
    } else {
        browser.new_page(url).await.map_err(|e| {
            crate::error::VaultError::SmartLoginError(format!("Failed to create page: {e}"))
        })?
    };

    // Ensure the page is at target URL
    let current_url = page.url().await.ok().flatten().unwrap_or_default();
    if current_url.is_empty() || current_url == "about:blank" || current_url == "chrome://newtab/" {
        let _ = page.goto(url).await;
    }

    logger.log(LoginState::NavigatingToUrl, format!("Connected. Page: {url}"));

    // Wait for initial page load
    let timeout = tokio::time::Duration::from_secs(config.page_load_timeout_secs);
    tokio::time::timeout(timeout, async {
        loop {
            let ready_js = r#"document.readyState === 'complete' || document.readyState === 'interactive'"#;
            if let Ok(result) = page.evaluate(ready_js).await {
                if let Ok(ready) = result.into_value::<bool>() {
                    if ready { break; }
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        }
    })
    .await
    .ok();

    let _ = page
        .evaluate(
            r#"
            (() => {
                try {
                    Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
                } catch(e) {}
            })()
            "#,
        )
        .await;

    let session = BrowserSession {
        browser,
        _handler_task: handler_task,
    };

    Ok((session, page))
}

// ─── Process Management ─────────────────────────────────────────────────

/// Check if current process has Administrator privileges on Windows.
#[cfg(windows)]
pub fn is_elevated() -> bool {
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn IsUserAnAdmin() -> i32;
    }
    unsafe { IsUserAnAdmin() != 0 }
}

#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    false
}

/// Close a browser gracefully by process name.
pub fn close_browser(process_name: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut kill_cmd = std::process::Command::new("taskkill");
        kill_cmd.args(["/IM", process_name, "/F"]);
        kill_cmd.creation_flags(0x08000000);
        let output = kill_cmd.output()
            .map_err(|e| format!("Failed to run taskkill: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // "not found" is OK — means it's already closed
            if !stderr.contains("not found") && !stderr.contains("not running") {
                return Err(format!("taskkill failed: {}", stderr.trim()));
            }
        }

        // Poll for up to 3500ms for all browser processes to fully exit
        for _ in 0..35 {
            if !is_process_running(process_name) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        // Additional delay to allow Windows to release file handles on the user profile
        std::thread::sleep(std::time::Duration::from_millis(300));
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let output = std::process::Command::new("pkill")
            .args(["-f", process_name])
            .output()
            .map_err(|e| format!("Failed to run pkill: {e}"))?;

        std::thread::sleep(std::time::Duration::from_millis(200));
        Ok(())
    }
}

/// Fetch a single list of all running process names to avoid redundant subprocess calls.
fn get_running_process_list() -> Vec<String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new("tasklist");
        cmd.args(["/NH", "/FO", "CSV"]);
        cmd.creation_flags(0x08000000);
        let output = cmd.output();

        match output {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                stdout
                    .lines()
                    .filter_map(|line| {
                        let parts: Vec<&str> = line.split(',').collect();
                        if !parts.is_empty() {
                            Some(parts[0].trim_matches('"').to_string())
                        } else {
                            None
                        }
                    })
                    .collect()
            }
            Err(_) => Vec::new(),
        }
    }

    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

/// Check if a process is running by name.
fn is_process_running(process_name: &str) -> bool {
    let running = get_running_process_list();
    let p_lower = process_name.to_lowercase();
    running.iter().any(|p| p.to_lowercase() == p_lower)
}

/// Browser process handle, either a direct spawned process or a de-elevated process.
enum BrowserChild {
    Process(std::process::Child),
    #[allow(dead_code)]
    DeElevated { pid: u32 },
}

impl BrowserChild {
    fn check_exited(&mut self) -> Option<String> {
        match self {
            BrowserChild::Process(c) => {
                if let Ok(Some(status)) = c.try_wait() {
                    Some(format!("{status}"))
                } else {
                    None
                }
            }
            BrowserChild::DeElevated { pid } => {
                #[cfg(windows)]
                {
                    if !unsafe { win32_deelevate::is_pid_alive(*pid) } {
                        return Some("process terminated".into());
                    }
                    None
                }
                #[cfg(not(windows))]
                {
                    let _ = pid;
                    None
                }
            }
        }
    }
}

#[cfg(windows)]
mod win32_deelevate {
    use std::ptr;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    #[repr(C)]
    struct StartupInfoW {
        cb: u32,
        lp_reserved: *mut u16,
        lp_desktop: *mut u16,
        lp_title: *mut u16,
        dw_x: u32,
        dw_y: u32,
        dw_x_size: u32,
        dw_y_size: u32,
        dw_x_count_chars: u32,
        dw_y_count_chars: u32,
        dw_fill_attribute: u32,
        dw_flags: u32,
        w_show_window: u16,
        cb_reserved2: u16,
        lp_reserved2: *mut u8,
        h_std_input: *mut std::ffi::c_void,
        h_std_output: *mut std::ffi::c_void,
        h_std_error: *mut std::ffi::c_void,
    }

    #[repr(C)]
    struct ProcessInformation {
        h_process: *mut std::ffi::c_void,
        h_thread: *mut std::ffi::c_void,
        dw_process_id: u32,
        dw_thread_id: u32,
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetShellWindow() -> *mut std::ffi::c_void;
        fn GetWindowThreadProcessId(hwnd: *mut std::ffi::c_void, lpdw_process_id: *mut u32) -> u32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(dw_desired_access: u32, b_inherit_handle: i32, dw_process_id: u32) -> *mut std::ffi::c_void;
        fn GetExitCodeProcess(h_process: *mut std::ffi::c_void, lp_exit_code: *mut u32) -> i32;
        fn CloseHandle(h_object: *mut std::ffi::c_void) -> i32;
    }

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn OpenProcessToken(process_handle: *mut std::ffi::c_void, desired_access: u32, token_handle: *mut *mut std::ffi::c_void) -> i32;
        fn DuplicateTokenEx(
            h_existing_token: *mut std::ffi::c_void,
            dw_desired_access: u32,
            lp_token_attributes: *mut std::ffi::c_void,
            impersonation_level: i32,
            token_type: i32,
            ph_new_token: *mut *mut std::ffi::c_void,
        ) -> i32;
        fn CreateProcessWithTokenW(
            h_token: *mut std::ffi::c_void,
            dw_logon_flags: u32,
            lp_application_name: *const u16,
            lp_command_line: *mut u16,
            dw_creation_flags: u32,
            lp_environment: *mut std::ffi::c_void,
            lp_current_directory: *const u16,
            lp_startup_info: *const StartupInfoW,
            lp_process_information: *mut ProcessInformation,
        ) -> i32;
    }

    pub unsafe fn is_pid_alive(pid: u32) -> bool {
        const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
        const STILL_ACTIVE: u32 = 259;

        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return false;
            }
            let mut exit_code: u32 = 0;
            let res = GetExitCodeProcess(handle, &mut exit_code);
            CloseHandle(handle);

            res != 0 && exit_code == STILL_ACTIVE
        }
    }

    pub fn spawn_de_elevated(
        exe: &str,
        args: &[String],
        cwd: &std::path::Path,
    ) -> Result<u32, String> {
        unsafe {
            let shell_wnd = GetShellWindow();
            if shell_wnd.is_null() {
                return Err("Explorer shell window not found".into());
            }

            let mut explorer_pid: u32 = 0;
            GetWindowThreadProcessId(shell_wnd, &mut explorer_pid);
            if explorer_pid == 0 {
                return Err("Failed to get Explorer process ID".into());
            }

            const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
            let proc_handle = OpenProcess(PROCESS_QUERY_INFORMATION, 0, explorer_pid);
            if proc_handle.is_null() {
                return Err("Failed to open Explorer process".into());
            }

            const TOKEN_DUPLICATE: u32 = 0x0002;
            const TOKEN_QUERY: u32 = 0x0008;
            const TOKEN_ASSIGN_PRIMARY: u32 = 0x0001;
            let mut token_handle = ptr::null_mut();
            let token_res = OpenProcessToken(
                proc_handle,
                TOKEN_DUPLICATE | TOKEN_QUERY | TOKEN_ASSIGN_PRIMARY,
                &mut token_handle,
            );
            CloseHandle(proc_handle);

            if token_res == 0 || token_handle.is_null() {
                return Err("Failed to open Explorer token".into());
            }

            const MAXIMUM_ALLOWED: u32 = 0x02000000;
            const SECURITY_IMPERSONATION: i32 = 2;
            const TOKEN_PRIMARY: i32 = 1;
            let mut primary_token = ptr::null_mut();
            let dup_res = DuplicateTokenEx(
                token_handle,
                MAXIMUM_ALLOWED,
                ptr::null_mut(),
                SECURITY_IMPERSONATION,
                TOKEN_PRIMARY,
                &mut primary_token,
            );
            CloseHandle(token_handle);

            if dup_res == 0 || primary_token.is_null() {
                return Err("Failed to duplicate primary token".into());
            }

            let mut cmd_line_str = format!("\"{}\"", exe);
            for arg in args {
                cmd_line_str.push(' ');
                if arg.contains(' ') || arg.contains('\t') {
                    cmd_line_str.push('"');
                    cmd_line_str.push_str(arg);
                    cmd_line_str.push('"');
                } else {
                    cmd_line_str.push_str(arg);
                }
            }

            let mut cmd_line_wide: Vec<u16> = OsStr::new(&cmd_line_str)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            let cwd_wide: Vec<u16> = OsStr::new(cwd.as_os_str())
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            let mut si: StartupInfoW = std::mem::zeroed();
            si.cb = std::mem::size_of::<StartupInfoW>() as u32;

            let mut pi: ProcessInformation = std::mem::zeroed();

            let create_res = CreateProcessWithTokenW(
                primary_token,
                0,
                ptr::null(),
                cmd_line_wide.as_mut_ptr(),
                0,
                ptr::null_mut(),
                cwd_wide.as_ptr(),
                &si,
                &mut pi,
            );
            CloseHandle(primary_token);

            if create_res == 0 {
                return Err("CreateProcessWithTokenW failed".into());
            }

            if !pi.h_thread.is_null() {
                CloseHandle(pi.h_thread);
            }
            if !pi.h_process.is_null() {
                CloseHandle(pi.h_process);
            }

            Ok(pi.dw_process_id)
        }
    }
}

fn spawn_browser_process(
    exe: &str,
    args: &[String],
    browser_info: &BrowserInfo,
    logger: &SmartLoginLogger,
) -> crate::Result<BrowserChild> {
    #[cfg(windows)]
    if is_elevated() {
        logger.log(LoginState::LaunchingBrowser, "Elevated execution detected — de-elevating browser to interactive desktop session...");
        let cwd = browser_info.exe_path.parent().unwrap_or(&browser_info.profile_dir);
        match win32_deelevate::spawn_de_elevated(exe, args, cwd) {
            Ok(pid) => {
                logger.log(LoginState::LaunchingBrowser, format!("De-elevated browser PID: {pid}. Waiting for CDP endpoint..."));
                return Ok(BrowserChild::DeElevated { pid });
            }
            Err(e) => {
                logger.log(LoginState::LaunchingBrowser, format!("De-elevation fallback ({e}) — launching directly with sandbox compatibility flags..."));
                let mut elevated_args = args.to_vec();
                elevated_args.push("--no-sandbox".to_string());
                elevated_args.push("--disable-gpu-sandbox".to_string());
                elevated_args.push("--test-type".to_string());
                let mut cmd = std::process::Command::new(exe);
                cmd.args(&elevated_args);
                if let Some(parent) = browser_info.exe_path.parent() {
                    cmd.current_dir(parent);
                }
                cmd.stdin(std::process::Stdio::null());
                cmd.stdout(std::process::Stdio::null());
                cmd.stderr(std::process::Stdio::null());
                let c = cmd.spawn().map_err(|err| {
                    crate::error::VaultError::SmartLoginError(format!(
                        "Failed to launch {}: {err}", browser_info.name
                    ))
                })?;
                logger.log(LoginState::LaunchingBrowser, format!("Browser PID: {}. Waiting for CDP endpoint...", c.id()));
                return Ok(BrowserChild::Process(c));
            }
        }
    }

    let mut cmd = std::process::Command::new(exe);
    cmd.args(args);
    if let Some(parent) = browser_info.exe_path.parent() {
        cmd.current_dir(parent);
    }
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    let c = cmd.spawn().map_err(|e| {
        crate::error::VaultError::SmartLoginError(format!(
            "Failed to launch {}: {e}", browser_info.name
        ))
    })?;
    logger.log(LoginState::LaunchingBrowser, format!("Browser PID: {}. Waiting for CDP endpoint...", c.id()));
    Ok(BrowserChild::Process(c))
}

// ─── CDP Endpoint Discovery ─────────────────────────────────────────────

/// Poll the CDP debug endpoint until it returns the WebSocket URL.
async fn wait_for_cdp_endpoint(
    child: &mut BrowserChild,
    port: u16,
    timeout_ms: u64,
) -> crate::Result<String> {
    let endpoint = format!("http://127.0.0.1:{port}/json/version");
    let deadline = tokio::time::Instant::now()
        + tokio::time::Duration::from_millis(timeout_ms);

    loop {
        // Fast-fail if browser process exited on startup
        if let Some(status) = child.check_exited() {
            return Err(crate::error::VaultError::SmartLoginError(format!(
                "Browser process exited prematurely with status: {status}. Check if another browser instance or profile lock is active."
            )));
        }

        if tokio::time::Instant::now() > deadline {
            return Err(crate::error::VaultError::SmartLoginError(
                "Timeout waiting for CDP endpoint. Browser may have failed to start.".into(),
            ));
        }

        match reqwest::get(&endpoint).await {
            Ok(resp) => {
                if let Ok(text) = resp.text().await {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(ws) = json.get("webSocketDebuggerUrl").and_then(|v| v.as_str()) {
                            return Ok(ws.to_string());
                        }
                    }
                }
            }
            Err(_) => {
                // CDP not ready yet, retry
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
    }
}

// ─── Windows Registry Scanner ───────────────────────────────────────────

#[cfg(windows)]
fn get_default_progid() -> Option<String> {
    use winreg::enums::*;
    use winreg::RegKey;

    // Read the user's default browser ProgId from UserChoice
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey(
        r"Software\Microsoft\Windows\Shell\Associations\UrlAssociations\https\UserChoice"
    ).ok()?;
    let progid: String = key.get_value("ProgID").ok()?;
    Some(progid)
}

#[cfg(not(windows))]
fn get_default_progid() -> Option<String> {
    None
}

#[cfg(windows)]
fn scan_registry_browsers(browsers: &mut Vec<BrowserInfo>, default_progid: &Option<String>) {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    // Scan HKLM\SOFTWARE\Clients\StartMenuInternet
    if let Ok(clients) = hklm.open_subkey(r"SOFTWARE\Clients\StartMenuInternet") {
        for name_result in clients.enum_keys() {
            if let Ok(name) = name_result {
                if let Some(info) = parse_registry_browser(&clients, &name, default_progid) {
                    // Only include Chromium-based browsers
                    if is_chromium_based(&info) {
                        browsers.push(info);
                    }
                }
            }
        }
    }

    // Also scan HKCU (user-installed browsers like Brave)
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(clients) = hkcu.open_subkey(r"SOFTWARE\Clients\StartMenuInternet") {
        for name_result in clients.enum_keys() {
            if let Ok(name) = name_result {
                if let Some(info) = parse_registry_browser(&clients, &name, default_progid) {
                    if is_chromium_based(&info) {
                        browsers.push(info);
                    }
                }
            }
        }
    }
}

#[cfg(windows)]
fn parse_registry_browser(
    clients: &winreg::RegKey,
    name: &str,
    default_progid: &Option<String>,
) -> Option<BrowserInfo> {
    let subkey = clients.open_subkey(name).ok()?;

    // Get display name
    let display_name: String = subkey.get_value("").ok().unwrap_or_else(|| name.to_string());

    // Get executable path from shell\open\command
    let cmd_key = subkey.open_subkey(r"shell\open\command").ok()?;
    let cmd_value: String = cmd_key.get_value("").ok()?;

    // Parse the executable path (may be quoted or have arguments)
    let exe_path = parse_exe_from_command(&cmd_value)?;

    if !exe_path.exists() {
        return None;
    }

    // Determine process name from the exe path
    let process_name = exe_path.file_name()?.to_string_lossy().to_string();

    // Determine profile directory
    let profile_dir = detect_profile_dir(&exe_path, &display_name);

    // Check if this is the default browser by matching ProgId
    let is_default = if let Some(progid) = default_progid {
        // Read the browser's ProgId from its Capabilities
        let cap_key = subkey.open_subkey(r"Capabilities\URLAssociations").ok();
        if let Some(cap) = cap_key {
            let browser_progid: Result<String, _> = cap.get_value("https");
            browser_progid.map(|p| &p == progid).unwrap_or(false)
        } else {
            // Fallback: match by name patterns
            match_progid_to_name(progid, &display_name)
        }
    } else {
        false
    };

    Some(BrowserInfo {
        name: display_name,
        exe_path,
        profile_dir,
        process_name,
        is_default,
        is_running: false, // set later
    })
}

/// Parse an executable path from a registry command string.
/// Handles: "C:\path\to\browser.exe" --arg  or  C:\path\to\browser.exe
fn parse_exe_from_command(cmd: &str) -> Option<PathBuf> {
    let trimmed = cmd.trim();
    if trimmed.starts_with('"') {
        // Quoted path
        let end = trimmed[1..].find('"')?;
        Some(PathBuf::from(&trimmed[1..=end]))
    } else {
        // Unquoted — take until first space or end
        let end = trimmed.find(' ').unwrap_or(trimmed.len());
        Some(PathBuf::from(&trimmed[..end]))
    }
}

/// Detect the user data directory for a Chromium browser from its exe path.
fn detect_profile_dir(exe_path: &PathBuf, name: &str) -> PathBuf {
    let local_appdata = std::env::var("LOCALAPPDATA")
        .unwrap_or_else(|_| r"C:\Users\Default\AppData\Local".into());

    let _name_lower = name.to_lowercase();
    let exe_str = exe_path.to_string_lossy().to_lowercase();

    // Detect by exe path components (most reliable)
    if exe_str.contains("brave") {
        return PathBuf::from(&local_appdata)
            .join("BraveSoftware")
            .join("Brave-Browser")
            .join("User Data");
    }
    if exe_str.contains("vivaldi") {
        return PathBuf::from(&local_appdata)
            .join("Vivaldi")
            .join("User Data");
    }
    if exe_str.contains("opera") {
        // Opera uses a different structure
        return PathBuf::from(&local_appdata)
            .join("Opera Software")
            .join("Opera Stable");
    }
    if exe_str.contains("msedge") || exe_str.contains("edge") {
        return PathBuf::from(&local_appdata)
            .join("Microsoft")
            .join("Edge")
            .join("User Data");
    }
    if exe_str.contains("chrome") {
        return PathBuf::from(&local_appdata)
            .join("Google")
            .join("Chrome")
            .join("User Data");
    }
    if exe_str.contains("chromium") {
        return PathBuf::from(&local_appdata)
            .join("Chromium")
            .join("User Data");
    }

    // Fallback: try to derive from the exe's own directory
    // Many Chromium browsers put "User Data" next to the Application folder
    if let Some(app_dir) = exe_path.parent() {
        if let Some(parent) = app_dir.parent() {
            let user_data = parent.join("User Data");
            if user_data.exists() {
                return user_data;
            }
        }
    }

    // Final fallback: use Chrome's default path
    PathBuf::from(&local_appdata)
        .join("Google")
        .join("Chrome")
        .join("User Data")
}

/// Check if a browser is Chromium-based by examining its executable path and name.
fn is_chromium_based(info: &BrowserInfo) -> bool {
    let name = info.name.to_lowercase();
    let exe = info.exe_path.to_string_lossy().to_lowercase();

    let chromium_indicators = [
        "chrome", "chromium", "brave", "edge", "msedge", "vivaldi",
        "opera", "arc", "sidekick", "thorium", "ungoogled",
    ];

    chromium_indicators.iter().any(|ind| name.contains(ind) || exe.contains(ind))
}

/// Match a ProgId to a browser name for default detection fallback.
fn match_progid_to_name(progid: &str, name: &str) -> bool {
    let progid_lower = progid.to_lowercase();
    let name_lower = name.to_lowercase();

    if progid_lower.contains("chrome") && name_lower.contains("chrome") { return true; }
    if progid_lower.contains("brave") && name_lower.contains("brave") { return true; }
    if progid_lower.contains("edge") && name_lower.contains("edge") { return true; }
    if progid_lower.contains("vivaldi") && name_lower.contains("vivaldi") { return true; }
    if progid_lower.contains("opera") && name_lower.contains("opera") { return true; }

    false
}

// ─── Fallback: Common Path Scanner ──────────────────────────────────────

fn scan_common_paths(browsers: &mut Vec<BrowserInfo>, default_progid: &Option<String>) {
    let program_files = std::env::var("PROGRAMFILES")
        .unwrap_or_else(|_| r"C:\Program Files".into());
    let program_files_x86 = std::env::var("PROGRAMFILES(X86)")
        .unwrap_or_else(|_| r"C:\Program Files (x86)".into());
    let local_appdata = std::env::var("LOCALAPPDATA")
        .unwrap_or_else(|_| r"C:\Users\Default\AppData\Local".into());

    // Known Chromium browser locations (exe relative paths)
    let candidates: Vec<(&str, Vec<PathBuf>)> = vec![
        ("Google Chrome", vec![
            PathBuf::from(&program_files).join(r"Google\Chrome\Application\chrome.exe"),
            PathBuf::from(&program_files_x86).join(r"Google\Chrome\Application\chrome.exe"),
            PathBuf::from(&local_appdata).join(r"Google\Chrome\Application\chrome.exe"),
        ]),
        ("Brave Browser", vec![
            PathBuf::from(&program_files).join(r"BraveSoftware\Brave-Browser\Application\brave.exe"),
            PathBuf::from(&local_appdata).join(r"BraveSoftware\Brave-Browser\Application\brave.exe"),
        ]),
        ("Microsoft Edge", vec![
            PathBuf::from(&program_files).join(r"Microsoft\Edge\Application\msedge.exe"),
            PathBuf::from(&program_files_x86).join(r"Microsoft\Edge\Application\msedge.exe"),
        ]),
        ("Vivaldi", vec![
            PathBuf::from(&local_appdata).join(r"Vivaldi\Application\vivaldi.exe"),
            PathBuf::from(&program_files).join(r"Vivaldi\Application\vivaldi.exe"),
        ]),
    ];

    let existing_exes: Vec<PathBuf> = browsers.iter()
        .map(|b| normalize_path(&b.exe_path))
        .collect();

    for (name, paths) in candidates {
        for path in paths {
            if path.exists() && !existing_exes.contains(&normalize_path(&path)) {
                let process_name = path.file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_default();

                let is_default = default_progid.as_ref().map(|pid| {
                    match_progid_to_name(pid, name)
                }).unwrap_or(false);

                browsers.push(BrowserInfo {
                    name: name.to_string(),
                    exe_path: path.clone(),
                    profile_dir: detect_profile_dir(&path, name),
                    process_name,
                    is_default,
                    is_running: false,
                });
                break; // Found one path for this browser, skip others
            }
        }
    }
}

fn normalize_path(path: &PathBuf) -> PathBuf {
    path.to_string_lossy().to_lowercase().into()
}
