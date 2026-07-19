//! IPC Server for browser native messaging host integration.
//! Listens on Named Pipes (Windows) and Unix Domain Sockets (macOS/Linux).
//! Verifies incoming queries using a DPAPI/Keychain decrypted session token.

use std::fs;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use crate::vault::manager::VaultManager;

// ─── Windows Named Pipe Server ──────────────────────────────────────────
#[cfg(target_os = "windows")]
pub fn start_ipc_server(manager_state: Arc<Mutex<Option<VaultManager>>>) {
    thread::spawn(move || {
        use windows::Win32::System::Pipes::{
            CreateNamedPipeW, ConnectNamedPipe, DisconnectNamedPipe,
            PIPE_ACCESS_DUPLEX, PIPE_TYPE_BYTE, PIPE_WAIT
        };
        use windows::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE, CloseHandle};
        use windows::core::PCWSTR;
        use std::os::windows::io::FromRawHandle;

        let pipe_name: Vec<u16> = r"\\.\pipe\yntra-vault-ipc\0".encode_utf16().collect();

        loop {
            unsafe {
                let h_pipe = CreateNamedPipeW(
                    PCWSTR::from_raw(pipe_name.as_ptr()),
                    PIPE_ACCESS_DUPLEX,
                    PIPE_TYPE_BYTE | PIPE_WAIT,
                    1, // Max instance count
                    1024 * 64, // Output buffer
                    1024 * 64, // Input buffer
                    0,
                    None,
                );

                if h_pipe == INVALID_HANDLE_VALUE {
                    thread::sleep(std::time::Duration::from_millis(500));
                    continue;
                }

                if ConnectNamedPipe(h_pipe, None).is_ok() {
                    let mut file = std::fs::File::from_raw_handle(h_pipe.0 as *mut _);
                    let mut len_buf = [0u8; 4];
                    if file.read_exact(&mut len_buf).is_ok() {
                        let len = u32::from_be_bytes(len_buf) as usize;
                        if len < 1_048_576 { // 1MB Cap
                            let mut req_buf = vec![0u8; len];
                            if file.read_exact(&mut req_buf).is_ok() {
                                let response = process_ipc_request(&req_buf, &manager_state);
                                let resp_len = response.len() as u32;
                                let _ = file.write_all(&resp_len.to_be_bytes());
                                let _ = file.write_all(&response);
                                let _ = file.flush();
                            }
                        }
                    }
                    let _ = DisconnectNamedPipe(h_pipe);
                } else {
                    let _ = CloseHandle(h_pipe);
                }
            }
        }
    });
}

// ─── Unix Domain Socket Server ──────────────────────────────────────────
#[cfg(not(target_os = "windows"))]
pub fn start_ipc_server(manager_state: Arc<Mutex<Option<VaultManager>>>) {
    thread::spawn(move || {
        use std::os::unix::net::UnixListener;

        let socket_path = "/tmp/yntra-vault-ipc.sock";
        let _ = fs::remove_file(socket_path);

        if let Ok(listener) = UnixListener::bind(socket_path) {
            for stream in listener.incoming() {
                if let Ok(mut stream) = stream {
                    let mut len_buf = [0u8; 4];
                    if stream.read_exact(&mut len_buf).is_ok() {
                        let len = u32::from_be_bytes(len_buf) as usize;
                        if len < 1_048_576 { // 1MB Cap
                            let mut req_buf = vec![0u8; len];
                            if stream.read_exact(&mut req_buf).is_ok() {
                                let response = process_ipc_request(&req_buf, &manager_state);
                                let resp_len = response.len() as u32;
                                let _ = stream.write_all(&resp_len.to_be_bytes());
                                let _ = stream.write_all(&response);
                                let _ = stream.flush();
                            }
                        }
                    }
                }
            }
        }
    });
}

// ─── Query Processing & Token Verification ─────────────────────────────
fn process_ipc_request(req_bytes: &[u8], manager_state: &Arc<Mutex<Option<VaultManager>>>) -> Vec<u8> {
    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(req_bytes) {
        if let Some(action) = json.get("action").and_then(|v| v.as_str()) {
            if action == "get_credentials" {
                if let Some(domain) = json.get("domain").and_then(|v| v.as_str()) {
                    // Verify session token
                    let incoming_token = json.get("session_token").and_then(|v| v.as_str()).unwrap_or("");
                    if let Ok(stored_token) = crate::crypto::read_session_token() {
                        if incoming_token.is_empty() || incoming_token != stored_token {
                            return b"{\"error\": \"Invalid session token\"}".to_vec();
                        }
                    } else {
                        return b"{\"error\": \"No active session token found\"}".to_vec();
                    }

                    // Query credentials from vault
                    let lock = manager_state.lock().unwrap();
                    if let Some(ref manager) = *lock {
                        if let Ok(entries) = manager.search_entries(domain) {
                            for entry_preview in entries {
                                if let Ok(entry) = manager.get_entry(entry_preview.id) {
                                    if entry.url.contains(domain) {
                                        let resp = serde_json::json!({
                                            "username": entry.username,
                                            "password": entry.password,
                                            "email": entry.email
                                        });
                                        return serde_json::to_vec(&resp).unwrap_or_default();
                                    }
                                }
                            }
                        }
                        return b"{\"error\": \"No matching credentials found\"}".to_vec();
                    } else {
                        return b"{\"error\": \"Vault is locked\"}".to_vec();
                    }
                }
            }
        }
    }
    b"{\"error\": \"Invalid request formatting\"}".to_vec()
}
