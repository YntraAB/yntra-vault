//! Entry-level operations and search.

use uuid::Uuid;
use crate::vault::types::*;
use crate::vault::manager::VaultManager;

impl VaultManager {

    /// Get entries filtered by tag name.
    pub fn entries_by_tag(&self, tag_name: &str) -> crate::Result<Vec<EntryPreview>> {
        let all = self.list_entries()?;
        Ok(all.into_iter().filter(|e| {
            e.tags.iter().any(|t| t == tag_name)
        }).collect())
    }

    /// Get favorite entries.
    pub fn favorite_entries(&self) -> crate::Result<Vec<EntryPreview>> {
        let all = self.list_entries()?;
        Ok(all.into_iter().filter(|e| e.favorite).collect())
    }

    /// Toggle favorite status for an entry.
    pub fn toggle_favorite(&mut self, id: Uuid) -> crate::Result<bool> {
        if !self.is_unlocked() {
            return Err(crate::error::VaultError::VaultLocked);
        }

        let entry = self.data_mut().entries.iter_mut()
            .find(|e| e.id == id)
            .ok_or(crate::error::VaultError::EntryNotFound(id.to_string()))?;

        entry.favorite = !entry.favorite;
        let new_state = entry.favorite;
        self.save()?;
        Ok(new_state)
    }

    /// Toggle pinned status for an entry.
    pub fn toggle_pin(&mut self, id: Uuid) -> crate::Result<bool> {
        if !self.is_unlocked() {
            return Err(crate::error::VaultError::VaultLocked);
        }

        let entry = self.data_mut().entries.iter_mut()
            .find(|e| e.id == id)
            .ok_or(crate::error::VaultError::EntryNotFound(id.to_string()))?;

        entry.pinned = !entry.pinned;
        let new_state = entry.pinned;
        self.save()?;
        Ok(new_state)
    }

    /// Get entries from the trash.
    pub fn list_trash(&self) -> crate::Result<Vec<TrashedEntryPreview>> {
        if !self.is_unlocked() {
            return Err(crate::error::VaultError::VaultLocked);
        }

        Ok(self.data_ref().trash.iter().map(|t| {
            TrashedEntryPreview {
                id: t.entry.id,
                title: t.entry.title.clone(),
                deleted_at: t.deleted_at,
                days_until_permanent: 30 - (chrono::Utc::now() - t.deleted_at).num_days(),
            }
        }).collect())
    }

    // Internal helpers for mutable/immutable data access
    pub(crate) fn data_mut(&mut self) -> &mut VaultData {
        &mut self.data
    }

    pub(crate) fn data_ref(&self) -> &VaultData {
        &self.data
    }
}

/// Lightweight trash entry info for the UI.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TrashedEntryPreview {
    pub id: Uuid,
    pub title: String,
    pub deleted_at: chrono::DateTime<chrono::Utc>,
    pub days_until_permanent: i64,
}

