//! Autostart Configuration for Yntra Vault.
//! Automatically starts the password manager when the user logs in.

// ─── Windows Implementation ─────────────────────────────────────────────
#[cfg(target_os = "windows")]
pub mod windows_autostart {
    use windows::Win32::System::Registry::{
        RegOpenKeyExW, RegSetValueExW, RegDeleteValueW, RegQueryValueExW,
        HKEY_CURRENT_USER, KEY_SET_VALUE, KEY_QUERY_VALUE, REG_SZ
    };
    use windows::core::PCWSTR;

    const SUBKEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run\0";
    const VALUE_NAME: &str = "YntraVault\0";

    pub fn enable_autostart() -> crate::Result<()> {
        let subkey_u16: Vec<u16> = SUBKEY.encode_utf16().collect();
        let value_name_u16: Vec<u16> = VALUE_NAME.encode_utf16().collect();

        // Get path of current executable
        let current_exe = std::env::current_exe().map_err(|e| {
            crate::error::VaultError::EncryptionError(format!("Get current exe: {}", e))
        })?;
        let current_exe_str = current_exe.to_string_lossy().into_owned() + "\0";
        let value_data: Vec<u16> = current_exe_str.encode_utf16().collect();

        unsafe {
            let mut hkey = windows::Win32::System::Registry::HKEY::default();
            let status = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR::from_raw(subkey_u16.as_ptr()),
                0,
                KEY_SET_VALUE,
                &mut hkey,
            );
            if status.is_err() {
                return Err(crate::error::VaultError::EncryptionError(format!("Failed to open registry key: {:?}", status)));
            }

            let set_status = RegSetValueExW(
                hkey,
                PCWSTR::from_raw(value_name_u16.as_ptr()),
                0,
                REG_SZ,
                Some(std::slice::from_raw_parts(value_data.as_ptr() as *const u8, value_data.len() * 2)),
            );
            let _ = windows::Win32::System::Registry::RegCloseKey(hkey);

            if set_status.is_err() {
                return Err(crate::error::VaultError::EncryptionError(format!("Failed to set registry value: {:?}", set_status)));
            }
        }
        Ok(())
    }

    pub fn disable_autostart() -> crate::Result<()> {
        let subkey_u16: Vec<u16> = SUBKEY.encode_utf16().collect();
        let value_name_u16: Vec<u16> = VALUE_NAME.encode_utf16().collect();

        unsafe {
            let mut hkey = windows::Win32::System::Registry::HKEY::default();
            let status = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR::from_raw(subkey_u16.as_ptr()),
                0,
                KEY_SET_VALUE,
                &mut hkey,
            );
            if status.is_err() {
                return Err(crate::error::VaultError::EncryptionError(format!("Failed to open registry key: {:?}", status)));
            }

            let del_status = RegDeleteValueW(hkey, PCWSTR::from_raw(value_name_u16.as_ptr()));
            let _ = windows::Win32::System::Registry::RegCloseKey(hkey);

            if del_status.is_err() {
                // If it doesn't exist (ERROR_FILE_NOT_FOUND = 2), ignore.
                if del_status.0 != 2 {
                    return Err(crate::error::VaultError::EncryptionError(format!("Failed to delete registry value: {:?}", del_status)));
                }
            }
        }
        Ok(())
    }

    pub fn is_autostart_enabled() -> crate::Result<bool> {
        let subkey_u16: Vec<u16> = SUBKEY.encode_utf16().collect();
        let value_name_u16: Vec<u16> = VALUE_NAME.encode_utf16().collect();

        unsafe {
            let mut hkey = windows::Win32::System::Registry::HKEY::default();
            let status = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR::from_raw(subkey_u16.as_ptr()),
                0,
                KEY_QUERY_VALUE,
                &mut hkey,
            );
            if status.is_err() {
                return Ok(false);
            }

            let mut cb_data = 0;
            let query_status = RegQueryValueExW(
                hkey,
                PCWSTR::from_raw(value_name_u16.as_ptr()),
                None,
                None,
                None,
                Some(&mut cb_data),
            );
            let _ = windows::Win32::System::Registry::RegCloseKey(hkey);

            Ok(query_status.is_ok())
        }
    }
}

// ─── macOS Implementation ───────────────────────────────────────────────
#[cfg(target_os = "macos")]
pub mod macos_autostart {
    use std::path::PathBuf;

    fn get_agent_path() -> crate::Result<PathBuf> {
        let home = std::env::var("HOME")
            .map_err(|e| crate::error::VaultError::EncryptionError(format!("No HOME directory: {}", e)))?;
        let mut path = PathBuf::from(home);
        path.push("Library/LaunchAgents/com.yntra.vault.plist");
        Ok(path)
    }

    pub fn enable_autostart() -> crate::Result<()> {
        let path = get_agent_path()?;
        let current_exe = std::env::current_exe().map_err(|e| {
            crate::error::VaultError::EncryptionError(format!("Get current exe: {}", e))
        })?;

        let plist_content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.yntra.vault</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>"#,
            current_exe.to_string_lossy()
        );

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(&path, plist_content).map_err(|e| {
            crate::error::VaultError::EncryptionError(format!("Failed to write LaunchAgent: {}", e))
        })?;
        Ok(())
    }

    pub fn disable_autostart() -> crate::Result<()> {
        let path = get_agent_path()?;
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
        Ok(())
    }

    pub fn is_autostart_enabled() -> crate::Result<bool> {
        let path = get_agent_path()?;
        Ok(path.exists())
    }
}

// ─── Linux Implementation ───────────────────────────────────────────────
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub mod linux_autostart {
    use std::path::PathBuf;

    fn get_desktop_path() -> crate::Result<PathBuf> {
        let home = std::env::var("HOME")
            .map_err(|e| crate::error::VaultError::EncryptionError(format!("No HOME directory: {}", e)))?;
        let mut path = PathBuf::from(home);
        path.push(".config/autostart/yntra-vault.desktop");
        Ok(path)
    }

    pub fn enable_autostart() -> crate::Result<()> {
        let path = get_desktop_path()?;
        let current_exe = std::env::current_exe().map_err(|e| {
            crate::error::VaultError::EncryptionError(format!("Get current exe: {}", e))
        })?;

        let desktop_content = format!(
            r#"[Desktop Entry]
Type=Application
Version=1.0
Name=Yntra Vault
Comment=Yntra Vault Password Manager
Exec={}
StartupNotify=false
Terminal=false
"#,
            current_exe.to_string_lossy()
        );

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(&path, desktop_content).map_err(|e| {
            crate::error::VaultError::EncryptionError(format!("Failed to write desktop autostart: {}", e))
        })?;
        Ok(())
    }

    pub fn disable_autostart() -> crate::Result<()> {
        let path = get_desktop_path()?;
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
        Ok(())
    }

    pub fn is_autostart_enabled() -> crate::Result<bool> {
        let path = get_desktop_path()?;
        Ok(path.exists())
    }
}

// ─── Unified Cross-Platform Exports ─────────────────────────────────────

/// Enable vault autostart on system login.
pub fn enable_autostart() -> crate::Result<()> {
    #[cfg(target_os = "windows")]
    {
        windows_autostart::enable_autostart()
    }
    #[cfg(target_os = "macos")]
    {
        macos_autostart::enable_autostart()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        linux_autostart::enable_autostart()
    }
}

/// Disable vault autostart on system login.
pub fn disable_autostart() -> crate::Result<()> {
    #[cfg(target_os = "windows")]
    {
        windows_autostart::disable_autostart()
    }
    #[cfg(target_os = "macos")]
    {
        macos_autostart::disable_autostart()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        linux_autostart::disable_autostart()
    }
}

/// Query whether vault autostart is currently enabled.
pub fn is_autostart_enabled() -> crate::Result<bool> {
    #[cfg(target_os = "windows")]
    {
        windows_autostart::is_autostart_enabled()
    }
    #[cfg(target_os = "macos")]
    {
        macos_autostart::is_autostart_enabled()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        linux_autostart::is_autostart_enabled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autostart_toggle_roundtrip() {
        // Save initial state
        let initial = is_autostart_enabled().unwrap();

        // Toggle enable
        enable_autostart().unwrap();
        assert!(is_autostart_enabled().unwrap());

        // Toggle disable
        disable_autostart().unwrap();
        assert!(!is_autostart_enabled().unwrap());

        // Restore initial state
        if initial {
            enable_autostart().unwrap();
        }
    }
}
