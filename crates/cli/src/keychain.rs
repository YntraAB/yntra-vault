//! Yntra Vault — OS Keychain & Credential Store Persistence

use std::path::PathBuf;
use yntra_vault_core::{Result, VaultError};
use yntra_vault_core::crypto::tpm::{hardware_wrap_key, hardware_unwrap_key};

fn get_keychain_storage_path() -> PathBuf {
    let mut dir = if let Ok(appdata) = std::env::var("LOCALAPPDATA") {
        PathBuf::from(appdata)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config")
    } else {
        std::env::temp_dir()
    };
    dir.push("YntraVault");
    let _ = std::fs::create_dir_all(&dir);
    dir.push("session.token");
    dir
}

pub fn store_session_token(token: &str) -> Result<()> {
    let path = get_keychain_storage_path();
    let encrypted = hardware_wrap_key(token.as_bytes())
        .map_err(|e| VaultError::InvalidFormat(format!("Failed to hardware-protect session token: {}", e)))?;
    std::fs::write(&path, encrypted)
        .map_err(|e| VaultError::InvalidFormat(format!("Failed to store session token in OS keychain: {}", e)))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn load_session_token() -> Option<String> {
    let path = get_keychain_storage_path();
    if path.exists() {
        if let Ok(bytes) = std::fs::read(&path) {
            // Attempt hardware-unwrapping first (App-Bound DPAPI / TPM / Keychain)
            if let Ok(decrypted) = hardware_unwrap_key(&bytes) {
                if let Ok(token_str) = String::from_utf8(decrypted) {
                    let trimmed = token_str.trim().to_string();
                    if !trimmed.is_empty() {
                        return Some(trimmed);
                    }
                }
            }
            // Fallback for legacy plaintext token file
            if let Ok(token_str) = String::from_utf8(bytes) {
                let trimmed = token_str.trim().to_string();
                if !trimmed.is_empty() {
                    return Some(trimmed);
                }
            }
        }
    }
    None
}

pub fn clear_session_token() -> Result<()> {
    let path = get_keychain_storage_path();
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_session_token_hardware_protected() {
        let test_token = "cli-test-hardware-bound-token-xyz-789";
        store_session_token(test_token).unwrap();

        // 1. Verify file on disk is NOT plaintext
        let path = get_keychain_storage_path();
        let raw_disk_bytes = std::fs::read(&path).unwrap();
        assert_ne!(raw_disk_bytes, test_token.as_bytes());

        // 2. Verify load successfully unwraps the token
        let loaded = load_session_token().unwrap();
        assert_eq!(loaded, test_token);

        // 3. Verify clear removes the file
        clear_session_token().unwrap();
        assert_eq!(load_session_token(), None);
    }
}
