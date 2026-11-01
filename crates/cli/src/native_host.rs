//! Yntra Vault — Browser Extension Native Messaging Host

use std::io::{self, Read, Write};
use std::path::PathBuf;
#[cfg(windows)]
use std::process::Command;
use serde_json::json;

use yntra_vault_core::{Result, VaultError};
use crate::ipc::{try_ipc_request, IpcRequest, IpcResponse};

pub async fn run_native_host_loop() -> Result<()> {
    verify_caller()?;
    let mut stdin_handle = io::stdin();
    let mut stdout_handle = io::stdout();

    loop {
        let mut len_bytes = [0u8; 4];
        if stdin_handle.read_exact(&mut len_bytes).is_err() {
            break;
        }

        let msg_len = u32::from_ne_bytes(len_bytes) as usize;
        if msg_len == 0 || msg_len > 10 * 1024 * 1024 {
            break;
        }

        let mut msg_buf = vec![0u8; msg_len];
        if stdin_handle.read_exact(&mut msg_buf).is_err() {
            break;
        }

        let resp_val = if let Ok(req_val) = serde_json::from_slice::<serde_json::Value>(&msg_buf) {
            let command = req_val.get("command").and_then(|v| v.as_str()).unwrap_or("");
            
            match command {
                "ping" => json!({"status": "ok", "version": env!("CARGO_PKG_VERSION")}),
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
            }
        } else { json!({"status": "error", "error": "Invalid JSON request"}) };
            let mut resp_bytes = serde_json::to_vec(&resp_val).map_err(|e| VaultError::InvalidFormat(e.to_string()))?;
            if resp_bytes.len() > 1024 * 1024 {
                resp_bytes = br#"{"status":"error","error":"Response exceeds the native messaging limit"}"#.to_vec();
            }
            let resp_len_bytes = (resp_bytes.len() as u32).to_ne_bytes();
            stdout_handle.write_all(&resp_len_bytes)?;
            stdout_handle.write_all(&resp_bytes)?;
            stdout_handle.flush()?;
    }

    Ok(())
}

pub fn is_browser_native_host_invocation(args: &[String]) -> bool {
    args.get(1).is_some_and(|arg| {
        let lower = arg.to_lowercase();
        lower.starts_with("chrome-extension://")
            || lower.starts_with("moz-extension://")
            || lower.starts_with("edge-extension://")
            || (lower.ends_with(".json") && (lower.contains("nativemessaginghosts") || lower.contains("com.yntra.vault")))
            || lower == "--native-host"
    })
}


fn manifest_directory(browser: &str) -> Result<PathBuf> {
    #[cfg(windows)]
    let directory = PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or_else(|| VaultError::InvalidState("LOCALAPPDATA is unavailable".into()))?).join("YntraVault").join("NativeMessagingHosts").join(&browser);
    #[cfg(not(windows))]
    let directory = {
        let home = dirs::home_dir().ok_or_else(|| VaultError::InvalidState("Home directory is unavailable".into()))?;
        #[cfg(target_os = "macos")]
        let relative = match browser { "firefox" => "Library/Application Support/Mozilla/NativeMessagingHosts", "edge" => "Library/Application Support/Microsoft Edge/NativeMessagingHosts", _ => "Library/Application Support/Google/Chrome/NativeMessagingHosts" };
        #[cfg(not(target_os = "macos"))]
        let relative = match browser.as_str() { "firefox" => ".mozilla/native-messaging-hosts", "edge" => ".config/microsoft-edge/NativeMessagingHosts", _ => ".config/google-chrome/NativeMessagingHosts" };
        home.join(relative)
    };
    Ok(directory)
}

fn caller_allowed(args: &[String], manifests: &[serde_json::Value], has_session: bool) -> bool {
    let Some(first) = args.get(1) else { return false; };
    if first == "--native-host" {
        return has_session;
    }
    if first.starts_with("chrome-extension://") || first.starts_with("edge-extension://") || first.starts_with("moz-extension://") {
        return manifests.iter().any(|manifest| manifest["allowed_origins"].as_array()
            .is_some_and(|origins| origins.iter().any(|origin| origin.as_str() == Some(first))));
    }
    // Firefox supplies the registered manifest path followed by its extension ID.
    if first.ends_with(".json") {
        return args.get(2).is_some_and(|id| manifests.iter().any(|manifest|
            manifest["allowed_extensions"].as_array().is_some_and(|ids|
                ids.iter().any(|allowed| allowed.as_str() == Some(id)))));
    }
    has_session && args.windows(2).any(|pair| pair[0] == "native-host" && matches!(pair[1].as_str(), "run" | "stdio"))
}

fn verify_caller() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let executable = std::env::current_exe()?;
    let manifests: Vec<_> = ["chrome", "edge", "firefox"].iter().filter_map(|browser| {
        let path = manifest_directory(browser).ok()?.join("com.yntra.vault.json");
        if std::fs::metadata(&path).ok()?.len() > 65536 { return None; }
        let manifest: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
        (manifest["name"] == "com.yntra.vault" && manifest["path"].as_str()
            .is_some_and(|path| std::path::Path::new(path) == executable)).then_some(manifest)
    }).collect();
    let has_session = !crate::ipc::get_session_token_from_env().is_empty();
    if caller_allowed(&args, &manifests, has_session) { Ok(()) }
    else { Err(VaultError::InvalidState("Native host requires a registered extension origin or an active CLI session".into())) }
}

fn native_manifest(browser: &str, extension_id: &str, executable: &std::path::Path) -> Result<serde_json::Value> {
    if !matches!(browser, "chrome" | "edge" | "firefox") {
        return Err(VaultError::InvalidFormat("Choose chrome, edge or firefox for native-host installation".into()));
    }
    let valid = if browser == "firefox" {
        !extension_id.is_empty() && extension_id.len() <= 255 && !extension_id.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, '/' | '\\' | '*'))
    } else { extension_id.len() == 32 && extension_id.bytes().all(|c| (b'a'..=b'p').contains(&c)) };
    if !valid { return Err(VaultError::InvalidFormat("Provide the exact installed extension ID with --extension-id".into())); }
    let mut manifest = json!({ "name": "com.yntra.vault", "description": "Yntra Vault Password Manager Native Messaging Host", "path": executable, "type": "stdio" });
    if browser == "firefox" { manifest["allowed_extensions"] = json!([extension_id]); }
    else { manifest["allowed_origins"] = json!([format!("chrome-extension://{extension_id}/")]); }
    Ok(manifest)
}

pub fn install_native_host_manifest(browser: &str, extension_id: &str) -> Result<()> {
    let browser = browser.to_lowercase();
    let executable = std::env::current_exe()?;
    let manifest = native_manifest(&browser, extension_id, &executable)?;
    let directory = manifest_directory(&browser)?;
    std::fs::create_dir_all(&directory)?;
    let manifest_path = directory.join("com.yntra.vault.json");
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| VaultError::InvalidFormat(e.to_string()))?;
    let mut staged = tempfile::NamedTempFile::new_in(&directory)?;
    staged.write_all(&bytes)?;
    staged.as_file().sync_all()?;
    staged.persist(&manifest_path).map_err(|error| VaultError::from(error.error))?;
    #[cfg(windows)]
    {
        let key = match browser.as_str() {
            "firefox" => r"HKCU\Software\Mozilla\NativeMessagingHosts\com.yntra.vault",
            "edge" => r"HKCU\Software\Microsoft\Edge\NativeMessagingHosts\com.yntra.vault",
            _ => r"HKCU\Software\Google\Chrome\NativeMessagingHosts\com.yntra.vault",
        };
        let status = Command::new("reg").args(["add", key, "/ve", "/t", "REG_SZ", "/d", &manifest_path.to_string_lossy(), "/f"]).status()?;
        if !status.success() { return Err(VaultError::InvalidState("Native messaging registration failed".into())); }
    }
    println!("Native messaging host installed for {}: {}", browser, manifest_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caller_must_match_registered_origin_or_have_a_session() {
        let manifests = vec![native_manifest("chrome", "abcdefghijklmnopabcdefghijklmnop", std::path::Path::new("/yntra")).unwrap(), native_manifest("firefox", "fixture@example.test", std::path::Path::new("/yntra")).unwrap()];
        let args = |values: &[&str]| values.iter().map(|value| value.to_string()).collect::<Vec<_>>();
        assert!(caller_allowed(&args(&["yntra", "chrome-extension://abcdefghijklmnopabcdefghijklmnop/"]), &manifests, false));
        assert!(!caller_allowed(&args(&["yntra", "chrome-extension://pppppppppppppppppppppppppppppppp/"]), &manifests, true));
        assert!(caller_allowed(&args(&["yntra", "com.yntra.vault.json", "fixture@example.test"]), &manifests, false));
        assert!(!caller_allowed(&args(&["yntra", "com.yntra.vault.json", "other@example.test"]), &manifests, true));
        assert!(!caller_allowed(&args(&["yntra", "native-host", "run"]), &manifests, false));
        assert!(caller_allowed(&args(&["yntra", "native-host", "run"]), &manifests, true));
        assert!(caller_allowed(&args(&["yntra", "--json", "--path", "fixture.vdb", "native-host", "stdio"]), &manifests, true));
    }

    #[test]
    fn manifests_bind_one_real_extension_and_browser_family() {
        let path = std::path::Path::new("/fixture/yntra");
        for browser in ["chrome", "edge"] {
            let manifest = native_manifest(browser, "abcdefghijklmnopabcdefghijklmnop", path).unwrap();
            assert_eq!(manifest["allowed_origins"], json!(["chrome-extension://abcdefghijklmnopabcdefghijklmnop/"]));
            assert!(manifest.get("allowed_extensions").is_none());
        }
        let firefox = native_manifest("firefox", "fixture@example.test", path).unwrap();
        assert_eq!(firefox["allowed_extensions"], json!(["fixture@example.test"]));
        assert!(firefox.get("allowed_origins").is_none());
        for (browser, id) in [("all", "fixture"), ("chrome", "*"), ("chrome", "yntravaultbrowserextensionid"), ("firefox", "*")] { assert!(native_manifest(browser, id, path).is_err()); }
        assert!(!is_browser_native_host_invocation(&["yntra.exe".into(), "native-host".into(), "install".into()]));
        assert!(!is_browser_native_host_invocation(&["yntra.exe".into(), "get".into(), "chrome-extension://example/".into()]));
    }

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
