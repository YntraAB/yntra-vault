//! Trash and soft-deletion operations for VaultManager.
//!
//! Handles moving entries to trash, listing recoverable trashed items,
//! restoring entries, and permanent deletion.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::VaultError;
use crate::vault::manager::VaultManager;

/// Lightweight trash entry info for the UI.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TrashedEntryPreview {
    pub id: Uuid,
    pub title: String,
    pub deleted_at: chrono::DateTime<Utc>,
    pub days_until_permanent: i64,
}

impl VaultManager {
    /// Get entries currently in the trash.
    pub fn list_trash(&self) -> crate::Result<Vec<TrashedEntryPreview>> {
        if !self.is_unlocked() {
            return Err(VaultError::VaultLocked);
        }

        Ok(self
            .data
            .trash
            .iter()
            .map(|t| TrashedEntryPreview {
                id: t.entry.id,
                title: t.entry.title.clone(),
                deleted_at: t.deleted_at,
                days_until_permanent: 30 - (Utc::now() - t.deleted_at).num_days(),
            })
            .collect())
    }

    /// Restore an entry from trash back to active entries.
    pub fn restore_from_trash(&mut self, id: Uuid) -> crate::Result<()> {
        if !self.is_unlocked() {
            return Err(VaultError::VaultLocked);
        }

        let pos = self
            .data
            .trash
            .iter()
            .position(|t| t.entry.id == id)
            .ok_or_else(|| VaultError::EntryNotFound(id.to_string()))?;

        let mut trashed = self.data.trash.remove(pos);
        // Refresh updated_at strictly succeeding deleted_at so 3-way sync recognizes restoration over remote trash tombstones
        let mut now = Utc::now();
        if now <= trashed.deleted_at {
            now = trashed.deleted_at + chrono::Duration::milliseconds(1);
        }
        trashed.entry.updated_at = now;

        self.remove_entry_from_index(id);
        self.data.entries.retain(|e| e.id != id);

        let e = &trashed.entry;
        self.add_entry_to_index(e.id, &e.title, &e.username, &e.url, &e.email, &e.tags);
        self.data.entries.push(trashed.entry);
        self.save()?;
        Ok(())
    }

    /// Permanently delete an individual entry from trash.
    pub fn permanent_delete(&mut self, id: Uuid) -> crate::Result<()> {
        if !self.is_unlocked() {
            return Err(VaultError::VaultLocked);
        }
        self.data.trash.retain(|t| t.entry.id != id);
        self.save()?;
        Ok(())
    }

    /// Permanently empty all entries from trash.
    pub fn empty_trash(&mut self) -> crate::Result<()> {
        if !self.is_unlocked() {
            return Err(VaultError::VaultLocked);
        }
        self.data.trash.clear();
        self.save()?;
        Ok(())
    }
}
