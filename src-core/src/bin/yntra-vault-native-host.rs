//! Browser Native Messaging Host for Yntra Vault.
//! Receives JSON commands from standard input, validates the calling process,
//! forwards them via IPC to the main Yntra Vault app, and writes responses to standard output.

use std::io::{self, Read, Write};

/// Limit messages to 1MB to prevent Denial of Service memory exhaustion.
const MAX_MSG_LEN: usize = 1_048_576;

// ─── Windows Process Validation ─────────────────────────────────────────
#[cfg(target_os = "windows")]
mod parent_validation {
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32First, Process32Next, PROCESSENTRY32, TH32CS_SNAPPROCESS
    };
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows::Win32::System::ProcessStatus::K32GetModuleFileNameExW;

    pub fn get_parent_pid() -> Option<u32> {
        let pid = std::process::id();
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
            let mut entry = PROCESSENTRY32 {
                dwSize: std::mem::size_of::<PROCESSENTRY32>() as u32,
                ..Default::default()
            };
            if Process32First(snapshot, &mut entry).is_ok() {
                loop {
                    if entry.th32ProcessID == pid {
                        let parent_pid = entry.th32ParentProcessID;
                        let _ = windows::Win32::Foundation::CloseHandle(snapshot);
                        return Some(parent_pid);
                    }
                    if Process32Next(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = windows::Win32::Foundation::CloseHandle(snapshot);
        }
        None
    }

    pub fn get_process_path(pid: u32) -> Option<String> {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut path_buf = vec![0u16; 4096];
            let len = K32GetModuleFileNameExW(handle, None, &mut path_buf);
            let _ = windows::Win32::Foundation::CloseHandle(handle);
            if len > 0 {
                let path_str = String::from_utf16_lossy(&path_buf[..len as usize]);
                return Some(path_str);
            }
        }
        None
    }

    pub fn verify_parent_process() -> bool {
        if let Some(parent_pid) = get_parent_pid() {
            if let Some(path) = get_process_path(parent_pid) {
                let lower = path.to_lowercase();
                let allowed = [
                    "chrome.exe",
                    "firefox.exe",
                    "msedge.exe",
                    "brave.exe",
                    "vivaldi.exe",
                    "arc.exe",
                ];
                for exe in &allowed {
                    if lower.ends_with(exe) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

#[cfg(not(target_os = "windows"))]
mod parent_validation {
    pub fn verify_parent_process() -> bool {
        let ppid = unsafe { libc::getppid() } as u32;

        #[cfg(target_os = "linux")]
        {
            let exe_link = format!("/proc/{}/exe", ppid);
            if let Ok(path) = std::fs::read_link(&exe_link) {
                let path_str = path.to_string_lossy().to_lowercase();
                let allowed = [
                    "chrome", "firefox", "msedge", "brave", "vivaldi", "arc",
                ];
                return allowed.iter().any(|exe| path_str.contains(exe));
            }
            false
        }

        #[cfg(target_os = "macos")]
        {
            let mut buf = vec![0u8; 4096];
            let ret = unsafe {
                libc::proc_pidpath(
                    ppid as i32,
                    buf.as_mut_ptr() as *mut libc::c_void,
                    buf.len() as u32,
                )
            };
            if ret > 0 {
                let path_str = String::from_utf8_lossy(&buf[..ret as usize]).to_lowercase();
                let allowed = [
                    "chrome", "firefox", "safari", "brave", "vivaldi", "arc",
                ];
                return allowed.iter().any(|exe| path_str.contains(exe));
            }
            false
        }
    }
}

// ─── IPC Connection Handles ─────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn connect_ipc() -> io::Result<std::fs::File> {
    std::fs::File::options()
        .read(true)
        .write(true)
        .open(r"\\.\pipe\yntra-vault-ipc")
}

#[cfg(not(target_os = "windows"))]
fn connect_ipc() -> io::Result<std::os::unix::net::UnixStream> {
    std::os::unix::net::UnixStream::connect("/tmp/yntra-vault-ipc.sock")
}

// ─── Native Messaging IO Helper ─────────────────────────────────────────

fn read_message() -> io::Result<Option<Vec<u8>>> {
    let mut len_buf = [0u8; 4];
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    
    if handle.read_exact(&mut len_buf).is_err() {
        return Ok(None); // End of File
    }
    
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_MSG_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Message length {} exceeds limit of 1MB", len),
        ));
    }
    
    let mut buf = vec![0u8; len];
    handle.read_exact(&mut buf)?;
    Ok(Some(buf))
}

fn write_message(msg: &[u8]) -> io::Result<()> {
    let len = msg.len() as u32;
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    
    handle.write_all(&len.to_le_bytes())?;
    handle.write_all(msg)?;
    handle.flush()?;
    Ok(())
}

fn main() {
    // 1. Authenticate parent process (reject callers that aren't approved browsers)
    if !parent_validation::verify_parent_process() {
        let err_json = b"{\"error\": \"Unauthorized parent process. Request rejected.\"}";
        let _ = write_message(err_json);
        std::process::exit(1);
    }

    // 2. Message Loop
    loop {
        match read_message() {
            Ok(Some(msg)) => {
                // Forward message over IPC to main Yntra Vault instance
                match connect_ipc() {
                    Ok(mut stream) => {
                        // Write size prefix and payload to IPC stream
                        let msg_len = msg.len() as u32;
                        if stream.write_all(&msg_len.to_be_bytes()).is_err() || stream.write_all(&msg).is_err() {
                            let _ = write_message(b"{\"error\": \"IPC write failed\"}");
                            continue;
                        }
                        let _ = stream.flush();

                        // Read response size and payload from IPC stream
                        let mut resp_len_buf = [0u8; 4];
                        if stream.read_exact(&mut resp_len_buf).is_ok() {
                            let resp_len = u32::from_be_bytes(resp_len_buf) as usize;
                            if resp_len <= MAX_MSG_LEN {
                                let mut resp_buf = vec![0u8; resp_len];
                                if stream.read_exact(&mut resp_buf).is_ok() {
                                    let _ = write_message(&resp_buf);
                                    continue;
                                }
                            }
                        }
                        let _ = write_message(b"{\"error\": \"Invalid IPC response received\"}");
                    }
                    Err(_) => {
                        // If Tauri application is locked/not running, return descriptive error
                        let _ = write_message(b"{\"error\": \"Yntra Vault application is not running or locked.\"}");
                    }
                }
            }
            Ok(None) => break, // EOF reached
            Err(e) => {
                let err_msg = format!("{{\"error\": \"Host error: {}\"}}", e);
                let _ = write_message(err_msg.as_bytes());
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_parent_pid_and_path() {
        #[cfg(target_os = "windows")]
        {
            let ppid = parent_validation::get_parent_pid();
            assert!(ppid.is_some());
            let path = parent_validation::get_process_path(std::process::id()).unwrap();
            assert!(path.to_lowercase().contains("yntra") || path.to_lowercase().contains("cargo") || path.to_lowercase().contains("test"));
        }
    }
}
