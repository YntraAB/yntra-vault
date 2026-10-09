//! Rekeying, master password changes, and key file operations for VaultManager.
//!
//! Handles re-encrypting all entries, attachments, and password histories under
//! a newly derived master key, along with keyfile generation.

use std::path::Path;

use crate::crypto::{derive_master_key_with_keyfile, derive_subkeys, EntryKey};
use crate::crypto::kdf::generate_salt;
use crate::error::VaultError;
use crate::vault::format::{FileHeader, KdfParams, FORMAT_VERSION};
use crate::vault::manager::{read_key_file_safely, VaultManager};
use crate::vault::types::{Entry, FieldScope};

impl VaultManager {
    /// Generate a 32-byte cryptographically secure random key file at path.
    pub fn generate_key_file(path: &Path) -> crate::Result<()> {
        let key_bytes = zeroize::Zeroizing::new(generate_salt());
        super::storage::atomic_create(path, key_bytes.as_slice())
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
    pub fn change_master_password(&mut self, current: &str, new_password: &str) -> crate::Result<Option<super::emergency::EmergencyKit>> {
        self.change_master_password_with_keyfiles(current, None, new_password, None)
    }

    /// Change the master password with optional current and new key file support.
    pub fn change_master_password_with_keyfiles(
        &mut self,
        current: &str,
        current_key_file: Option<&Path>,
        new_password: &str,
        new_key_file: Option<&Path>,
    ) -> crate::Result<Option<super::emergency::EmergencyKit>> {
        self.keys.as_ref().ok_or(VaultError::VaultLocked)?;
        super::validation::validate_new_master_password(new_password)?;
        if current.is_empty() { return Err(VaultError::InvalidPassword); }
        let previous = (self.data.clone(), self.keys.clone(), self.salt, self.hardware2fa.clone(), self.biometric.clone(), self.storage.clone());
        let result = self.change_master_password_inner(current, current_key_file, new_password, new_key_file);
        if result.is_err() {
            (self.data, self.keys, self.salt, self.hardware2fa, self.biometric, self.storage) = previous;
        }
        result
    }

    fn change_master_password_inner(
        &mut self,
        current: &str,
        current_key_file: Option<&Path>,
        new_password: &str,
        new_key_file: Option<&Path>,
    ) -> crate::Result<Option<super::emergency::EmergencyKit>> {
        let cur_kf_bytes = match current_key_file {
            Some(kf_path) => Some(read_key_file_safely(kf_path)?),
            None => None,
        };

        // Verify current password against active session keys using constant-time comparison
        if !self.verify_master_password_with_keyfile(current, cur_kf_bytes.as_ref().map(|b| b.as_slice()))? {
            return Err(VaultError::InvalidPassword);
        }

        let new_kf_bytes = match new_key_file {
            Some(kf_path) => Some(read_key_file_safely(kf_path)?),
            None => None,
        };

        let current_keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?.clone();

        // Generate new salt and derive new subkeys
        let new_salt = generate_salt();
        let new_mk = derive_master_key_with_keyfile(
            new_password.as_bytes(),
            new_kf_bytes.as_ref().map(|b| b.as_slice()),
            &new_salt,
        )?;
        let mut new_keys = derive_subkeys(&new_mk)?;
        if self.storage.is_some() {
            new_keys.vault_key.bytes = generate_salt();
        }

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

        let kit = if self.storage.is_some() {
            let had_recovery = self.storage.as_ref().is_some_and(|s| s.header.recovery.is_some());
            if let Some(session) = &mut self.storage {
                session.change_password(new_password, new_kf_bytes.as_ref().map(|b| b.as_slice()), &new_keys)?;
            }
            if had_recovery { Some(self.rotate_emergency_kit()?) } else { None }
        } else {
            // Hardware 2FA envelopes wrapped old keys and require the physical token to re-encrypt.
            // Invalidate stale envelopes so user can re-enroll with their physical key under the new master password.
            self.hardware2fa = None;
            self.data.settings.hardware_password_key = None;

            // Invalidate active emergency kit shares derived from the old master password
            if let Some(ref mut audit) = self.data.settings.emergency_kit_audit
                && !audit.active_fingerprint.is_empty() {
                    audit.history.push(crate::vault::types::EmergencyKitAuditEntry {
                        timestamp: chrono::Utc::now(),
                        fingerprint: audit.active_fingerprint.clone(),
                        action: "invalidated".into(),
                    });
                    audit.active_fingerprint.clear();
                }

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
            None
        };

        self.save()?;
        self.rebuild_search_index();
        Ok(kit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::NewEntry;
    use subtle::ConstantTimeEq;

    #[test]
    fn test_protected_vault_password_change_rotates_entry_keys_and_reencrypts() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("rekey_protected.vdb");
        let mut manager = VaultManager::create("Test", "old password 2026", &path).unwrap();
        let _kit = manager.generate_emergency_kit("old password 2026").unwrap();
        assert!(manager.storage.is_some());

        let entry_id = manager.add_entry(NewEntry {
            title: "Bank Account".into(),
            username: "user".into(),
            password: "TopSecretPassword123!".into(),
            url: "".into(),
            email: "".into(),
            notes: "".into(),
            tags: vec![],
            totp_secret: None,
            custom_fields: vec![],
            entry_type: None,
            generate_passkey: None,
            attachments: None,
        }).unwrap();

        let old_keys = manager.get_subkeys().unwrap().clone();
        let old_blob = manager.get_entry(entry_id).unwrap().password.clone();
        assert_eq!(old_blob, "TopSecretPassword123!");

        // Extract raw encrypted ciphertext of the password field
        let raw_encrypted_before = manager.data.entries.iter().find(|e| e.id == entry_id).unwrap().encrypted_password.clone();

        // Change master password
        let _ = manager.change_master_password("old password 2026", "new password 2026").unwrap();

        let new_keys = manager.get_subkeys().unwrap().clone();
        // Subkeys must be different!
        assert!(!bool::from(old_keys.entry_key.ct_eq(&new_keys.entry_key)));
        assert!(!bool::from(old_keys.vault_key.ct_eq(&new_keys.vault_key)));
        assert!(!bool::from(old_keys.hmac_key.ct_eq(&new_keys.hmac_key)));

        let raw_encrypted_after = manager.data.entries.iter().find(|e| e.id == entry_id).unwrap().encrypted_password.clone();
        // Ciphertext must have changed due to re-encryption under new entry_key!
        assert_ne!(raw_encrypted_before.ciphertext, raw_encrypted_after.ciphertext);

        // Old entry_key must fail to decrypt the re-encrypted entry
        assert!(VaultManager::decrypt_entry_field(&raw_encrypted_after, &old_keys.entry_key, &entry_id, "password").is_err());

        // New entry_key successfully decrypts the entry
        let decrypted = VaultManager::decrypt_entry_field(&raw_encrypted_after, &new_keys.entry_key, &entry_id, "password").unwrap();
        assert_eq!(String::from_utf8(decrypted.to_vec()).unwrap(), "TopSecretPassword123!");

        // Reopening from disk with new password succeeds and verifies entry
        let reopened = VaultManager::open(&path, "new password 2026").unwrap();
        assert_eq!(reopened.get_entry(entry_id).unwrap().password, "TopSecretPassword123!");

        // Reopening from disk with old password fails
        assert!(VaultManager::open(&path, "old password 2026").is_err());
    }
}
