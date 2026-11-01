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

impl Drop for DecryptedHistoryItem {
    fn drop(&mut self) { use zeroize::Zeroize; self.password.zeroize(); }
}

impl VaultManager {
    pub fn list_conflict_history(&self, entry_id: Uuid) -> crate::Result<Vec<ConflictHistoryItem>> {
        self.keys_ref().ok_or(VaultError::VaultLocked)?;
        if !self.data.entries.iter().any(|entry| entry.id == entry_id) {
            return Err(VaultError::EntryNotFound(entry_id.to_string()));
        }
        Ok(self.data.settings.sync.history.get(&entry_id).into_iter().flatten().rev()
            .map(|version| ConflictHistoryItem { id: version.id, saved_at: version.saved_at }).collect())
    }

    fn historical_snapshot(&self, entry_id: Uuid, version_id: Uuid) -> crate::Result<crate::vault::VaultData> {
        let keys = self.keys_ref().ok_or(VaultError::VaultLocked)?;
        let version = self.data.settings.sync.history.get(&entry_id)
            .and_then(|history| history.iter().find(|version| version.id == version_id))
            .ok_or_else(|| VaultError::EntryNotFound("Conflict version no longer exists".into()))?;
        let bytes = Self::decrypt_entry_field(&version.encrypted_entry, &keys.entry_key, &entry_id, "sync-history")?;
        let historical: crate::vault::Entry = rmp_serde::from_slice(&bytes)
            .map_err(|_| VaultError::SerializationError("Invalid conflict history".into()))?;
        if historical.id != entry_id {
            return Err(VaultError::InvalidState("Conflict version identity does not match".into()));
        }
        let mut snapshot = self.data.clone();
        let target = snapshot.entries.iter_mut().find(|entry| entry.id == entry_id)
            .ok_or_else(|| VaultError::EntryNotFound(entry_id.to_string()))?;
        *target = historical;
        Ok(snapshot)
    }

    pub fn preview_conflict_history(&self, entry_id: Uuid, version_id: Uuid) -> crate::Result<crate::services::sync::review::SyncPreview> {
        let snapshot = self.historical_snapshot(entry_id, version_id)?;
        crate::services::sync::review::preview(&self.data, &snapshot, self.get_subkeys()?)
    }

    pub fn inspect_conflict_history(&self, entry_id: Uuid, version_id: Uuid, field: &str) -> crate::Result<String> {
        let snapshot = self.historical_snapshot(entry_id, version_id)?;
        crate::services::sync::review::inspect(&snapshot, entry_id, field, self.get_subkeys()?)
    }

    pub fn restore_conflict_history(&mut self, entry_id: Uuid, version_id: Uuid, expected_revision: &str) -> crate::Result<()> {
        use crate::services::sync::review::{self, Choice, SyncDecision};
        self.transactional_data_change(|manager| {
            if review::revision(&manager.data)? != expected_revision {
                return Err(VaultError::InvalidState("Vault changed; review the historical version again".into()));
            }
            let snapshot = manager.historical_snapshot(entry_id, version_id)?;
            let keys = manager.get_subkeys()?.clone();
            let preview = review::preview(&manager.data, &snapshot, &keys)?;
            let decisions: Vec<_> = preview.changes.iter().map(|change| SyncDecision {
                key: change.key.clone(), choice: Choice::Remote, text: None,
            }).collect();
            manager.data = review::resolve(&manager.data, &snapshot, &preview, &decisions, &keys, manager.session_identity())?;
            manager.rebuild_search_index();
            manager.save_reviewed()
        })
    }

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
        self.transactional_data_change(|manager| manager.restore_password_from_history_inner(entry_id, history_index))
    }

    fn restore_password_from_history_inner(
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
        let history_item = &entry.password_history[entry.password_history.len() - 1 - history_index];
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
        entry.breach_status = Default::default();
        entry.strength_score = None;

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

#[derive(serde::Serialize)]
pub struct ConflictHistoryItem {
    pub id: Uuid,
    pub saved_at: chrono::DateTime<chrono::Utc>,
}

