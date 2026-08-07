//! Password history — rollback to previous passwords.

use uuid::Uuid;
use crate::vault::manager::VaultManager;
use crate::vault::types::{PasswordHistoryItem, FieldScope};
use crate::error::VaultError;

/// A decrypted password history item for the UI.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct DecryptedHistoryItem {
    pub password: String,
    pub changed_at: chrono::DateTime<chrono::Utc>,
}

impl VaultManager {
    /// Get password history for an entry (decrypted).
    pub fn get_password_history(&self, entry_id: Uuid) -> crate::Result<Vec<DecryptedHistoryItem>> {
        let keys = self.keys_ref().ok_or(VaultError::VaultLocked)?;

        let entry = self.data_ref().entries.iter()
            .find(|e| e.id == entry_id)
            .ok_or(VaultError::EntryNotFound(entry_id.to_string()))?;

        let mut history = Vec::new();
        for item in &entry.password_history {
            let pw_bytes = VaultManager::decrypt_entry_field(&item.encrypted_password, &keys.entry_key, &entry_id, FieldScope::History)
                .or_else(|_| VaultManager::decrypt_entry_field(&item.encrypted_password, &keys.entry_key, &entry_id, FieldScope::Password))?;
            let password = String::from_utf8(pw_bytes.to_vec())
                .map_err(|e| VaultError::DecryptionError(format!("History password: {}", e)))?;
            history.push(DecryptedHistoryItem {
                password,
                changed_at: item.changed_at,
            });
        }

        // Reverse so newest first
        history.reverse();
        Ok(history)
    }

    /// Restore a specific password from history (makes it the current password).
    pub fn restore_password_from_history(
        &mut self,
        entry_id: Uuid,
        history_index: usize,
    ) -> crate::Result<()> {
        let entry_key = self.keys_ref().ok_or(VaultError::VaultLocked)?.entry_key.clone();

        let entry = self.data_mut().entries.iter_mut()
            .find(|e| e.id == entry_id)
            .ok_or(VaultError::EntryNotFound(entry_id.to_string()))?;

        if history_index >= entry.password_history.len() {
            return Err(VaultError::EntryNotFound("History index out of bounds".into()));
        }

        // Decrypt the history item to restore (trying FieldScope::History, with fallback to FieldScope::Password for legacy vaults)
        let history_item = &entry.password_history[history_index];
        let restored_pw_bytes = VaultManager::decrypt_entry_field(&history_item.encrypted_password, &entry_key, &entry_id, FieldScope::History)
            .or_else(|_| VaultManager::decrypt_entry_field(&history_item.encrypted_password, &entry_key, &entry_id, FieldScope::Password))?;

        // Decrypt current active password (FieldScope::Password)
        let current_pw_bytes = VaultManager::decrypt_entry_field(&entry.encrypted_password, &entry_key, &entry_id, FieldScope::Password)?;

        // Encrypt current active password with FieldScope::History and push to history
        let current_history_blob = VaultManager::encrypt_entry_field(&current_pw_bytes, &entry_key, &entry_id, FieldScope::History)?;
        let current_history = PasswordHistoryItem {
            encrypted_password: current_history_blob,
            changed_at: entry.password_changed_at,
        };

        // Encrypt restored password with FieldScope::Password and set as active password
        entry.encrypted_password = VaultManager::encrypt_entry_field(&restored_pw_bytes, &entry_key, &entry_id, FieldScope::Password)?;
        entry.password_changed_at = chrono::Utc::now();
        entry.updated_at = chrono::Utc::now();

        entry.password_history.push(current_history);

        // Keep max history
        while entry.password_history.len() > crate::vault::types::MAX_PASSWORD_HISTORY {
            entry.password_history.remove(0);
        }

        self.save()?;
        Ok(())
    }

    // Internal helper to access keys
    fn keys_ref(&self) -> Option<&crate::crypto::SubKeys> {
        self.keys.as_ref()
    }
}

