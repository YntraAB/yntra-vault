//! Yntra Vault — OS Keychain & Credential Store Persistence

use std::path::PathBuf;
use crate::{Result, VaultError};

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
    std::fs::write(&path, token.as_bytes())
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
        if let Ok(content) = std::fs::read_to_string(&path) {
            let trimmed = content.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
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
