//! Yntra Vault — Browser Extension Native Messaging Host

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::Command;
use colored::*;
use serde_json::json;

use yntra_vault_core::{Result, VaultError};
use crate::ipc::{try_ipc_request, IpcRequest, IpcResponse};

pub async fn run_native_host_loop() -> Result<()> {
    let mut stdin_handle = io::stdin();
    let mut stdout_handle = io::stdout();

    loop {
        let mut len_bytes = [0u8; 4];
        if stdin_handle.read_exact(&mut len_bytes).is_err() {
            break;
        }

        let msg_len = u32::from_le_bytes(len_bytes) as usize;
        if msg_len == 0 || msg_len > 10 * 1024 * 1024 {
            break;
        }

        let mut msg_buf = vec![0u8; msg_len];
        if stdin_handle.read_exact(&mut msg_buf).is_err() {
            break;
        }

        if let Ok(req_val) = serde_json::from_slice::<serde_json::Value>(&msg_buf) {
            let command = req_val.get("command").and_then(|v| v.as_str()).unwrap_or("list");
            
            let resp_val = match command {
                "ping" => json!({"status": "ok", "version": "0.1.0"}),
                "list" => {
                    if let Some(IpcResponse::ListEntries(list)) = try_ipc_request(&IpcRequest::ListEntries).await {
                        json!({"status": "ok", "entries": list})
                    } else {
                        json!({"status": "locked", "error": "Vault is locked. Run 'yntra unlock' to start daemon."})
                    }
                }
                "get" => {
                    let query = req_val.get("query").and_then(|v| v.as_str()).unwrap_or("");
                    if let Some(IpcResponse::GetEntry(entry)) = try_ipc_request(&IpcRequest::GetEntry { query: query.to_string() }).await {
                        json!({"status": "ok", "entry": entry})
                    } else {
                        json!({"status": "error", "error": "Entry not found or vault locked"})
                    }
                }
                _ => json!({"status": "error", "error": "Unknown command"}),
            };

            let resp_bytes = serde_json::to_vec(&resp_val).unwrap();
            let resp_len_bytes = (resp_bytes.len() as u32).to_le_bytes();

            let _ = stdout_handle.write_all(&resp_len_bytes);
            let _ = stdout_handle.write_all(&resp_bytes);
            let _ = stdout_handle.flush();
        }
    }

    Ok(())
}

pub fn is_browser_native_host_invocation(args: &[String]) -> bool {
    args.iter().skip(1).any(|arg| {
        let lower = arg.to_lowercase();
        lower.starts_with("chrome-extension://")
            || lower.starts_with("moz-extension://")
            || lower.starts_with("edge-extension://")
            || (lower.ends_with(".json") && (lower.contains("nativemessaginghosts") || lower.contains("com.yntra.vault")))
            || lower == "--native-host"
            || lower == "native-host"
    })
}

pub fn install_native_host_manifest(browser: &str) -> Result<()> {
    let exe_path = std::env::current_exe()
        .map_err(|e| VaultError::InvalidFormat(format!("Failed to locate binary path: {}", e)))?;

    let mut dir = if let Ok(appdata) = std::env::var("LOCALAPPDATA") {
        PathBuf::from(appdata)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config")
    } else {
        std::env::temp_dir()
    };
    dir.push("YntraVault");
    let _ = std::fs::create_dir_all(&dir);

    let manifest_path = dir.join("com.yntra.vault.json");

    let is_firefox = browser.eq_ignore_ascii_case("firefox");
    let manifest_json = if is_firefox {
        json!({
            "name": "com.yntra.vault",
            "description": "Yntra Vault Password Manager Native Messaging Host",
            "path": exe_path.to_string_lossy(),
            "type": "stdio",
            "allowed_extensions": [
                "yntravault@yntra.com"
            ]
        })
    } else {
        json!({
            "name": "com.yntra.vault",
            "description": "Yntra Vault Password Manager Native Messaging Host",
            "path": exe_path.to_string_lossy(),
            "type": "stdio",
            "allowed_origins": [
                "chrome-extension://yntravaultbrowserextensionid/",
                "edge-extension://yntravaultbrowserextensionid/"
            ]
        })
    };

    std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest_json).unwrap())
        .map_err(|e| VaultError::InvalidFormat(format!("Failed to write manifest: {}", e)))?;

    println!("{} Generated Native Messaging manifest at: {}", "✓".green().bold(), manifest_path.display().to_string().yellow());

    #[cfg(windows)]
    {
        let browsers: Vec<&str> = if browser.eq_ignore_ascii_case("all") {
            vec!["chrome", "edge", "firefox"]
        } else {
            vec![browser]
        };

        for b in browsers {
            let reg_path = match b.to_lowercase().as_str() {
                "firefox" => r"HKCU\Software\Mozilla\NativeMessagingHosts\com.yntra.vault",
                "edge" => r"HKCU\Software\Microsoft\Edge\NativeMessagingHosts\com.yntra.vault",
                _ => r"HKCU\Software\Google\Chrome\NativeMessagingHosts\com.yntra.vault",
            };

            let status = Command::new("reg")
                .args(&["add", reg_path, "/ve", "/t", "REG_SZ", "/d", &manifest_path.to_string_lossy(), "/f"])
                .status();

            if let Ok(st) = status {
                if st.success() {
                    println!("{} Registered Windows Registry key for {}", "✓".green().bold(), b.cyan());
                }
            }
        }
    }

    #[cfg(not(windows))]
    {
        if let Ok(home) = std::env::var("HOME") {
            let home_path = PathBuf::from(home);
            let target_dirs = vec![
                home_path.join(".config/google-chrome/NativeMessagingHosts"),
                home_path.join(".config/chromium/NativeMessagingHosts"),
                home_path.join(".config/microsoft-edge/NativeMessagingHosts"),
                home_path.join(".mozilla/native-messaging-hosts"),
                home_path.join("Library/Application Support/Google/Chrome/NativeMessagingHosts"),
                home_path.join("Library/Application Support/Mozilla/NativeMessagingHosts"),
            ];

            for t_dir in target_dirs {
                if t_dir.parent().map(|p| p.exists()).unwrap_or(false) {
                    let _ = std::fs::create_dir_all(&t_dir);
                    let target_file = t_dir.join("com.yntra.vault.json");
                    let _ = std::fs::copy(&manifest_path, target_file);
                }
            }
        }
    }

    println!("{} Native Messaging Host installation complete!", "✓".green().bold());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_browser_native_host_detection() {
        assert!(is_browser_native_host_invocation(&[
            "yntra.exe".into(),
            "chrome-extension://knldjmfmopnmpfpdoneapnegffipkgkd/".into(),
        ]));
        assert!(is_browser_native_host_invocation(&[
            "yntra.exe".into(),
            "edge-extension://knldjmfmopnmpfpdoneapnegffipkgkd/".into(),
        ]));
        assert!(is_browser_native_host_invocation(&[
            "yntra.exe".into(),
            r"C:\Users\test\AppData\Local\NativeMessagingHosts\com.yntra.vault.json".into(),
        ]));
        assert!(!is_browser_native_host_invocation(&[
            "yntra.exe".into(),
            "get".into(),
            "github".into(),
        ]));
    }
}
