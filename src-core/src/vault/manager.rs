//! Vault Manager — orchestrates all vault operations
//!
//! Handles create, open, save, lock with full multi-layer encryption.

use std::path::{Path, PathBuf};
use std::fs;
use chrono::Utc;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

use crate::crypto::{
    derive_master_key, derive_subkeys,
    encrypt_vault, decrypt_vault,
    compute_hmac, verify_hmac,
    SubKeys,
};
use crate::crypto::kdf::generate_salt;
use crate::vault::format::{VaultFile, FileHeader, KdfParams, FORMAT_VERSION};
use crate::vault::types::*;
use crate::error::VaultError;
use zeroize::Zeroize;


/// Active vault state — holds decrypted data + derived keys.
pub struct VaultManager {
    /// Path to the .vdb file
    pub(crate) path: PathBuf,
    /// Decrypted vault contents
    pub(crate) data: VaultData,
    /// Derived subkeys (zeroed on lock)
    pub(crate) keys: Option<SubKeys>,
    /// Salt from the file header
    pub(crate) salt: [u8; 32],
    /// In-memory Zero-Disclosure search index
    pub(crate) search_index: std::collections::HashMap<[u8; 8], Vec<Uuid>>,
}

impl VaultManager {
    /// Create a brand new vault with the given master password.
    pub fn create(name: &str, password: &str, path: &Path) -> crate::Result<Self> {
        let salt = generate_salt();

        // Derive keys from master password
        let master_key = derive_master_key(password.as_bytes(), &salt)?;
        let subkeys = derive_subkeys(&master_key)?;

        let now = Utc::now();
        let vault_id = Uuid::new_v4();

        let data = VaultData {
            metadata: VaultMetadata {
                id: vault_id,
                name: name.to_string(),
                created_at: now,
                updated_at: now,
                entry_count: 0,
                version: FORMAT_VERSION,
            },
            entries: Vec::new(),
            tags: Vec::new(),
            trash: Vec::new(),
        };

        let mut manager = VaultManager {
            path: path.to_path_buf(),
            data,
            keys: Some(subkeys),
            salt,
            search_index: std::collections::HashMap::new(),
        };

        // Rebuild index
        manager.rebuild_search_index();

        // Save to disk
        manager.save()?;

        Ok(manager)
    }

    /// Open an existing vault with the master password.
    pub fn open(path: &Path, password: &str) -> crate::Result<Self> {
        // Read file
        let file_bytes = fs::read(path)
            .map_err(|e| VaultError::VaultNotFound(format!("{}: {}", path.display(), e)))?;

        // Parse file format
        let vault_file = VaultFile::from_bytes(&file_bytes)?;

        // Derive keys from password + stored salt
        let master_key = derive_master_key(password.as_bytes(), &vault_file.header.salt)?;
        let subkeys = derive_subkeys(&master_key)?;

        // Verify HMAC integrity FIRST (detect tampering before decryption)
        verify_hmac(
            &vault_file.encrypted_payload,
            &vault_file.hmac,
            &subkeys.hmac_key,
        )?;

        // Decrypt vault payload (Layer 1: XChaCha20-Poly1305)
        let encrypted_blob = crate::crypto::cipher::EncryptedBlob {
            nonce: vault_file.encrypted_payload[..24].to_vec(),
            ciphertext: vault_file.encrypted_payload[24..].to_vec(),
        };

        let decrypted = decrypt_vault(&encrypted_blob, &subkeys.vault_key)?;

        // Deserialize vault data
        let data: VaultData = bincode::deserialize(&decrypted)
            .map_err(|e| VaultError::SerializationError(format!("Vault deserialize: {}", e)))?;

        let mut manager = VaultManager {
            path: path.to_path_buf(),
            data,
            keys: Some(subkeys),
            salt: vault_file.header.salt,
            search_index: std::collections::HashMap::new(),
        };
        manager.rebuild_search_index();
        Ok(manager)
    }

    /// Save the vault to disk with full encryption.
    pub fn save(&mut self) -> crate::Result<()> {
        let keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;

        // Update metadata
        self.data.metadata.updated_at = Utc::now();
        self.data.metadata.entry_count = self.data.entries.len();

        // Clean up old trash (> 30 days)
        let cutoff = Utc::now() - chrono::Duration::days(30);
        self.data.trash.retain(|t| t.deleted_at > cutoff);

        // Serialize vault data
        let serialized = bincode::serialize(&self.data)
            .map_err(|e| VaultError::SerializationError(format!("Vault serialize: {}", e)))?;

        // Encrypt (Layer 1: XChaCha20-Poly1305)
        let encrypted = encrypt_vault(&serialized, &keys.vault_key)?;

        // Combine nonce + ciphertext as the payload
        let mut payload = Vec::with_capacity(encrypted.nonce.len() + encrypted.ciphertext.len());
        payload.extend_from_slice(&encrypted.nonce);
        payload.extend_from_slice(&encrypted.ciphertext);

        // Compute HMAC (Layer 3: integrity)
        let hmac = compute_hmac(&payload, &keys.hmac_key);
        let mut hmac_bytes = [0u8; 64];
        hmac_bytes.copy_from_slice(&hmac);

        // Build vault file
        let vault_file = VaultFile {
            header: FileHeader {
                version: FORMAT_VERSION,
                flags: 0,
                salt: self.salt,
                kdf_params: KdfParams::default(),
            },
            hmac: hmac_bytes,
            encrypted_payload: payload,
        };

        // Write to disk atomically (write to temp file, then rename)
        let file_bytes = vault_file.to_bytes()?;
        let temp_path = self.path.with_extension("vdb.tmp");
        fs::write(&temp_path, &file_bytes)?;
        fs::rename(&temp_path, &self.path)?;

        Ok(())
    }

    /// Retrieve the derived SubKeys if the vault is unlocked.
    pub fn get_subkeys(&self) -> crate::Result<&crate::crypto::SubKeys> {
        self.keys.as_ref().ok_or(VaultError::VaultLocked)
    }

    /// Lock the vault — zeroes all keys from memory.
    pub fn lock(&mut self) {
        self.keys = None; // SubKeys implement ZeroizeOnDrop
        self.data.entries.clear();
        self.data.tags.clear();
        self.data.trash.clear();
        self.search_index.clear();
    }

    /// Check if the vault is unlocked.
    pub fn is_unlocked(&self) -> bool {
        self.keys.is_some()
    }

    /// Get vault metadata (always available, even when locked).
    pub fn metadata(&self) -> &VaultMetadata {
        &self.data.metadata
    }

    /// Get vault info for the selection screen.
    pub fn info(&self) -> VaultInfo {
        VaultInfo {
            id: self.data.metadata.id,
            name: self.data.metadata.name.clone(),
            path: self.path.to_string_lossy().to_string(),
            entry_count: self.data.metadata.entry_count,
            last_opened: Some(Utc::now()),
        }
    }

    // ─── Entry Operations ───────────────────────────────────────────

    /// Get all entry previews (no password decryption needed).
    pub fn list_entries(&self) -> crate::Result<Vec<EntryPreview>> {
        if !self.is_unlocked() {
            return Err(VaultError::VaultLocked);
        }

        Ok(self.data.entries.iter().map(|e| {
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
            }
        }).collect())
    }

    /// Get a full entry with decrypted password.
    pub fn get_entry(&self, id: Uuid) -> crate::Result<DecryptedEntry> {
        let keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;

        let entry = self.data.entries.iter()
            .find(|e| e.id == id)
            .ok_or(VaultError::EntryNotFound(id.to_string()))?;

        // Decrypt password (Layer 2: AES-256-GCM)
        let password_bytes = crate::crypto::decrypt_entry(
            &entry.encrypted_password,
            &keys.entry_key,
        )?;
        let password = String::from_utf8(password_bytes)
            .map_err(|e| VaultError::DecryptionError(format!("Invalid UTF-8 password: {}", e)))?;

        // Decrypt TOTP secret if present
        let totp_secret = if let Some(ref encrypted_totp) = entry.encrypted_totp_secret {
            let bytes = crate::crypto::decrypt_entry(encrypted_totp, &keys.entry_key)?;
            Some(String::from_utf8(bytes)
                .map_err(|e| VaultError::DecryptionError(format!("Invalid UTF-8 TOTP: {}", e)))?)
        } else {
            None
        };

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
        })
    }

    /// Add a new entry to the vault.
    pub fn add_entry(&mut self, new: NewEntry) -> crate::Result<Uuid> {
        let keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;

        let now = Utc::now();
        let id = Uuid::new_v4();

        // Encrypt password (Layer 2: AES-256-GCM)
        let encrypted_password = crate::crypto::encrypt_entry(
            new.password.as_bytes(),
            &keys.entry_key,
        )?;

        // Encrypt TOTP secret if provided
        let encrypted_totp = if let Some(ref secret) = new.totp_secret {
            Some(crate::crypto::encrypt_entry(secret.as_bytes(), &keys.entry_key)?)
        } else {
            None
        };

        let entry = Entry {
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
            strength_score: Some(crate::breach::strength::analyze_password(&new.password)),
            password_changed_at: now,
        };

        self.add_entry_to_index(entry.id, &entry.title, &entry.username, &entry.url, &entry.email, &entry.tags);
        self.data.entries.push(entry);
        self.save()?;

        Ok(id)
    }

    /// Update an existing entry. Tracks password history.
    pub fn update_entry(&mut self, id: Uuid, update: UpdateEntry) -> crate::Result<()> {
        let entry_key_bytes = self.keys.as_ref().ok_or(VaultError::VaultLocked)?.entry_key.bytes;
        let entry_key = crate::crypto::kdf::EntryKey { bytes: entry_key_bytes };
        let now = Utc::now();

        // 1. Perform modifications in a nested block to drop `entry` borrow
        {
            let entry = self.data.entries.iter_mut()
                .find(|e| e.id == id)
                .ok_or(VaultError::EntryNotFound(id.to_string()))?;

            // If password changed, save old one to history and reset breach status
            if let Some(ref new_password) = update.password {
                let old_password_bytes = crate::crypto::decrypt_entry(
                    &entry.encrypted_password,
                    &entry_key,
                )?;
                let old_password = String::from_utf8(old_password_bytes)
                    .map_err(|e| crate::error::VaultError::DecryptionError(e.to_string()))?;

                if &old_password != new_password {
                    // Save current password to history before overwriting
                    let history_item = PasswordHistoryItem {
                        encrypted_password: entry.encrypted_password.clone(),
                        changed_at: entry.password_changed_at,
                    };
                    entry.password_history.push(history_item);

                    // Keep only last N entries
                    if entry.password_history.len() > MAX_PASSWORD_HISTORY {
                        entry.password_history.remove(0);
                    }

                    entry.encrypted_password = crate::crypto::encrypt_entry(
                        new_password.as_bytes(),
                        &entry_key,
                    )?;
                    entry.password_changed_at = now;
                    entry.breach_status = BreachStatus::Unknown; // Reset breach status
                    entry.strength_score = Some(crate::breach::strength::analyze_password(new_password)); // Recalculate
                }
            }

            if let Some(title) = update.title { entry.title = title; }
            if let Some(username) = update.username { entry.username = username; }
            if let Some(url) = update.url { entry.url = url; }
            if let Some(email) = update.email { entry.email = email; }
            if let Some(notes) = update.notes { entry.notes = notes; }
            if let Some(tags) = update.tags { entry.tags = tags; }
            if let Some(fav) = update.favorite { entry.favorite = fav; }
            if let Some(pin) = update.pinned { entry.pinned = pin; }
            if let Some(fields) = update.custom_fields { entry.custom_fields = fields; }
            if let Some(breach) = update.breach_status { entry.breach_status = breach; }

            // Update TOTP secret
            if let Some(ref totp_secret) = update.totp_secret {
                if totp_secret.is_empty() {
                    entry.encrypted_totp_secret = None;
                } else {
                    entry.encrypted_totp_secret = Some(
                        crate::crypto::encrypt_entry(totp_secret.as_bytes(), &entry_key)?
                    );
                }
            }

            entry.updated_at = now;
        }

        // 2. Update search index and save
        self.remove_entry_from_index(id);

        let (title, username, url, email, tags) = {
            let entry = self.data.entries.iter()
                .find(|e| e.id == id)
                .ok_or(VaultError::EntryNotFound(id.to_string()))?;
            (entry.title.clone(), entry.username.clone(), entry.url.clone(), entry.email.clone(), entry.tags.clone())
        };

        self.add_entry_to_index(id, &title, &username, &url, &email, &tags);

        self.save()?;

        Ok(())
    }

    /// Soft-delete: Move entry to trash (recoverable for 30 days).
    pub fn delete_entry(&mut self, id: Uuid) -> crate::Result<()> {
        if !self.is_unlocked() {
            return Err(VaultError::VaultLocked);
        }

        let pos = self.data.entries.iter().position(|e| e.id == id)
            .ok_or(VaultError::EntryNotFound(id.to_string()))?;

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

    /// Permanently delete from trash.
    pub fn permanent_delete(&mut self, id: Uuid) -> crate::Result<()> {
        self.data.trash.retain(|t| t.entry.id != id);
        self.save()?;
        Ok(())
    }

    /// Restore an entry from trash.
    pub fn restore_from_trash(&mut self, id: Uuid) -> crate::Result<()> {
        let pos = self.data.trash.iter().position(|t| t.entry.id == id)
            .ok_or(VaultError::EntryNotFound(id.to_string()))?;

        let trashed = self.data.trash.remove(pos);
        let e = &trashed.entry;
        self.add_entry_to_index(e.id, &e.title, &e.username, &e.url, &e.email, &e.tags);
        self.data.entries.push(trashed.entry);
        self.save()?;
        Ok(())
    }

    // ─── Tags ───────────────────────────────────────────────────────

    pub fn tags(&self) -> &[Tag] {
        &self.data.tags
    }

    pub fn add_tag(&mut self, name: &str, color: &str, icon: &str) -> crate::Result<Uuid> {
        let id = Uuid::new_v4();
        self.data.tags.push(Tag {
            id,
            name: name.to_string(),
            color: color.to_string(),
            icon: icon.to_string(),
        });
        self.save()?;
        Ok(id)
    }

    pub fn delete_tag(&mut self, id: Uuid) -> crate::Result<()> {
        self.data.tags.retain(|t| t.id != id);
        self.save()?;
        Ok(())
    }

    // ─── Security Audit ─────────────────────────────────────────────

    /// Generate a full security audit report.
    pub fn security_audit(&self) -> crate::Result<SecurityAudit> {
        if !self.is_unlocked() {
            return Err(VaultError::VaultLocked);
        }
        let keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;

        let mut issues = Vec::new();
        let mut breached = 0usize;
        let mut weak = 0usize;
        let mut old = 0usize;
        let mut no_2fa = 0usize;
        let mut reused = 0usize;

        let total = self.data.entries.len();

        // Decrypt all passwords for reuse detection and on-the-fly strength checking
        let mut plain_passwords = Vec::with_capacity(total);
        for entry in &self.data.entries {
            let pwd_bytes = crate::crypto::decrypt_entry(&entry.encrypted_password, &keys.entry_key)?;
            let pwd = String::from_utf8(pwd_bytes)
                .map_err(|e| crate::error::VaultError::DecryptionError(e.to_string()))?;
            plain_passwords.push((entry.id, entry.title.clone(), pwd));
        }

        // Group entries by password to identify reused passwords
        use std::collections::HashMap;
        let mut pwd_map: HashMap<String, Vec<(Uuid, String)>> = HashMap::new();
        for (id, title, pwd) in &plain_passwords {
            pwd_map.entry(pwd.clone())
                .or_default()
                .push((*id, title.clone()));
        }

        for (i, entry) in self.data.entries.iter().enumerate() {
            let (_, _, pwd) = &plain_passwords[i];

            // Breach status
            if let BreachStatus::Breached { breach_count, .. } = &entry.breach_status {
                breached += 1;
                issues.push(SecurityIssue {
                    entry_id: entry.id,
                    entry_title: entry.title.clone(),
                    issue_type: IssueType::Breached,
                    severity: IssueSeverity::Critical,
                    description: format!("Password found in {} data breaches", breach_count),
                });
            }

            // Weak password (fallback to real-time calculation if None)
            let score = match &entry.strength_score {
                Some(score) => score.clone(),
                None => crate::breach::strength::analyze_password(pwd),
            };

            if score.level <= StrengthLevel::Weak {
                weak += 1;
                issues.push(SecurityIssue {
                    entry_id: entry.id,
                    entry_title: entry.title.clone(),
                    issue_type: IssueType::WeakPassword,
                    severity: IssueSeverity::Warning,
                    description: format!("Password strength: {:?} ({:.0} bits entropy)", score.level, score.entropy_bits),
                });
            }

            // Reused password
            if let Some(duplicates) = pwd_map.get(pwd) {
                if duplicates.len() > 1 {
                    reused += 1;
                    let other_services: Vec<String> = duplicates.iter()
                        .filter(|(dup_id, _)| dup_id != &entry.id)
                        .map(|(_, dup_title)| dup_title.clone())
                        .collect();
                    issues.push(SecurityIssue {
                        entry_id: entry.id,
                        entry_title: entry.title.clone(),
                        issue_type: IssueType::ReusedPassword,
                        severity: IssueSeverity::Warning,
                        description: format!("Password is reused on: {}", other_services.join(", ")),
                    });
                }
            }

            // Old password (> 90 days)
            let age_days = (Utc::now() - entry.password_changed_at).num_days();
            if age_days > 90 {
                old += 1;
                issues.push(SecurityIssue {
                    entry_id: entry.id,
                    entry_title: entry.title.clone(),
                    issue_type: IssueType::OldPassword,
                    severity: IssueSeverity::Info,
                    description: format!("Password hasn't been changed in {} days", age_days),
                });
            }

            // Missing 2FA on important accounts
            if entry.encrypted_totp_secret.is_none() && is_important_service(&entry.url) {
                no_2fa += 1;
                issues.push(SecurityIssue {
                    entry_id: entry.id,
                    entry_title: entry.title.clone(),
                    issue_type: IssueType::Missing2FA,
                    severity: IssueSeverity::Warning,
                    description: "This service supports 2FA but none is configured".to_string(),
                });
            }
        }

        // Zeroize decrypted passwords in memory for security
        for (_, _, mut pwd) in plain_passwords {
            pwd.zeroize();
        }
        for (mut pwd, _) in pwd_map {
            pwd.zeroize();
        }

        // Calculate health score (0-100)
        let issue_penalty = (breached * 20 + weak * 10 + reused * 15 + old * 2 + no_2fa * 5) as u8;
        let health_score = 100u8.saturating_sub(issue_penalty);

        Ok(SecurityAudit {
            total_entries: total,
            breached_count: breached,
            weak_count: weak,
            reused_count: reused,
            old_count: old,
            no_2fa_count: no_2fa,
            health_score,
            issues,
        })
    }

    /// Change the master password — re-derives keys and re-encrypts everything.
    pub fn change_master_password(&mut self, current: &str, new_password: &str) -> crate::Result<()> {
        // Verify current password by trying to derive same keys
        let current_mk = derive_master_key(current.as_bytes(), &self.salt)?;
        let current_keys = derive_subkeys(&current_mk)?;

        // Quick check: try decrypting first entry's password (or first trashed entry if entries is empty)
        if let Some(entry) = self.data.entries.first() {
            crate::crypto::decrypt_entry(&entry.encrypted_password, &current_keys.entry_key)
                .map_err(|_| VaultError::InvalidPassword)?;
        } else if let Some(trashed) = self.data.trash.first() {
            crate::crypto::decrypt_entry(&trashed.entry.encrypted_password, &current_keys.entry_key)
                .map_err(|_| VaultError::InvalidPassword)?;
        }

        // Generate new salt
        let new_salt = generate_salt();
        let new_mk = derive_master_key(new_password.as_bytes(), &new_salt)?;
        let new_keys = derive_subkeys(&new_mk)?;

        // Re-encrypt every entry's password and TOTP with new keys
        for entry in &mut self.data.entries {
            // Decrypt with old key, re-encrypt with new key
            let pw_bytes = crate::crypto::decrypt_entry(&entry.encrypted_password, &current_keys.entry_key)?;
            entry.encrypted_password = crate::crypto::encrypt_entry(&pw_bytes, &new_keys.entry_key)?;

            if let Some(ref totp) = entry.encrypted_totp_secret {
                let totp_bytes = crate::crypto::decrypt_entry(totp, &current_keys.entry_key)?;
                entry.encrypted_totp_secret = Some(
                    crate::crypto::encrypt_entry(&totp_bytes, &new_keys.entry_key)?
                );
            }

            // Re-encrypt password history
            for hist in &mut entry.password_history {
                let hist_bytes = crate::crypto::decrypt_entry(&hist.encrypted_password, &current_keys.entry_key)?;
                hist.encrypted_password = crate::crypto::encrypt_entry(&hist_bytes, &new_keys.entry_key)?;
            }
        }

        // Re-encrypt every trashed entry's password and TOTP with new keys
        for trashed in &mut self.data.trash {
            let entry = &mut trashed.entry;
            // Decrypt with old key, re-encrypt with new key
            let pw_bytes = crate::crypto::decrypt_entry(&entry.encrypted_password, &current_keys.entry_key)?;
            entry.encrypted_password = crate::crypto::encrypt_entry(&pw_bytes, &new_keys.entry_key)?;

            if let Some(ref totp) = entry.encrypted_totp_secret {
                let totp_bytes = crate::crypto::decrypt_entry(totp, &current_keys.entry_key)?;
                entry.encrypted_totp_secret = Some(
                    crate::crypto::encrypt_entry(&totp_bytes, &new_keys.entry_key)?
                );
            }

            // Re-encrypt password history
            for hist in &mut entry.password_history {
                let hist_bytes = crate::crypto::decrypt_entry(&hist.encrypted_password, &current_keys.entry_key)?;
                hist.encrypted_password = crate::crypto::encrypt_entry(&hist_bytes, &new_keys.entry_key)?;
            }
        }

        // Update salt and keys
        self.salt = new_salt;
        self.keys = Some(new_keys);
        self.save()?;

        Ok(())
    }

    // ─── Helpers ────────────────────────────────────────────────────

    #[allow(dead_code)]
    fn default_tags() -> Vec<Tag> {
        vec![
            Tag { id: Uuid::new_v4(), name: "Work".into(), color: "#5b8def".into(), icon: "briefcase".into() },
            Tag { id: Uuid::new_v4(), name: "Personal".into(), color: "#5acf7e".into(), icon: "user".into() },
            Tag { id: Uuid::new_v4(), name: "Finance".into(), color: "#f5a623".into(), icon: "credit-card".into() },
            Tag { id: Uuid::new_v4(), name: "Social".into(), color: "#bd7ee8".into(), icon: "users".into() },
            Tag { id: Uuid::new_v4(), name: "Development".into(), color: "#ef6b6b".into(), icon: "code".into() },
        ]
    }
}

/// Helper to check if a service is known to support 2FA.
fn is_important_service(url: &str) -> bool {
    let important = [
        "google", "gmail", "github", "amazon", "aws", "microsoft",
        "apple", "facebook", "twitter", "x.com", "dropbox", "slack",
        "discord", "paypal", "stripe", "cloudflare", "digitalocean",
        "linkedin", "instagram", "reddit", "twitch",
    ];
    let url_lower = url.to_lowercase();
    important.iter().any(|s| url_lower.contains(s))
}

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct TestVault {
        path: PathBuf,
    }

    impl TestVault {
        fn new() -> Self {
            let mut path = std::env::temp_dir();
            path.push(format!("yntra_vault_test_{}.vdb", Uuid::new_v4()));
            TestVault { path }
        }
    }

    impl Drop for TestVault {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    #[test]
    fn test_vault_lifecycle_and_master_password_change() {
        let test_vault = TestVault::new();
        let password = "initial-secure-password";
        
        // 1. Create vault
        let mut manager = VaultManager::create("my-test-vault", password, &test_vault.path).unwrap();
        assert!(manager.is_unlocked());
        assert_eq!(manager.metadata().name, "my-test-vault");
        
        // 2. Add an entry
        let entry1 = NewEntry {
            title: "Service A".to_string(),
            username: "userA".to_string(),
            password: "passwordA-1".to_string(),
            url: "https://a.com".to_string(),
            email: "a@a.com".to_string(),
            notes: "Notes A".to_string(),
            tags: vec!["Work".to_string()],
            totp_secret: Some("JBSWY3DPEHPK3PXP".to_string()),
            custom_fields: Vec::new(),
            entry_type: Some(EntryType::Login),
        };
        
        let id1 = manager.add_entry(entry1).unwrap();
        
        // 3. Update entry to generate history item
        let update = UpdateEntry {
            password: Some("passwordA-2".to_string()),
            ..Default::default()
        };
        manager.update_entry(id1, update).unwrap();
        
        // Check history count
        let history = manager.get_password_history(id1).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].password, "passwordA-1");
        
        // 4. Delete entry (moves it to trash)
        manager.delete_entry(id1).unwrap();
        assert_eq!(manager.list_entries().unwrap().len(), 0);
        assert_eq!(manager.list_trash().unwrap().len(), 1);
        
        // 5. Add second active entry
        let entry2 = NewEntry {
            title: "Service B".to_string(),
            username: "userB".to_string(),
            password: "passwordB-1".to_string(),
            url: "https://b.com".to_string(),
            email: "b@b.com".to_string(),
            notes: "Notes B".to_string(),
            tags: vec!["Personal".to_string()],
            totp_secret: None,
            custom_fields: Vec::new(),
            entry_type: Some(EntryType::Login),
        };
        let id2 = manager.add_entry(entry2).unwrap();
        
        // 6. Change master password
        let new_password = "new-secure-password";
        manager.change_master_password(password, new_password).unwrap();
        
        // 7. Save and lock
        manager.save().unwrap();
        manager.lock();
        assert!(!manager.is_unlocked());
        
        // 8. Re-open with new master password
        let reopened = VaultManager::open(&test_vault.path, new_password).unwrap();
        assert!(reopened.is_unlocked());
        
        // Check active entry
        let dec2 = reopened.get_entry(id2).unwrap();
        assert_eq!(dec2.password, "passwordB-1");
        
        // 9. Restore first entry from trash and verify it decrypts correctly
        let mut reopened_mut = reopened;
        reopened_mut.restore_from_trash(id1).unwrap();
        
        let dec1 = reopened_mut.get_entry(id1).unwrap();
        assert_eq!(dec1.password, "passwordA-2");
        assert_eq!(dec1.totp_secret.as_deref(), Some("JBSWY3DPEHPK3PXP"));
        
        // Check restored entry's password history decrypts correctly
        let restored_history = reopened_mut.get_password_history(id1).unwrap();
        assert_eq!(restored_history.len(), 1);
        assert_eq!(restored_history[0].password, "passwordA-1");
    }
}


