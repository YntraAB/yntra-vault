//! Attachment operations for VaultManager.
//!
//! Handles encrypted file attachment retrieval, addition, and deletion.

use chrono::Utc;
use uuid::Uuid;

use crate::error::VaultError;
use crate::vault::manager::VaultManager;
use crate::vault::types::{AttachmentInfo, FieldScope, FileAttachment};

impl VaultManager {
    /// Decrypt and return the raw byte payload of a file attachment.
    pub fn get_attachment_data(&self, entry_id: Uuid, attachment_id: Uuid) -> crate::Result<Vec<u8>> {
        let keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;
        let entry = self
            .data
            .entries
            .iter()
            .find(|e| e.id == entry_id)
            .ok_or(VaultError::EntryNotFound(entry_id.to_string()))?;

        let attachment = entry
            .attachments
            .iter()
            .find(|a| a.id == attachment_id)
            .ok_or_else(|| VaultError::InvalidFormat(format!("Attachment {} not found", attachment_id)))?;

        let data = Self::decrypt_entry_field(
            &attachment.encrypted_blob,
            &keys.entry_key,
            &entry_id,
            FieldScope::Attachment {
                attachment_id: &attachment_id,
            },
        )?;

        Ok(data.to_vec())
    }

    /// Add an attachment directly to an existing unlocked entry.
    pub fn add_attachment(
        &mut self,
        entry_id: Uuid,
        name: &str,
        mime_type: &str,
        data: &[u8],
    ) -> crate::Result<AttachmentInfo> {
        let entry_key = self.keys.as_ref().ok_or(VaultError::VaultLocked)?.entry_key.clone();
        let now = Utc::now();
        let attachment_id = Uuid::new_v4();

        let encrypted_blob = Self::encrypt_entry_field(
            data,
            &entry_key,
            &entry_id,
            FieldScope::Attachment {
                attachment_id: &attachment_id,
            },
        )?;

        let attachment = FileAttachment {
            id: attachment_id,
            name: name.to_string(),
            size: data.len() as u64,
            mime_type: mime_type.to_string(),
            created_at: now,
            encrypted_blob,
        };

        let info = AttachmentInfo {
            id: attachment_id,
            name: name.to_string(),
            size: data.len() as u64,
            mime_type: mime_type.to_string(),
            created_at: now,
        };

        let entry = self
            .data
            .entries
            .iter_mut()
            .find(|e| e.id == entry_id)
            .ok_or(VaultError::EntryNotFound(entry_id.to_string()))?;

        entry.attachments.push(attachment);
        entry.updated_at = now;
        self.save()?;

        Ok(info)
    }

    /// Delete an attachment from an existing unlocked entry.
    pub fn delete_attachment(&mut self, entry_id: Uuid, attachment_id: Uuid) -> crate::Result<()> {
        let entry = self
            .data
            .entries
            .iter_mut()
            .find(|e| e.id == entry_id)
            .ok_or(VaultError::EntryNotFound(entry_id.to_string()))?;

        let pos = entry
            .attachments
            .iter()
            .position(|a| a.id == attachment_id)
            .ok_or_else(|| VaultError::InvalidFormat(format!("Attachment {} not found", attachment_id)))?;

        entry.attachments.remove(pos);
        entry.updated_at = Utc::now();
        self.save()?;

        Ok(())
    }
}
