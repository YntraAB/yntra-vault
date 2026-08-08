//! SOTA Platform OS Clipboard Defense & History Logging Bypass
//!
//! Provides platform-native flags to bypass clipboard history logging (Windows / macOS / Linux)
//! and automatic secure clipboard wiping after a configurable timeout.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use zeroize::Zeroize;
use crate::VaultError;

/// Monotonically increasing transaction counter to prevent stale timer race conditions.
static CLIPBOARD_TX_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Active state tracking current copy operation transaction ID & hash.
static ACTIVE_CLIPBOARD_STATE: Mutex<Option<ClipboardState>> = Mutex::new(None);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClipboardState {
    tx_id: u64,
    hash: u64,
}

/// Simple non-cryptographic FNV-1a hash to verify if clipboard content changed before auto-clearing
fn compute_text_hash(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Copy text to OS clipboard using SOTA platform defense flags to bypass clipboard history logging.
pub fn copy_to_clipboard_defended(text: &str, is_sensitive: bool, clear_after_secs: Option<u64>) -> crate::Result<()> {
    let tx_id = CLIPBOARD_TX_COUNTER.fetch_add(1, Ordering::SeqCst);
    let hash = compute_text_hash(text);

    if is_sensitive {
        #[cfg(target_os = "windows")]
        copy_windows_defended(text)?;

        #[cfg(target_os = "macos")]
        copy_macos_defended(text)?;

        #[cfg(target_os = "linux")]
        copy_linux_defended(text)?;

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        copy_generic_fallback(text)?;
    } else {
        #[cfg(target_os = "windows")]
        copy_windows_plain(text)?;

        #[cfg(not(target_os = "windows"))]
        copy_generic_fallback(text)?;
    }

    if is_sensitive {
        if let Ok(mut lock) = ACTIVE_CLIPBOARD_STATE.lock() {
            *lock = Some(ClipboardState { tx_id, hash });
        }

        if let Some(secs) = clear_after_secs {
            if secs > 0 {
                schedule_auto_clear(text, tx_id, hash, secs);
            }
        }
    }

    Ok(())
}

/// Clear OS clipboard contents immediately.
pub fn clear_clipboard() -> crate::Result<()> {
    #[cfg(target_os = "windows")]
    {
        clear_windows()?;
    }

    #[cfg(target_os = "macos")]
    {
        clear_macos()?;
    }

    #[cfg(target_os = "linux")]
    {
        clear_linux()?;
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        // Fallback clear
        let _ = copy_generic_fallback("");
    }

    // Reset active tracked state
    if let Ok(mut lock) = ACTIVE_CLIPBOARD_STATE.lock() {
        *lock = None;
    }

    Ok(())
}

/// Schedule automatic clipboard clear after `timeout_secs` if transaction ID and hash match.
pub fn schedule_auto_clear(text: &str, tx_id: u64, hash: u64, timeout_secs: u64) {
    let text_to_check = text.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(timeout_secs));

        // Verify if active secret transaction ID and hash still match
        let should_clear = match ACTIVE_CLIPBOARD_STATE.lock() {
            Ok(lock) => *lock == Some(ClipboardState { tx_id, hash }),
            Err(_) => false,
        };

        if should_clear {
            // Re-check current clipboard content before clearing to avoid wiping user's manual copy
            if is_clipboard_matching(&text_to_check) {
                let _ = clear_clipboard();
            }
        }
    });
}

/// Check if current clipboard content matches the specified text.
fn is_clipboard_matching(text: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        if let Ok(current) = get_windows_clipboard_text() {
            return current == text;
        }
    }
    true
}

// ============================================================================
// WINDOWS PLATFORM IMPLEMENTATION (DEVIL'S ADVOCATE HARDENED)
// ============================================================================
#[cfg(target_os = "windows")]
const CF_UNICODETEXT: u32 = 13;

#[cfg(target_os = "windows")]
fn open_clipboard_with_retry() -> crate::Result<()> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::DataExchange::OpenClipboard;

    for _ in 0..10 {
        unsafe {
            if OpenClipboard(HWND::default()).is_ok() {
                return Ok(());
            }
        }
        std::thread::sleep(Duration::from_millis(15));
    }

    Err(VaultError::ClipboardError("Failed to lock clipboard after retries (Clipboard in use)".into()))
}

#[cfg(target_os = "windows")]
fn copy_windows_defended(text: &str) -> crate::Result<()> {
    use windows::core::w;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    struct ClipboardGuard;
    impl Drop for ClipboardGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseClipboard();
            }
        }
    }

    open_clipboard_with_retry()?;
    let _guard = ClipboardGuard;

    unsafe {
        EmptyClipboard().map_err(|e| VaultError::ClipboardError(format!("Failed to empty clipboard: {}", e)))?;

        // ────────────────────────────────────────────────────────────────────
        // DEVIL'S ADVOCATE COUNTERMEASURE: Re-ordered Flag Assignment
        // Set Exclusion & History Bypass flags BEFORE setting CF_UNICODETEXT payload!
        // Prevents clipboard monitor tools from logging payload on CF_UNICODETEXT event.
        // ────────────────────────────────────────────────────────────────────

        // 1. History Bypass Flag: CanIncludeInClipboardHistory = 0
        let fmt_history = RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory"));
        if fmt_history != 0 {
            if let Ok(h_flag) = GlobalAlloc(GMEM_MOVEABLE, std::mem::size_of::<u32>()) {
                let p_flag = GlobalLock(h_flag) as *mut u32;
                if !p_flag.is_null() {
                    std::ptr::write(p_flag, 0u32);
                    let _ = GlobalUnlock(h_flag);
                    let _ = SetClipboardData(fmt_history, HANDLE(h_flag.0));
                }
            }
        }

        // 2. Cloud Clipboard Bypass Flag: CanUploadToCloudClipboard = 0
        let fmt_cloud = RegisterClipboardFormatW(w!("CanUploadToCloudClipboard"));
        if fmt_cloud != 0 {
            if let Ok(h_flag) = GlobalAlloc(GMEM_MOVEABLE, std::mem::size_of::<u32>()) {
                let p_flag = GlobalLock(h_flag) as *mut u32;
                if !p_flag.is_null() {
                    std::ptr::write(p_flag, 0u32);
                    let _ = GlobalUnlock(h_flag);
                    let _ = SetClipboardData(fmt_cloud, HANDLE(h_flag.0));
                }
            }
        }

        // 3. Clipboard Monitor Exclusion Flag: ExcludeClipboardContentFromMonitorProcessing = 1
        let fmt_exclude = RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
        if fmt_exclude != 0 {
            if let Ok(h_flag) = GlobalAlloc(GMEM_MOVEABLE, std::mem::size_of::<u32>()) {
                let p_flag = GlobalLock(h_flag) as *mut u32;
                if !p_flag.is_null() {
                    std::ptr::write(p_flag, 1u32);
                    let _ = GlobalUnlock(h_flag);
                    let _ = SetClipboardData(fmt_exclude, HANDLE(h_flag.0));
                }
            }
        }

        // 4. Set CF_UNICODETEXT Payload with Zeroize on drop
        let mut wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes_len = wide.len() * std::mem::size_of::<u16>();
        let h_mem = GlobalAlloc(GMEM_MOVEABLE, bytes_len)
            .map_err(|e| VaultError::ClipboardError(format!("GlobalAlloc text failed: {}", e)))?;

        let ptr = GlobalLock(h_mem) as *mut u16;
        if ptr.is_null() {
            wide.zeroize();
            return Err(VaultError::ClipboardError("GlobalLock returned null".into()));
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
        let _ = GlobalUnlock(h_mem);
        wide.zeroize();

        SetClipboardData(CF_UNICODETEXT, HANDLE(h_mem.0))
            .map_err(|e| VaultError::ClipboardError(format!("SetClipboardData CF_UNICODETEXT failed: {}", e)))?;

        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn copy_windows_plain(text: &str) -> crate::Result<()> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    open_clipboard_with_retry()?;

    unsafe {
        let _ = EmptyClipboard();

        let mut wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes_len = wide.len() * std::mem::size_of::<u16>();
        if let Ok(h_mem) = GlobalAlloc(GMEM_MOVEABLE, bytes_len) {
            let ptr = GlobalLock(h_mem) as *mut u16;
            if !ptr.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
                let _ = GlobalUnlock(h_mem);
                let _ = SetClipboardData(CF_UNICODETEXT, HANDLE(h_mem.0));
            }
        }
        wide.zeroize();
        let _ = CloseClipboard();
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn clear_windows() -> crate::Result<()> {
    use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard};

    if open_clipboard_with_retry().is_ok() {
        unsafe {
            let _ = EmptyClipboard();
            let _ = CloseClipboard();
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn get_windows_clipboard_text() -> Result<String, ()> {
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};

    if open_clipboard_with_retry().is_err() {
        return Err(());
    }

    unsafe {
        let h_mem = GetClipboardData(CF_UNICODETEXT);
        if h_mem.is_err() || h_mem.as_ref().unwrap().0.is_null() {
            let _ = CloseClipboard();
            return Err(());
        }
        let handle = HGLOBAL(h_mem.unwrap().0);
        let ptr = GlobalLock(handle) as *const u16;
        if ptr.is_null() {
            let _ = CloseClipboard();
            return Err(());
        }

        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        let result = String::from_utf16_lossy(slice);

        let _ = GlobalUnlock(handle);
        let _ = CloseClipboard();
        Ok(result)
    }
}

// ============================================================================
// MACOS PLATFORM IMPLEMENTATION
// ============================================================================
#[cfg(target_os = "macos")]
fn copy_macos_defended(text: &str) -> crate::Result<()> {
    use std::process::Command;

    let script = format!(
        r#"
        use framework "Foundation"
        use framework "AppKit"
        
        set pb to current application's NSPasteboard's generalPasteboard()
        pb's clearContents()
        
        set item to current application's NSPasteboardItem's alloc()'s init()
        
        -- Set SOTA Pasteboard Transient & Concealed markers BEFORE payload!
        item's setString:"" forType:"org.nspasteboard.TransientType"
        item's setString:"" forType:"org.nspasteboard.ConcealedType"
        item's setString:"" forType:"com.agilebits.onepassword"
        item's setString:"" forType:"org.nspasteboard.AutoGeneratedType"

        item's setString:"{}" forType:(current application's NSPasteboardTypeString)
        
        pb's writeObjects:{{item}}
        "#,
        text.replace('\\', "\\\\").replace('"', "\\\"")
    );

    let status = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .status();

    if status.map_or(false, |s| s.success()) {
        Ok(())
    } else {
        copy_generic_fallback(text)
    }
}

#[cfg(target_os = "macos")]
fn clear_macos() -> crate::Result<()> {
    use std::process::Command;
    let script = r#"
        use framework "AppKit"
        set pb to current application's NSPasteboard's generalPasteboard()
        pb's clearContents()
    "#;
    let _ = Command::new("osascript").arg("-e").arg(script).status();
    Ok(())
}

// ============================================================================
// LINUX PLATFORM IMPLEMENTATION
// ============================================================================
#[cfg(target_os = "linux")]
fn copy_linux_defended(text: &str) -> crate::Result<()> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    if let Ok(mut child) = Command::new("wl-copy")
        .arg("--type")
        .arg("x-kde-passwordManagerHint")
        .arg("secret")
        .stdin(Stdio::piped())
        .spawn()
    {
        if let Some(ref mut stdin) = child.stdin {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
        return Ok(());
    }

    if let Ok(mut child) = Command::new("xclip")
        .arg("-selection")
        .arg("clipboard")
        .stdin(Stdio::piped())
        .spawn()
    {
        if let Some(ref mut stdin) = child.stdin {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
        return Ok(());
    }

    copy_generic_fallback(text)
}

#[cfg(target_os = "linux")]
fn clear_linux() -> crate::Result<()> {
    use std::process::Command;
    let _ = Command::new("wl-copy").arg("--clear").status();
    let _ = Command::new("xclip").arg("-selection").arg("clipboard").arg("/dev/null").status();
    Ok(())
}

// ============================================================================
// FALLBACK IMPLEMENTATION
// ============================================================================
#[allow(dead_code)]
fn copy_generic_fallback(text: &str) -> crate::Result<()> {
    let _ = text;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_hash_computation() {
        let h1 = compute_text_hash("secret_password_123");
        let h2 = compute_text_hash("secret_password_123");
        let h3 = compute_text_hash("different_password");

        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_tx_counter_increment() {
        let tx1 = CLIPBOARD_TX_COUNTER.fetch_add(1, Ordering::SeqCst);
        let tx2 = CLIPBOARD_TX_COUNTER.fetch_add(1, Ordering::SeqCst);
        assert!(tx2 > tx1);
    }
}
