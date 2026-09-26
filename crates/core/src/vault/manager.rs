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
use subtle::ConstantTimeEq;

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
pub use crate::vault::trash::{TrashedEntryPreview, VaultStorageMetrics};

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
    pub(crate) storage: Option<super::storage::StorageSession>,
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
    if meta.len() == 0 {
        return Err(VaultError::InvalidFormat(format!(
            "Key file ({}) is empty (0 bytes)",
            path.display()
        )));
    }
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
        let bytes = key_file_path.map(read_key_file_safely).transpose()?;
        Self::create_with_keyfile_bytes(name, password, bytes.as_ref().map(|b| b.as_slice()), path)
    }

    pub fn create_with_keyfile_bytes(name: &str, password: &str, key_file_bytes: Option<&[u8]>, path: &Path) -> crate::Result<Self> {
        validate_keyfile_bytes(key_file_bytes)?;
        crate::vault::validation::validate_display_name(name)?;
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let salt = generate_salt();


        // Derive keys from master password + optional key file
        let master_key = derive_master_key_with_keyfile(
            password.as_bytes(),
            key_file_bytes,
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
            storage: None,
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
        let bytes = key_file_path.map(read_key_file_safely).transpose()?;
        Self::open_with_keyfile_bytes(path, password, bytes.as_ref().map(|b| b.as_slice()))
    }

    pub fn open_with_keyfile_bytes(path: &Path, password: &str, key_file_bytes: Option<&[u8]>) -> crate::Result<Self> {
        validate_keyfile_bytes(key_file_bytes)?;
        let file_bytes = super::storage::read_bounded(path)
            .map_err(|e| VaultError::VaultNotFound(format!("{}: {}", path.display(), e)))?;

        if super::storage::is_protected(&file_bytes) {
            let (header, _) = super::storage::parse(&file_bytes)?;
            let serial = super::storage::connected_serial(&header)?;
            let (session, keys) = super::storage::StorageSession::open(header, password, key_file_bytes, serial.as_deref())?;
            let inner = session.decrypt(&file_bytes)?;
            let mut manager = Self::from_decrypted_vault_file(path, VaultFile::from_bytes(&inner)?, keys)?;
            manager.storage = Some(session);
            return Ok(manager);
        }

        let vault_file = VaultFile::from_bytes(&file_bytes)?;

        if vault_file.hardware2fa.is_some() {
            return Err(VaultError::Hardware2FaRequired);
        }


        // Derive keys from password + optional key file + stored salt
        let master_key = derive_master_key_with_keyfile(
            password.as_bytes(),
            key_file_bytes,
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
            storage: None,
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
        let file_bytes = super::storage::read_bounded(&self.path)
            .map_err(|e| VaultError::VaultNotFound(format!("{}: {}", self.path.display(), e)))?;

        let file_bytes = if let Some(session) = &self.storage {
            session.decrypt(&file_bytes)?
        } else {
            Zeroizing::new(file_bytes)
        };

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
        let serialized = Zeroizing::new(rmp_serde::to_vec(&self.data)
            .map_err(|e| VaultError::SerializationError(format!("Vault serialize: {}", e)))?);

        let mut flags = 0u16;
        if self.biometric.is_some() {
            flags |= crate::vault::format::FLAG_HAS_BIOMETRIC;
        }
        if self.hardware2fa.is_some() {
            flags |= crate::vault::format::FLAG_HAS_HARDWARE_2FA;
        }

        let header = FileHeader {
            version: if self.hardware2fa.is_some() && self.data.settings.hardware_password_key.is_some() { super::format::HARDWARE_BOUND_VERSION } else { FORMAT_VERSION },
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
        let inner = Zeroizing::new(vault_file.to_bytes()?);
        let file_bytes = if let Some(session) = &self.storage { session.seal(&inner)? } else { inner.to_vec() };
        if let Some(session) = &self.storage { session.check_disk_header(&self.path)?; }
        super::storage::atomic_write(&self.path, &file_bytes)?;
        if let Some(session) = &mut self.storage { session.mark_persisted()?; }

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
        self.storage = None;
        self.data.entries.clear();
        self.data.tags.clear();
        self.data.trash.clear();
        self.data.settings = Default::default();
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

    /// Cryptographically verifies candidate master password against active vault session keys.
    /// Derives candidate subkeys using Argon2id and HKDF, and compares in constant time.
    pub fn verify_master_password(&self, candidate: &str) -> crate::Result<bool> {
        self.verify_master_password_with_keyfile(candidate, None)
    }

    /// Cryptographically verifies candidate master password with optional keyfile
    /// against active vault session keys using constant-time comparison.
    pub fn verify_master_password_with_keyfile(
        &self,
        candidate: &str,
        key_file_bytes: Option<&[u8]>,
    ) -> crate::Result<bool> {
        if let Some(session) = &self.storage { return session.verify(candidate, key_file_bytes); }
        let active_keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;
        let candidate_mk = derive_master_key_with_keyfile(
            candidate.as_bytes(),
            key_file_bytes,
            &self.salt,
        )?;
        let candidate_keys = derive_subkeys(&candidate_mk)?;
        let is_valid = if self.hardware2fa.is_some() && self.data.settings.hardware_password_key.is_some() {
            // Entry keys retain the password+keyfile KDF; the payload key is random.
            bool::from(active_keys.entry_key.ct_eq(&candidate_keys.entry_key))
        } else { bool::from(active_keys.vault_key.ct_eq(&candidate_keys.vault_key)) };
        Ok(is_valid)
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

    /// Returns the list of paired trusted devices, deduplicated by ID and identity.
    pub fn get_trusted_devices(&self) -> Vec<TrustedDevice> {
        let mut seen_ids = std::collections::HashSet::new();
        let mut seen_names = std::collections::HashSet::new();
        let mut deduped = Vec::new();
        for dev in &self.data.settings.trusted_devices {
            let name_key = (dev.name.to_lowercase(), dev.os.to_lowercase());
            if seen_ids.insert(dev.id) && seen_names.insert(name_key) {
                deduped.push(dev.clone());
            }
        }
        deduped
    }

    /// Revokes authorization for a paired trusted device by ID and persists changes to disk.
    pub fn revoke_trusted_device(&mut self, device_id: Uuid) -> crate::Result<()> {
        let initial_len = self.data.settings.trusted_devices.len();
        self.data.settings.trusted_devices.retain(|d| d.id != device_id);
        if self.data.settings.trusted_devices.len() != initial_len {
            self.save()?;
        }
        Ok(())
    }

    /// Registers or updates a paired trusted device and persists changes to disk, avoiding duplicates.
    pub fn register_trusted_device(&mut self, device: TrustedDevice) -> crate::Result<()> {
        self.data.settings.trusted_devices.retain(|d| {
            d.id != device.id && !(d.name.eq_ignore_ascii_case(&device.name) && d.os == device.os)
        });
        self.data.settings.trusted_devices.push(device);
        self.save()?;
        Ok(())
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

fn validate_keyfile_bytes(bytes: Option<&[u8]>) -> crate::Result<()> {
    if bytes.is_some_and(|b| b.is_empty() || b.len() as u64 > MAX_KEY_FILE_SIZE) {
        return Err(VaultError::InvalidFormat("Key file must contain 1 byte to 32 MB".into()));
    }
    Ok(())
}
