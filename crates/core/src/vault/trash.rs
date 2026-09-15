//! Trash and soft-deletion operations for VaultManager.
//!
//! Handles moving entries to trash, listing recoverable trashed items,
//! restoring entries, permanent deletion, and vault storage compaction.

use std::collections::HashSet;
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

/// Detailed encrypted storage footprint metrics for a vault.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct VaultStorageMetrics {
    pub entry_count: usize,
    pub trashed_entry_count: usize,
    pub tag_count: usize,
    pub active_attachment_count: usize,
    pub active_attachment_bytes: u64,
    pub trashed_attachment_count: usize,
    pub trashed_attachment_bytes: u64,
    pub vault_file_bytes: u64,
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

    /// Count of items currently residing in the trash.
    pub fn trash_count(&self) -> crate::Result<usize> {
        if !self.is_unlocked() {
            return Err(VaultError::VaultLocked);
        }
        Ok(self.data.trash.len())
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
        self.purge_all_trash().map(|_| ())
    }

    /// Permanently empty all entries from trash and return the count of deleted entries.
    pub fn purge_all_trash(&mut self) -> crate::Result<usize> {
        if !self.is_unlocked() {
            return Err(VaultError::VaultLocked);
        }
        let count = self.data.trash.len();
        self.data.trash.clear();
        self.save()?;
        Ok(count)
    }

    /// Purge expired entries from trash whose retention exceeds `max_age_days`.
    /// Returns the number of permanently removed items.
    pub fn purge_expired_trash(&mut self, max_age_days: i64) -> crate::Result<usize> {
        if !self.is_unlocked() {
            return Err(VaultError::VaultLocked);
        }
        let cutoff = Utc::now() - chrono::Duration::days(max_age_days);
        let before_count = self.data.trash.len();
        self.data.trash.retain(|t| t.deleted_at > cutoff);
        let purged = before_count - self.data.trash.len();
        if purged > 0 {
            self.save()?;
        }
        Ok(purged)
    }

    /// Calculate aggregate storage metrics for entries, attachments, folders, and on-disk payload.
    pub fn get_storage_metrics(&self) -> crate::Result<VaultStorageMetrics> {
        if !self.is_unlocked() {
            return Err(VaultError::VaultLocked);
        }

        let mut active_attachment_count = 0;
        let mut active_attachment_bytes = 0u64;
        let mut all_tags = HashSet::new();

        for entry in &self.data.entries {
            active_attachment_count += entry.attachments.len();
            for att in &entry.attachments {
                active_attachment_bytes += att.size;
            }
            for tag in &entry.tags {
                all_tags.insert(tag.clone());
            }
        }

        let mut trashed_attachment_count = 0;
        let mut trashed_attachment_bytes = 0u64;
        for trashed in &self.data.trash {
            trashed_attachment_count += trashed.entry.attachments.len();
            for att in &trashed.entry.attachments {
                trashed_attachment_bytes += att.size;
            }
        }

        let vault_file_bytes = std::fs::metadata(&self.path)
            .map(|m| m.len())
            .unwrap_or(0);

        Ok(VaultStorageMetrics {
            entry_count: self.data.entries.len(),
            trashed_entry_count: self.data.trash.len(),
            tag_count: all_tags.len(),
            active_attachment_count,
            active_attachment_bytes,
            trashed_attachment_count,
            trashed_attachment_bytes,
            vault_file_bytes,
        })
    }

    /// Perform a full storage compaction: purges expired trash items, re-indexes
    /// zero-disclosure search terms, writes an atomic re-packed vault file, and returns
    /// storage metrics.
    pub fn compact_vault(&mut self) -> crate::Result<VaultStorageMetrics> {
        if !self.is_unlocked() {
            return Err(VaultError::VaultLocked);
        }

        self.purge_expired_trash(30)?;
        self.rebuild_search_index();
        self.save()?;
        self.get_storage_metrics()
    }
}
