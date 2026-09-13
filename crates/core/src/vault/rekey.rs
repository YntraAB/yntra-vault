//! Rekeying, master password changes, and key file operations for VaultManager.
//!
//! Handles re-encrypting all entries, attachments, and password histories under
//! a newly derived master key, along with keyfile generation.

use std::fs;
use std::path::Path;
use subtle::ConstantTimeEq;
use zeroize::Zeroize;

use crate::crypto::{derive_master_key_with_keyfile, derive_subkeys, EntryKey};
use crate::crypto::kdf::generate_salt;
use crate::error::VaultError;
use crate::vault::format::{FileHeader, KdfParams, FORMAT_VERSION};
use crate::vault::manager::{read_key_file_safely, VaultManager};
use crate::vault::types::{Entry, FieldScope};

impl VaultManager {
    /// Generate a 32-byte cryptographically secure random key file at path.
    pub fn generate_key_file(path: &Path) -> crate::Result<()> {
        use rand::Rng;
        let mut key_bytes = [0u8; 32];
        rand::rng().fill(&mut key_bytes);
        fs::write(path, key_bytes)?;
        key_bytes.zeroize();
        Ok(())
    }

    /// Re-encrypt all sensitive fields of an entry with a new entry key.
    pub(crate) fn reencrypt_entry(
        entry: &mut Entry,
        current_key: &EntryKey,
        new_key: &EntryKey,
    ) -> crate::Result<()> {
        let pw_bytes =
            Self::decrypt_entry_field(&entry.encrypted_password, current_key, &entry.id, "password")?;
        entry.encrypted_password =
            Self::encrypt_entry_field(&pw_bytes, new_key, &entry.id, "password")?;

        if let Some(ref totp) = entry.encrypted_totp_secret {
            let totp_bytes = Self::decrypt_entry_field(totp, current_key, &entry.id, "totp")?;
            entry.encrypted_totp_secret =
                Some(Self::encrypt_entry_field(&totp_bytes, new_key, &entry.id, "totp")?);
        }

        if let Some(ref passkey) = entry.encrypted_passkey {
            let passkey_bytes = Self::decrypt_entry_field(passkey, current_key, &entry.id, "passkey")?;
            entry.encrypted_passkey =
                Some(Self::encrypt_entry_field(&passkey_bytes, new_key, &entry.id, "passkey")?);
        }

        for hist in &mut entry.password_history {
            let hist_bytes = Self::decrypt_entry_field(&hist.encrypted_password, current_key, &entry.id, "history")
                .or_else(|_| Self::decrypt_entry_field(&hist.encrypted_password, current_key, &entry.id, "password"))?;
            hist.encrypted_password =
                Self::encrypt_entry_field(&hist_bytes, new_key, &entry.id, "history")?;
        }

        for att in &mut entry.attachments {
            let scope = FieldScope::Attachment {
                attachment_id: &att.id,
            };
            let att_bytes = Self::decrypt_entry_field(&att.encrypted_blob, current_key, &entry.id, scope)?;
            att.encrypted_blob = Self::encrypt_entry_field(&att_bytes, new_key, &entry.id, scope)?;
        }

        Ok(())
    }

    /// Change the vault's master password.
    pub fn change_master_password(&mut self, current: &str, new_password: &str) -> crate::Result<()> {
        self.change_master_password_with_keyfiles(current, None, new_password, None)
    }

    /// Change the master password with optional current and new key file support.
    pub fn change_master_password_with_keyfiles(
        &mut self,
        current: &str,
        current_key_file: Option<&Path>,
        new_password: &str,
        new_key_file: Option<&Path>,
    ) -> crate::Result<()> {
        let cur_kf_bytes = match current_key_file {
            Some(kf_path) => Some(read_key_file_safely(kf_path)?),
            None => None,
        };

        // Verify current password by trying to derive same keys
        let current_mk = derive_master_key_with_keyfile(
            current.as_bytes(),
            cur_kf_bytes.as_ref().map(|b| b.as_slice()),
            &self.salt,
        )?;
        let current_keys = derive_subkeys(&current_mk)?;

        // Verify current password against active session keys using constant-time comparison
        let active_keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;
        if !bool::from(active_keys.vault_key.ct_eq(&current_keys.vault_key)) {
            return Err(VaultError::InvalidPassword);
        }

        let new_kf_bytes = match new_key_file {
            Some(kf_path) => Some(read_key_file_safely(kf_path)?),
            None => None,
        };

        // Generate new salt
        let new_salt = generate_salt();
        let new_mk = derive_master_key_with_keyfile(
            new_password.as_bytes(),
            new_kf_bytes.as_ref().map(|b| b.as_slice()),
            &new_salt,
        )?;
        let new_keys = derive_subkeys(&new_mk)?;

        // Re-encrypt every active entry's sensitive fields with new keys
        for entry in &mut self.data.entries {
            Self::reencrypt_entry(entry, &current_keys.entry_key, &new_keys.entry_key)?;
        }

        // Re-encrypt every trashed entry's sensitive fields with new keys
        for trashed in &mut self.data.trash {
            Self::reencrypt_entry(&mut trashed.entry, &current_keys.entry_key, &new_keys.entry_key)?;
        }

        // Update salt and keys
        self.salt = new_salt;
        self.keys = Some(new_keys.clone());

        // Hardware 2FA envelopes wrapped old keys and require the physical token to re-encrypt.
        // Invalidate stale envelopes so user can re-enroll with their physical key under the new master password.
        self.hardware2fa = None;

        if self.biometric.is_some() {
            let temp_header = FileHeader {
                version: FORMAT_VERSION,
                flags: crate::vault::format::FLAG_HAS_BIOMETRIC,
                salt: self.salt,
                kdf_params: KdfParams::default(),
            };
            if let Ok(aad) = temp_header.aad_bytes() {
                match crate::crypto::biometric::create_embedded_biometric_header(&new_keys, &aad) {
                    Ok(new_bio) => self.biometric = Some(new_bio),
                    Err(_) => self.biometric = None,
                }
            } else {
                self.biometric = None;
            }
        }

        self.save()?;
        self.rebuild_search_index();
        Ok(())
    }
}
