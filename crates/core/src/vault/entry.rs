//! Entry-level operations, search filtering, and CRUD for VaultManager.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

use crate::crypto::cipher::EncryptedBlob;
use crate::crypto::EntryKey;
use crate::error::VaultError;
use crate::vault::manager::VaultManager;
use crate::vault::types::*;
pub use crate::vault::trash::TrashedEntryPreview;

// ─── DTOs for frontend communication ────────────────────────────────

/// Data for creating a new entry.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NewEntry {
    pub title: String,
    pub username: String,
    pub password: String,
    pub url: String,
    pub email: String,
    pub notes: String,
    pub tags: Vec<String>,
    pub totp_secret: Option<String>,
    pub custom_fields: Vec<CustomField>,
    pub entry_type: Option<EntryType>,
    /// If true, auto-generate an ES256 passkey keypair for this entry
    pub generate_passkey: Option<bool>,
    /// Optional file attachments to create along with entry
    #[serde(default)]
    pub attachments: Option<Vec<NewAttachment>>,
}

/// Data for updating an existing entry (all fields optional).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct UpdateEntry {
    pub title: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub url: Option<String>,
    pub email: Option<String>,
    pub notes: Option<String>,
    pub tags: Option<Vec<String>>,
    pub favorite: Option<bool>,
    pub pinned: Option<bool>,
    pub totp_secret: Option<String>,
    pub custom_fields: Option<Vec<CustomField>>,
    pub breach_status: Option<BreachStatus>,
    /// "generate" to create new passkey, "remove" to delete existing
    pub passkey_action: Option<String>,
    /// Staged new file attachments to encrypt and add
    #[serde(default)]
    pub new_attachments: Option<Vec<NewAttachment>>,
    /// Attachment IDs to delete from the entry
    #[serde(default)]
    pub delete_attachment_ids: Option<Vec<Uuid>>,
}

/// Fully decrypted entry for the detail view.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DecryptedEntry {
    pub id: Uuid,
    pub title: String,
    pub username: String,
    pub password: String,
    pub url: String,
    pub email: String,
    pub notes: String,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub pinned: bool,
    pub totp_secret: Option<String>,
    pub custom_fields: Vec<CustomField>,
    pub entry_type: EntryType,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
    pub password_changed_at: chrono::DateTime<Utc>,
    pub breach_status: BreachStatus,
    pub strength_score: Option<StrengthScore>,
    pub password_history_count: usize,
    pub has_passkey: bool,
    pub passkey_public_key: Option<Vec<u8>>,
    #[serde(default)]
    pub attachments: Vec<AttachmentInfo>,
}

impl VaultManager {
    /// Get all entry previews (no password decryption needed).
    pub fn list_entries(&self) -> crate::Result<Vec<EntryPreview>> {
        if !self.is_unlocked() {
            return Err(VaultError::VaultLocked);
        }

        Ok(self
            .data
            .entries
            .iter()
            .map(|e| {
                let age = (Utc::now() - e.password_changed_at).num_days();
                EntryPreview {
                    id: e.id,
                    title: e.title.clone(),
                    username: e.username.clone(),
                    url: e.url.clone(),
                    email: e.email.clone(),
                    tags: e.tags.clone(),
                    favorite: e.favorite,
                    pinned: e.pinned,
                    has_totp: e.encrypted_totp_secret.is_some(),
                    entry_type: e.entry_type.clone(),
                    updated_at: e.updated_at,
                    breach_status: e.breach_status.clone(),
                    strength_score: e.strength_score.clone(),
                    password_age_days: age,
                    has_passkey: e.encrypted_passkey.is_some(),
                    attachment_count: e.attachments.len(),
                }
            })
            .collect())
    }

    /// Executes a closure with a zero-allocation stack-buffered length-prefixed AAD for field encryption/decryption.
    /// Format: [16-byte raw UUID] || [u16_BE(scope_len)] || [scope_bytes]
    pub(crate) fn with_entry_field_aad<'a, F, R>(
        entry_id: &Uuid,
        field_scope: impl Into<FieldScope<'a>>,
        f: F,
    ) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        let scope: FieldScope<'a> = field_scope.into();
        const MAX_STACK_SCOPE: usize = 64;
        let scope_cow = scope.as_cow();
        let scope_bytes = scope_cow.as_bytes();
        let scope_len = scope_bytes.len();
        let scope_len_be = (scope_len as u16).to_be_bytes();

        if scope_len <= MAX_STACK_SCOPE {
            let mut stack_buf = [0u8; 16 + 2 + MAX_STACK_SCOPE];
            stack_buf[..16].copy_from_slice(entry_id.as_bytes());
            stack_buf[16..18].copy_from_slice(&scope_len_be);
            let end = 18 + scope_len;
            stack_buf[18..end].copy_from_slice(scope_bytes);
            let res = f(&stack_buf[..end]);
            stack_buf.zeroize();
            res
        } else {
            let mut aad = Vec::with_capacity(16 + 2 + scope_len);
            aad.extend_from_slice(entry_id.as_bytes());
            aad.extend_from_slice(&scope_len_be);
            aad.extend_from_slice(scope_bytes);
            let res = f(&aad);
            aad.zeroize();
            res
        }
    }

    /// Legacy un-prefixed AAD builder [16-byte raw UUID] || [scope_bytes] for backwards compatibility fallback.
    pub(crate) fn with_entry_field_aad_legacy<'a, F, R>(
        entry_id: &Uuid,
        field_scope: impl Into<FieldScope<'a>>,
        f: F,
    ) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        let scope: FieldScope<'a> = field_scope.into();
        const MAX_STACK_SCOPE: usize = 64;
        let scope_cow = scope.as_cow();
        let scope_bytes = scope_cow.as_bytes();
        if scope_bytes.len() <= MAX_STACK_SCOPE {
            let mut stack_buf = [0u8; 16 + MAX_STACK_SCOPE];
            stack_buf[..16].copy_from_slice(entry_id.as_bytes());
            let end = 16 + scope_bytes.len();
            stack_buf[16..end].copy_from_slice(scope_bytes);
            let res = f(&stack_buf[..end]);
            stack_buf.zeroize();
            res
        } else {
            let mut aad = Vec::with_capacity(16 + scope_bytes.len());
            aad.extend_from_slice(entry_id.as_bytes());
            aad.extend_from_slice(scope_bytes);
            let res = f(&aad);
            aad.zeroize();
            res
        }
    }

    pub(crate) fn encrypt_entry_field<'a>(
        plaintext: &[u8],
        master_entry_key: &EntryKey,
        entry_id: &Uuid,
        field_scope: impl Into<FieldScope<'a>>,
    ) -> crate::Result<EncryptedBlob> {
        let per_entry_key = crate::crypto::derive_per_entry_key(master_entry_key, entry_id)?;
        let blob = Self::with_entry_field_aad(entry_id, field_scope, |aad| {
            crate::crypto::encrypt_entry_with_aad(plaintext, &per_entry_key, aad)
        })?;
        Ok(blob)
    }

    pub fn decrypt_entry_field<'a>(
        blob: &EncryptedBlob,
        master_entry_key: &EntryKey,
        entry_id: &Uuid,
        field_scope: impl Into<FieldScope<'a>> + Copy,
    ) -> crate::Result<Zeroizing<Vec<u8>>> {
        let per_entry_key = crate::crypto::derive_per_entry_key(master_entry_key, entry_id)?;

        // 1. Try per-entry HKDF key + length-prefixed AAD
        if let Ok(bytes) = Self::with_entry_field_aad(entry_id, field_scope, |aad| {
            crate::crypto::decrypt_entry_with_aad(blob, &per_entry_key, aad)
        }) {
            return Ok(bytes);
        }

        // 2. Try global entry key + length-prefixed AAD
        if let Ok(bytes) = Self::with_entry_field_aad(entry_id, field_scope, |aad| {
            crate::crypto::decrypt_entry_with_aad(blob, master_entry_key, aad)
        }) {
            return Ok(bytes);
        }

        // 3. Fallback for legacy un-prefixed AAD (per-entry key)
        if let Ok(bytes) = Self::with_entry_field_aad_legacy(entry_id, field_scope, |aad| {
            crate::crypto::decrypt_entry_with_aad(blob, &per_entry_key, aad)
        }) {
            return Ok(bytes);
        }

        // 4. Fallback for legacy un-prefixed AAD (global entry key)
        let bytes = Self::with_entry_field_aad_legacy(entry_id, field_scope, |aad| {
            crate::crypto::decrypt_entry_with_aad(blob, master_entry_key, aad)
        })?;
        Ok(bytes)
    }

    /// Get a full entry with decrypted password.
    pub fn get_entry(&self, id: Uuid) -> crate::Result<DecryptedEntry> {
        let keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;

        let entry = self
            .data
            .entries
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| VaultError::EntryNotFound(id.to_string()))?;

        // Decrypt password (Layer 2: XChaCha20-Poly1305 with per-entry key + AAD)
        let password_bytes =
            Self::decrypt_entry_field(&entry.encrypted_password, &keys.entry_key, &entry.id, "password")?;
        let password = String::from_utf8(password_bytes.to_vec())
            .map_err(|e| VaultError::DecryptionError(format!("Invalid UTF-8 password: {}", e)))?;

        // Decrypt TOTP secret if present
        let totp_secret = if let Some(ref encrypted_totp) = entry.encrypted_totp_secret {
            let bytes = Self::decrypt_entry_field(encrypted_totp, &keys.entry_key, &entry.id, "totp")?;
            Some(
                String::from_utf8(bytes.to_vec())
                    .map_err(|e| VaultError::DecryptionError(format!("Invalid UTF-8 TOTP: {}", e)))?,
            )
        } else {
            None
        };

        let attachments = entry
            .attachments
            .iter()
            .map(|att| AttachmentInfo {
                id: att.id,
                name: att.name.clone(),
                size: att.size,
                mime_type: att.mime_type.clone(),
                created_at: att.created_at,
            })
            .collect();

        Ok(DecryptedEntry {
            id: entry.id,
            title: entry.title.clone(),
            username: entry.username.clone(),
            password,
            url: entry.url.clone(),
            email: entry.email.clone(),
            notes: entry.notes.clone(),
            tags: entry.tags.clone(),
            favorite: entry.favorite,
            pinned: entry.pinned,
            totp_secret,
            custom_fields: entry.custom_fields.clone(),
            entry_type: entry.entry_type.clone(),
            created_at: entry.created_at,
            updated_at: entry.updated_at,
            password_changed_at: entry.password_changed_at,
            breach_status: entry.breach_status.clone(),
            strength_score: entry.strength_score.clone(),
            password_history_count: entry.password_history.len(),
            has_passkey: entry.encrypted_passkey.is_some(),
            passkey_public_key: entry.passkey_public_key.clone(),
            attachments,
        })
    }

    /// Decrypt the passkey private key for an entry.
    pub fn get_passkey_private_key(&self, id: Uuid) -> crate::Result<Vec<u8>> {
        let keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;
        let entry = self
            .data
            .entries
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| VaultError::EntryNotFound(id.to_string()))?;

        let enc_passkey = entry
            .encrypted_passkey
            .as_ref()
            .ok_or_else(|| VaultError::InvalidFormat("Entry has no passkey".into()))?;

        let bytes = Self::decrypt_entry_field(
            enc_passkey,
            &keys.entry_key,
            &id,
            FieldScope::Passkey,
        )?;
        Ok(bytes.to_vec())
    }

    /// Add a new entry to the vault.
    pub fn add_entry(&mut self, new: NewEntry) -> crate::Result<Uuid> {
        crate::vault::validation::validate_display_name(&new.title)?;
        for field in &new.custom_fields {
            crate::vault::validation::validate_display_name(&field.name)?;
        }
        let keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;

        let now = Utc::now();
        let id = Uuid::new_v4();

        // Encrypt password (Layer 2: XChaCha20-Poly1305 with per-entry key + AAD)
        let encrypted_password = Self::encrypt_entry_field(
            new.password.as_bytes(),
            &keys.entry_key,
            &id,
            FieldScope::Password,
        )?;

        // Encrypt TOTP secret if provided
        let encrypted_totp = if let Some(ref secret) = new.totp_secret {
            Some(Self::encrypt_entry_field(
                secret.as_bytes(),
                &keys.entry_key,
                &id,
                FieldScope::Totp,
            )?)
        } else {
            None
        };

        // Encrypt staged file attachments if provided
        let mut attachments = Vec::new();
        if let Some(new_atts) = new.attachments {
            for att in new_atts {
                let att_id = Uuid::new_v4();
                let encrypted_blob = Self::encrypt_entry_field(
                    &att.data,
                    &keys.entry_key,
                    &id,
                    FieldScope::Attachment {
                        attachment_id: &att_id,
                    },
                )?;
                attachments.push(FileAttachment {
                    id: att_id,
                    name: att.name,
                    size: att.data.len() as u64,
                    mime_type: att.mime_type,
                    created_at: now,
                    encrypted_blob,
                });
            }
        }

        let mut entry = Entry {
            id,
            title: new.title,
            username: new.username,
            encrypted_password,
            url: new.url,
            email: new.email,
            notes: new.notes,
            tags: new.tags,
            favorite: false,
            pinned: false,
            encrypted_totp_secret: encrypted_totp,
            custom_fields: new.custom_fields,
            entry_type: new.entry_type.unwrap_or_default(),
            created_at: now,
            updated_at: now,
            password_history: Vec::new(),
            breach_status: BreachStatus::Unknown,
            strength_score: if new.password.is_empty() {
                None
            } else {
                Some(crate::breach::strength::analyze_password(&new.password))
            },
            password_changed_at: now,
            encrypted_passkey: None,
            passkey_public_key: None,
            attachments,
        };

        // Generate passkey if requested (single keypair for both fields)
        if new.generate_passkey.unwrap_or(false) {
            let pair = crate::crypto::passkey::generate_passkey_pair()?;
            entry.encrypted_passkey = Some(Self::encrypt_entry_field(
                &pair.private_key,
                &keys.entry_key,
                &id,
                FieldScope::Passkey,
            )?);
            entry.passkey_public_key = Some(pair.public_key);
        }

        self.add_entry_to_index(
            entry.id,
            &entry.title,
            &entry.username,
            &entry.url,
            &entry.email,
            &entry.tags,
        );
        self.data.entries.push(entry);
        self.save()?;

        Ok(id)
    }

    /// Update an existing entry. Tracks password history.
    /// Does not update `updated_at` or trigger disk write if only non-content metadata (e.g. breach status) changed.
    pub fn update_entry(&mut self, id: Uuid, update: UpdateEntry) -> crate::Result<()> {
        let entry_key = self.keys.as_ref().ok_or(VaultError::VaultLocked)?.entry_key.clone();
        let now = Utc::now();
        let mut content_changed = false;
        let mut search_index_changed = false;

        // 1. Perform modifications in a nested block to drop `entry` borrow
        {
            let entry = self
                .data
                .entries
                .iter_mut()
                .find(|e| e.id == id)
                .ok_or_else(|| VaultError::EntryNotFound(id.to_string()))?;

            // If password changed, save old one to history and reset breach status
            if let Some(title) = &update.title {
                if title != &entry.title {
                    crate::vault::validation::validate_display_name(title)?;
                }
            }
            if let Some(fields) = &update.custom_fields {
                for field in fields {
                    if !entry.custom_fields.iter().any(|old| old.id == field.id && old.name == field.name) {
                        crate::vault::validation::validate_display_name(&field.name)?;
                    }
                }
            }
            if let Some(ref new_password) = update.password {
                let old_password_bytes = Self::decrypt_entry_field(
                    &entry.encrypted_password,
                    &entry_key,
                    &id,
                    FieldScope::Password,
                )?;
                if old_password_bytes.as_slice() != new_password.as_bytes() {
                    content_changed = true;
                    // Save current password to history before overwriting (encrypted under scope "history")
                    let history_encrypted = Self::encrypt_entry_field(
                        old_password_bytes.as_slice(),
                        &entry_key,
                        &id,
                        FieldScope::History,
                    )?;
                    let history_item = PasswordHistoryItem {
                        encrypted_password: history_encrypted,
                        changed_at: entry.password_changed_at,
                    };
                    entry.password_history.push(history_item);

                    // Keep only last N entries
                    if entry.password_history.len() > MAX_PASSWORD_HISTORY {
                        entry.password_history.remove(0);
                    }

                    entry.encrypted_password = Self::encrypt_entry_field(
                        new_password.as_bytes(),
                        &entry_key,
                        &id,
                        FieldScope::Password,
                    )?;
                    entry.password_changed_at = now;
                    entry.breach_status = BreachStatus::Unknown; // Reset breach status
                    entry.strength_score = if new_password.is_empty() {
                        None
                    } else {
                        Some(crate::breach::strength::analyze_password(new_password))
                    };
                }
            }

            if let Some(title) = update.title
                && entry.title != title {
                    entry.title = title;
                    content_changed = true;
                    search_index_changed = true;
                }
            if let Some(username) = update.username
                && entry.username != username {
                    entry.username = username;
                    content_changed = true;
                    search_index_changed = true;
                }
            if let Some(url) = update.url
                && entry.url != url {
                    entry.url = url;
                    content_changed = true;
                    search_index_changed = true;
                }
            if let Some(email) = update.email
                && entry.email != email {
                    entry.email = email;
                    content_changed = true;
                    search_index_changed = true;
                }
            if let Some(notes) = update.notes
                && entry.notes != notes {
                    entry.notes = notes;
                    content_changed = true;
                }
            if let Some(tags) = update.tags
                && entry.tags != tags {
                    entry.tags = tags;
                    content_changed = true;
                    search_index_changed = true;
                }
            if let Some(fav) = update.favorite
                && entry.favorite != fav {
                    entry.favorite = fav;
                    content_changed = true;
                }
            if let Some(pin) = update.pinned
                && entry.pinned != pin {
                    entry.pinned = pin;
                    content_changed = true;
                }
            if let Some(fields) = update.custom_fields
                && entry.custom_fields != fields {
                    entry.custom_fields = fields;
                    content_changed = true;
                }
            if let Some(breach) = update.breach_status {
                // Breach status updates are advisory metadata and do NOT count as content changes.
                entry.breach_status = breach;
            }

            // Update TOTP secret
            if let Some(ref totp_secret) = update.totp_secret {
                let is_different = match &entry.encrypted_totp_secret {
                    None => !totp_secret.is_empty(),
                    Some(blob) => {
                        let old_totp = Self::decrypt_entry_field(blob, &entry_key, &id, FieldScope::Totp)
                            .map(|b| String::from_utf8_lossy(&b).to_string())
                            .unwrap_or_default();
                        &old_totp != totp_secret
                    }
                };
                if is_different {
                    content_changed = true;
                    if totp_secret.is_empty() {
                        entry.encrypted_totp_secret = None;
                    } else {
                        entry.encrypted_totp_secret = Some(Self::encrypt_entry_field(
                            totp_secret.as_bytes(),
                            &entry_key,
                            &id,
                            FieldScope::Totp,
                        )?);
                    }
                }
            }

            // Handle passkey: generate new, or remove existing
            if let Some(ref action) = update.passkey_action {
                match action.as_str() {
                    "generate" => {
                        content_changed = true;
                        let pair = crate::crypto::passkey::generate_passkey_pair()?;
                        entry.encrypted_passkey = Some(Self::encrypt_entry_field(
                            &pair.private_key,
                            &entry_key,
                            &id,
                            FieldScope::Passkey,
                        )?);
                        entry.passkey_public_key = Some(pair.public_key);
                    }
                    "remove"
                        if (entry.encrypted_passkey.is_some() || entry.passkey_public_key.is_some()) => {
                            content_changed = true;
                            entry.encrypted_passkey = None;
                            entry.passkey_public_key = None;
                        }
                    _ => {}
                }
            }

            // Handle deleting attachments if requested
            if let Some(ref del_ids) = update.delete_attachment_ids
                && !del_ids.is_empty() {
                    let prev_len = entry.attachments.len();
                    entry.attachments.retain(|att| !del_ids.contains(&att.id));
                    if entry.attachments.len() != prev_len {
                        content_changed = true;
                    }
                }

            // Handle adding new staged attachments if requested
            if let Some(new_atts) = update.new_attachments
                && !new_atts.is_empty() {
                    content_changed = true;
                    for att in new_atts {
                        let att_id = Uuid::new_v4();
                        let encrypted_blob = Self::encrypt_entry_field(
                            &att.data,
                            &entry_key,
                            &id,
                            FieldScope::Attachment {
                                attachment_id: &att_id,
                            },
                        )?;
                        entry.attachments.push(FileAttachment {
                            id: att_id,
                            name: att.name,
                            size: att.data.len() as u64,
                            mime_type: att.mime_type,
                            created_at: now,
                            encrypted_blob,
                        });
                    }
                }

            if content_changed {
                entry.updated_at = now.max(entry.updated_at + chrono::Duration::nanoseconds(1));
            }
        }

        // 2. Update search index and save only if actual content changed
        if content_changed {
            if search_index_changed {
                self.remove_entry_from_index(id);

                let (title, username, url, email, tags) = {
                    let entry = self
                        .data
                        .entries
                        .iter()
                        .find(|e| e.id == id)
                        .ok_or_else(|| VaultError::EntryNotFound(id.to_string()))?;
                    (
                        entry.title.clone(),
                        entry.username.clone(),
                        entry.url.clone(),
                        entry.email.clone(),
                        entry.tags.clone(),
                    )
                };

                self.add_entry_to_index(id, &title, &username, &url, &email, &tags);
            }

            self.save()?;
        }

        Ok(())
    }

    /// Update an entry's breach status in memory without altering updated_at or writing to disk.
    pub fn update_entry_breach_status(&mut self, id: Uuid, breach_status: BreachStatus) -> crate::Result<()> {
        let entry = self
            .data
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or_else(|| VaultError::EntryNotFound(id.to_string()))?;
        entry.breach_status = breach_status;
        Ok(())
    }

    /// Soft-delete: Move entry to trash (recoverable for 30 days).
    pub fn delete_entry(&mut self, id: Uuid) -> crate::Result<()> {
        if !self.is_unlocked() {
            return Err(VaultError::VaultLocked);
        }

        let pos = self
            .data
            .entries
            .iter()
            .position(|e| e.id == id)
            .ok_or_else(|| VaultError::EntryNotFound(id.to_string()))?;

        let entry = self.data.entries.remove(pos);
        self.remove_entry_from_index(id);

        // Move to trash instead of permanent delete
        self.data.trash.push(TrashedEntry {
            entry,
            deleted_at: Utc::now(),
        });

        self.save()?;
        Ok(())
    }

    /// Get entries filtered by tag name.
    pub fn entries_by_tag(&self, tag_name: &str) -> crate::Result<Vec<EntryPreview>> {
        let all = self.list_entries()?;
        Ok(all.into_iter().filter(|e| e.tags.iter().any(|t| t == tag_name)).collect())
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

        let entry = self
            .data_mut()
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or_else(|| crate::error::VaultError::EntryNotFound(id.to_string()))?;

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

        let entry = self
            .data_mut()
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or_else(|| crate::error::VaultError::EntryNotFound(id.to_string()))?;

        entry.pinned = !entry.pinned;
        let new_state = entry.pinned;
        self.save()?;
        Ok(new_state)
    }

    // Internal helpers for mutable/immutable data access
    pub(crate) fn data_mut(&mut self) -> &mut VaultData {
        &mut self.data
    }

    pub(crate) fn data_ref(&self) -> &VaultData {
        &self.data
    }
}
