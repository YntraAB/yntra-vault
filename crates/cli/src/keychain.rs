//! Yntra Vault — OS Keychain & Credential Store Persistence

use std::path::PathBuf;
use yntra_vault_core::{Result, VaultError};
use yntra_vault_core::crypto::tpm::{
    write_session_token, read_session_token, clear_session_token as tpm_clear_session_token,
    get_session_token_path,
};

#[allow(dead_code)]
pub fn get_keychain_storage_path() -> PathBuf {
    get_session_token_path()
}

pub fn store_session_token(token: &str) -> Result<()> {
    write_session_token(token).map_err(VaultError::from)
}

pub fn load_session_token() -> Option<String> {
    read_session_token().ok().filter(|s| !s.trim().is_empty())
}

pub fn clear_session_token() -> Result<()> {
    tpm_clear_session_token().map_err(VaultError::from)
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
