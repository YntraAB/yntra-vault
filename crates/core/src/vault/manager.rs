//! Vault Manager — orchestrates all vault operations
//!
//! Holds active vault state, keys, and coordinates lifecycle operations:
//! create, open, save, and lock with multi-layer authenticated encryption.

use std::fs;
use std::path::{Path, PathBuf};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::crypto::{
    derive_master_key_with_keyfile, derive_subkeys,
    decrypt_vault, decrypt_vault_with_aad,
    encrypt_vault_with_aad, verify_hmac,
    SubKeys,
};
use crate::crypto::kdf::generate_salt;
use crate::error::VaultError;
use crate::vault::format::{FileHeader, KdfParams, VaultFile, FORMAT_VERSION};
use crate::vault::types::*;

// Re-exports for backwards compatibility across crates
pub use crate::vault::entry::{DecryptedEntry, NewEntry, UpdateEntry};
pub use crate::vault::trash::TrashedEntryPreview;

/// Active vault state — holds decrypted data + derived keys.
pub struct VaultManager {
    /// Path to the .vdb file
    pub path: PathBuf,
    /// Decrypted vault contents
    pub data: VaultData,
    /// Derived subkeys (zeroed on lock)
    pub(crate) keys: Option<SubKeys>,
    /// Salt from the file header
    pub(crate) salt: [u8; 32],
    /// Embedded biometric container block
    pub(crate) biometric: Option<crate::vault::format::EmbeddedBiometricHeader>,
    /// Embedded hardware 2FA container block
    pub(crate) hardware2fa: Option<Vec<crate::crypto::hardware2fa::EmbeddedHardware2FaHeader>>,
    /// In-memory Zero-Disclosure search index
    pub(crate) search_index: std::collections::HashMap<[u8; 8], Vec<Uuid>>,
}

/// Maximum allowed key file size (32 MB) to prevent out-of-memory DoS attacks.
const MAX_KEY_FILE_SIZE: u64 = 32 * 1024 * 1024;

/// Safely reads keyfile bytes into a hardware page-locked RAM buffer (`LockedBuffer`)
/// protected by VirtualLock/mlock and canary guard pages, enforcing a 32MB maximum size limit.
pub fn read_key_file_safely(path: &Path) -> crate::Result<crate::crypto::LockedBuffer> {
    let meta = fs::metadata(path).map_err(|e| {
        VaultError::VaultNotFound(format!("Key file error ({}): {}", path.display(), e))
    })?;
    if meta.len() > MAX_KEY_FILE_SIZE {
        return Err(VaultError::InvalidFormat(format!(
            "Key file ({}) size ({} bytes) exceeds maximum allowed limit of 32 MB",
            path.display(),
            meta.len()
        )));
    }
    let raw_bytes = Zeroizing::new(fs::read(path).map_err(|e| {
        VaultError::VaultNotFound(format!("Key file error ({}): {}", path.display(), e))
    })?);
    let locked = crate::crypto::LockedBuffer::new(&raw_bytes);
    Ok(locked)
}

impl VaultManager {
    /// Create a brand new vault with the given master password.
    pub fn create(name: &str, password: &str, path: &Path) -> crate::Result<Self> {
        Self::create_with_keyfile(name, password, None, path)
    }

    /// Create a brand new vault with master password and optional key file.
    pub fn create_with_keyfile(
        name: &str,
        password: &str,
        key_file_path: Option<&Path>,
        path: &Path,
    ) -> crate::Result<Self> {
        let salt = generate_salt();

        let key_file_bytes = match key_file_path {
            Some(kf_path) => Some(read_key_file_safely(kf_path)?),
            None => None,
        };

        // Derive keys from master password + optional key file
        let master_key = derive_master_key_with_keyfile(
            password.as_bytes(),
            key_file_bytes.as_ref().map(|b| b.as_slice()),
            &salt,
        )?;
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
            settings: VaultSettings::default(),
        };

        let mut manager = VaultManager {
            path: path.to_path_buf(),
            data,
            keys: Some(subkeys),
            salt,
            biometric: None,
            hardware2fa: None,
            search_index: std::collections::HashMap::new(),
        };

        // Write initial empty vault to disk
        manager.save()?;
        Ok(manager)
    }

    /// Open an existing vault with master password.
    pub fn open(path: &Path, password: &str) -> crate::Result<Self> {
        Self::open_with_keyfile(path, password, None)
    }

    /// Open an existing vault with master password and optional key file.
    pub fn open_with_keyfile(
        path: &Path,
        password: &str,
        key_file_path: Option<&Path>,
    ) -> crate::Result<Self> {
        let file_bytes = fs::read(path)
            .map_err(|e| VaultError::VaultNotFound(format!("{}: {}", path.display(), e)))?;

        let vault_file = VaultFile::from_bytes(&file_bytes)?;

        if vault_file.hardware2fa.is_some() {
            return Err(VaultError::Hardware2FaRequired);
        }

        let key_file_bytes = match key_file_path {
            Some(kf_path) => Some(read_key_file_safely(kf_path)?),
            None => None,
        };

        // Derive keys from password + optional key file + stored salt
        let master_key = derive_master_key_with_keyfile(
            password.as_bytes(),
            key_file_bytes.as_ref().map(|b| b.as_slice()),
            &vault_file.header.salt,
        )?;
        let subkeys = derive_subkeys(&master_key)?;

        // Verify HMAC integrity for legacy v1/v2 files
        if vault_file.header.version <= 2 {
            if let Some(expected_hmac) = &vault_file.hmac {
                verify_hmac(
                    &vault_file.encrypted_payload,
                    expected_hmac,
                    &subkeys.hmac_key,
                )?;
            } else {
                return Err(VaultError::InvalidFormat(
                    "Missing expected HMAC in legacy v1/v2 file format".into(),
                ));
            }
        }

        Self::from_decrypted_vault_file(path, vault_file, subkeys)
    }

    /// Common decryption and deserialization of vault payload from a VaultFile using SubKeys.
    pub(crate) fn decrypt_vault_payload(
        vault_file: &VaultFile,
        subkeys: &SubKeys,
    ) -> crate::Result<VaultData> {
        if vault_file.encrypted_payload.len() < 24 {
            return Err(VaultError::InvalidFormat(
                "Encrypted payload too short (must be at least 24 bytes for XChaCha20 nonce)".into(),
            ));
        }

        // Decrypt vault payload (Layer 1: XChaCha20-Poly1305 with AAD for v3+)
        let encrypted_blob = crate::crypto::cipher::EncryptedBlob {
            nonce: vault_file.encrypted_payload[..24].to_vec(),
            ciphertext: vault_file.encrypted_payload[24..].to_vec(),
        };

        let decrypted = if vault_file.header.version >= 3 {
            let aad = vault_file.header.aad_bytes()?;
            decrypt_vault_with_aad(&encrypted_blob, &subkeys.vault_key, &aad)?
        } else {
            decrypt_vault(&encrypted_blob, &subkeys.vault_key)?
        };

        // Deserialize vault data based on file format version
        let data: VaultData = match vault_file.header.version {
            // v1: bincode payload (legacy format)
            1 => {
                match bincode::deserialize(&decrypted) {
                    Ok(d) => d,
                    Err(_) => {
                        // Pre-passkey bincode layout
                        let legacy: LegacyVaultData = bincode::deserialize(&decrypted)
                            .map_err(|e| VaultError::SerializationError(
                                format!("Legacy vault deserialize: {}", e)
                            ))?;
                        legacy.into_current()
                    }
                }
            }
            // v2+: MessagePack payload (self-describing, future-proof)
            _ => {
                rmp_serde::from_slice(&decrypted)
                    .map_err(|e| VaultError::SerializationError(
                        format!("Vault deserialize: {}", e)
                    ))?
            }
        };

        Ok(data)
    }

    /// Common decryption, deserialization, and manager initialization from a verified VaultFile and SubKeys.
    pub(crate) fn from_decrypted_vault_file(
        path: &Path,
        vault_file: VaultFile,
        subkeys: SubKeys,
    ) -> crate::Result<Self> {
        let data = Self::decrypt_vault_payload(&vault_file, &subkeys)?;

        let mut manager = VaultManager {
            path: path.to_path_buf(),
            data,
            keys: Some(subkeys),
            salt: vault_file.header.salt,
            biometric: vault_file.biometric,
            hardware2fa: vault_file.hardware2fa,
            search_index: std::collections::HashMap::new(),
        };
        manager.rebuild_search_index();
        Ok(manager)
    }

    /// Reload vault contents from the underlying file on disk using current derived keys.
    ///
    /// Re-reads the file from `self.path`, decrypts it using current active `SubKeys`, updates
    /// in-memory data, header metadata, and rebuilds the search index.
    pub fn reload(&mut self) -> crate::Result<()> {
        let subkeys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;
        let file_bytes = fs::read(&self.path)
            .map_err(|e| VaultError::VaultNotFound(format!("{}: {}", self.path.display(), e)))?;

        let vault_file = VaultFile::from_bytes(&file_bytes)?;

        if vault_file.header.version <= 2 {
            if let Some(expected_hmac) = &vault_file.hmac {
                verify_hmac(
                    &vault_file.encrypted_payload,
                    expected_hmac,
                    &subkeys.hmac_key,
                )?;
            } else {
                return Err(VaultError::InvalidFormat(
                    "Missing expected HMAC in legacy v1/v2 file format".into(),
                ));
            }
        }

        let data = Self::decrypt_vault_payload(&vault_file, subkeys)?;

        self.data = data;
        self.salt = vault_file.header.salt;
        self.biometric = vault_file.biometric;
        self.hardware2fa = vault_file.hardware2fa;
        self.rebuild_search_index();

        Ok(())
    }

    /// Save the vault to disk with full encryption (Single-File .vdb Architecture).
    pub fn save(&mut self) -> crate::Result<()> {
        let keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;

        // Update metadata
        self.data.metadata.updated_at = Utc::now();
        self.data.metadata.entry_count = self.data.entries.len();

        // Clean up old trash (> 30 days)
        let cutoff = Utc::now() - chrono::Duration::days(30);
        self.data.trash.retain(|t| t.deleted_at > cutoff);

        // Serialize vault data as MessagePack (self-describing)
        let serialized = rmp_serde::to_vec(&self.data)
            .map_err(|e| VaultError::SerializationError(format!("Vault serialize: {}", e)))?;

        let mut flags = 0u16;
        if self.biometric.is_some() {
            flags |= crate::vault::format::FLAG_HAS_BIOMETRIC;
        }
        if self.hardware2fa.is_some() {
            flags |= crate::vault::format::FLAG_HAS_HARDWARE_2FA;
        }

        let header = FileHeader {
            version: FORMAT_VERSION,
            flags,
            salt: self.salt,
            kdf_params: KdfParams::default(),
        };
        let aad = header.aad_bytes()?;

        // Encrypt with Authenticated Header AAD binding (Layer 1: XChaCha20-Poly1305)
        let encrypted = encrypt_vault_with_aad(&serialized, &keys.vault_key, &aad)?;

        // Combine nonce + ciphertext as the payload
        let mut payload = Vec::with_capacity(encrypted.nonce.len() + encrypted.ciphertext.len());
        payload.extend_from_slice(&encrypted.nonce);
        payload.extend_from_slice(&encrypted.ciphertext);

        // Build single-file vault structure (v4/v5)
        let vault_file = VaultFile {
            header,
            hmac: None,
            biometric: self.biometric.clone(),
            hardware2fa: self.hardware2fa.clone(),
            encrypted_payload: payload,
        };

        // Write to disk atomically (write to temp file, then rename)
        let file_bytes = vault_file.to_bytes()?;
        let temp_path = self.path.with_extension("vdb.tmp");
        fs::write(&temp_path, &file_bytes)?;
        fs::rename(&temp_path, &self.path)?;

        // Clean up any legacy sidecar files if present
        let old_bio = self.path.with_extension("vdb.bio");
        let old_kek = self.path.with_extension("vdb.bio_kek");
        if old_bio.exists() {
            let _ = fs::remove_file(old_bio);
        }
        if old_kek.exists() {
            let _ = fs::remove_file(old_kek);
        }

        Ok(())
    }

    /// Explicitly save the current vault state to disk.
    pub fn save_vault(&mut self) -> crate::Result<()> {
        self.save()
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

    /// Retrieve the root salt for this vault.
    pub fn salt(&self) -> [u8; 32] {
        self.salt
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
}

// ─── Legacy Migration Types ─────────────────────────────────────────────
// Pre-passkey Entry layout for backwards-compatible deserialization.
// Vaults saved before passkey support used this layout. On open, they
// are migrated to the current format and re-saved on next write.

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct LegacyEntry {
    pub id: Uuid,
    pub title: String,
    pub username: String,
    pub encrypted_password: crate::crypto::cipher::EncryptedBlob,
    pub url: String,
    pub email: String,
    pub notes: String,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub pinned: bool,
    pub encrypted_totp_secret: Option<crate::crypto::cipher::EncryptedBlob>,
    pub custom_fields: Vec<CustomField>,
    pub entry_type: EntryType,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
    pub password_history: Vec<PasswordHistoryItem>,
    pub breach_status: BreachStatus,
    pub strength_score: Option<StrengthScore>,
    pub password_changed_at: chrono::DateTime<Utc>,
}

impl LegacyEntry {
    fn into_current(self) -> Entry {
        Entry {
            id: self.id,
            title: self.title,
            username: self.username,
            encrypted_password: self.encrypted_password,
            url: self.url,
            email: self.email,
            notes: self.notes,
            tags: self.tags,
            favorite: self.favorite,
            pinned: self.pinned,
            encrypted_totp_secret: self.encrypted_totp_secret,
            custom_fields: self.custom_fields,
            entry_type: self.entry_type,
            created_at: self.created_at,
            updated_at: self.updated_at,
            password_history: self.password_history,
            breach_status: self.breach_status,
            strength_score: self.strength_score,
            password_changed_at: self.password_changed_at,
            encrypted_passkey: None,
            passkey_public_key: None,
            attachments: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct LegacyTrashedEntry {
    pub entry: LegacyEntry,
    pub deleted_at: chrono::DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct LegacyVaultData {
    pub metadata: VaultMetadata,
    pub entries: Vec<LegacyEntry>,
    pub tags: Vec<Tag>,
    pub trash: Vec<LegacyTrashedEntry>,
}

impl LegacyVaultData {
    pub(crate) fn into_current(self) -> VaultData {
        VaultData {
            metadata: self.metadata,
            entries: self.entries.into_iter().map(|e| e.into_current()).collect(),
            tags: self.tags,
            trash: self
                .trash
                .into_iter()
                .map(|t| TrashedEntry {
                    entry: t.entry.into_current(),
                    deleted_at: t.deleted_at,
                })
                .collect(),
            settings: VaultSettings::default(),
        }
    }
}
