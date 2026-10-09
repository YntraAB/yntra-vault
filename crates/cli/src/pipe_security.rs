//! Security attributes and DACL management for Windows Named Pipes.
//!
//! Enforces that local pipes (SSH Agent, IPC Daemon) can only be opened by
//! the current user token SID, SYSTEM, and Administrators, rejecting all
//! cross-user access, low-integrity sandboxes, and remote/network connections.

#[cfg(windows)]
pub struct PipeSecurityAttributes {
    sa: windows::Win32::Security::SECURITY_ATTRIBUTES,
    sd: windows::Win32::Security::PSECURITY_DESCRIPTOR,
}

#[cfg(windows)]
impl PipeSecurityAttributes {
    /// Create security attributes restricting access strictly to current user SID + SYSTEM + Admins.
    pub fn new_current_user_only() -> yntra_vault_core::Result<Self> {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::{CloseHandle, HANDLE, LocalFree, HLOCAL, BOOL};
        use windows::Win32::Security::{
            GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER,
            PSECURITY_DESCRIPTOR,
        };
        use windows::Win32::Security::Authorization::{
            ConvertSidToStringSidW,
            ConvertStringSecurityDescriptorToSecurityDescriptorW,
        };
        use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

        unsafe {
            let mut token = HANDLE::default();
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
                .map_err(|e| yntra_vault_core::VaultError::SyncError(format!("Failed to open process token: {}", e)))?;

            let mut return_length = 0;
            let _ = GetTokenInformation(token, TokenUser, None, 0, &mut return_length);
            if return_length == 0 {
                let _ = CloseHandle(token);
                return Err(yntra_vault_core::VaultError::SyncError("Failed to query token user size".into()));
            }

            let mut buffer = vec![0u8; return_length as usize];
            if let Err(e) = GetTokenInformation(token, TokenUser, Some(buffer.as_mut_ptr() as *mut _), return_length, &mut return_length) {
                let _ = CloseHandle(token);
                return Err(yntra_vault_core::VaultError::SyncError(format!("Failed to get token user: {}", e)));
            }
            let _ = CloseHandle(token);

            let token_user = buffer.as_ptr() as *const TOKEN_USER;
            let sid = (*token_user).User.Sid;
            let mut string_sid = windows::core::PWSTR::null();
            ConvertSidToStringSidW(sid, &mut string_sid)
                .map_err(|e| yntra_vault_core::VaultError::SyncError(format!("Failed to convert SID to string: {}", e)))?;

            let user_sid = string_sid.to_string().map_err(|e| {
                let _ = LocalFree(HLOCAL(string_sid.0 as _));
                yntra_vault_core::VaultError::SyncError(format!("Failed to format SID string: {}", e))
            })?;
            let _ = LocalFree(HLOCAL(string_sid.0 as _));

            // SDDL explanation:
            // D: = DACL
            // P = SE_DACL_PROTECTED (no inheritance from parent/defaults)
            // (A;;GA;;;{user_sid}) = Allow Generic All to current user
            // (A;;GA;;;SY) = Allow Generic All to LocalSystem
            // (A;;GA;;;BA) = Allow Generic All to Built-in Administrators
            let sddl = format!("D:P(A;;GA;;;{})(A;;GA;;;SY)(A;;GA;;;BA)", user_sid);
            let wide_sddl: Vec<u16> = sddl.encode_utf16().chain(std::iter::once(0)).collect();

            let mut sd = PSECURITY_DESCRIPTOR::default();
            const SDDL_REVISION_1: u32 = 1;
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR::from_raw(wide_sddl.as_ptr()),
                SDDL_REVISION_1,
                &mut sd,
                None,
            ).map_err(|e| yntra_vault_core::VaultError::SyncError(format!("Failed to compile SDDL: {}", e)))?;

            let sa = windows::Win32::Security::SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<windows::Win32::Security::SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: sd.0,
                bInheritHandle: BOOL(0),
            };

            Ok(Self { sa, sd })
        }
    }

    /// Return a raw pointer suitable for `create_with_security_attributes_raw`.
    pub fn as_mut_ptr(&mut self) -> *mut std::ffi::c_void {
        &mut self.sa as *mut _ as *mut std::ffi::c_void
    }
}

#[cfg(windows)]
impl Drop for PipeSecurityAttributes {
    fn drop(&mut self) {
        if !self.sd.0.is_null() {
            unsafe {
                let _ = windows::Win32::Foundation::LocalFree(windows::Win32::Foundation::HLOCAL(self.sd.0 as _));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use super::*;

    #[cfg(windows)]
    #[test]
    fn test_pipe_security_attributes_creation() {
        let mut attrs = PipeSecurityAttributes::new_current_user_only().expect("Should create security attributes");
        assert!(!attrs.sd.0.is_null());
        assert!(!attrs.as_mut_ptr().is_null());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn test_named_pipe_with_security_attributes_lifecycle() {
        use tokio::net::windows::named_pipe::{ServerOptions, ClientOptions};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let pipe_name = format!(r"\\.\pipe\yntra-test-sec-pipe-{}", uuid::Uuid::new_v4());
        let mut sec_attrs = PipeSecurityAttributes::new_current_user_only().expect("Attributes should succeed");

        let mut server = unsafe {
            ServerOptions::new()
                .first_pipe_instance(true)
                .reject_remote_clients(true)
                .create_with_security_attributes_raw(&pipe_name, sec_attrs.as_mut_ptr())
        }
        .expect("Server should create with security attributes");

        let client_task = tokio::spawn(async move {
            let mut client = ClientOptions::new().open(&pipe_name).expect("Client should connect");
            client.write_all(b"PING").await.expect("Client write");
            let mut buf = [0u8; 4];
            client.read_exact(&mut buf).await.expect("Client read");
            assert_eq!(&buf, b"PONG");
        });

        server.connect().await.expect("Server connect");
        let mut buf = [0u8; 4];
        server.read_exact(&mut buf).await.expect("Server read");
        assert_eq!(&buf, b"PING");
        server.write_all(b"PONG").await.expect("Server write");

        client_task.await.expect("Client task completed");
    }
}
