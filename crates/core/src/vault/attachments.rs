//! Attachment operations for VaultManager.
//!
//! Handles encrypted file attachment retrieval, addition, and deletion.

use chrono::Utc;
use uuid::Uuid;

use crate::error::VaultError;
use crate::vault::manager::VaultManager;
use crate::vault::types::{AttachmentInfo, FieldScope, FileAttachment};

/// Maximum allowed file attachment size (25 MB) to prevent out-of-memory errors during vault MessagePack serialization.
pub const MAX_ATTACHMENT_SIZE: usize = 25 * 1024 * 1024;

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
        if data.len() > MAX_ATTACHMENT_SIZE {
            return Err(VaultError::InvalidFormat(format!(
                "Attachment size ({} bytes) exceeds maximum allowed limit of {} bytes (25 MB)",
                data.len(),
                MAX_ATTACHMENT_SIZE
            )));
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_attachment_max_size_enforcement() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("attachments.vdb");
        let mut mgr = VaultManager::create("Test Vault", "TestPass#123", &db_path).unwrap();

        let entry = mgr.add_entry(crate::vault::manager::NewEntry {
            title: "Test Entry".into(),
            username: "user".into(),
            password: "pass".into(),
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

        // Valid small attachment succeeds
        let valid_data = b"small attachment payload";
        let info = mgr.add_attachment(entry, "notes.txt", "text/plain", valid_data).unwrap();
        assert_eq!(info.name, "notes.txt");

        // Oversized attachment (> 25MB) fails with InvalidFormat
        let oversized = vec![0u8; MAX_ATTACHMENT_SIZE + 1];
        let err = mgr.add_attachment(entry, "huge.bin", "application/octet-stream", &oversized);
        assert!(err.is_err());
        match err.err().unwrap() {
            VaultError::InvalidFormat(msg) => {
                assert!(msg.contains("exceeds maximum allowed limit"));
            }
            other => panic!("Expected InvalidFormat for oversized attachment, got {:?}", other),
        }
    }
}
