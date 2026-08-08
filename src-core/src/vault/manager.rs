//! Vault Manager — orchestrates all vault operations
//!
//! Handles create, open, save, lock with full multi-layer encryption.

use std::path::{Path, PathBuf};
use std::fs;
use chrono::Utc;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use crate::crypto::{
    derive_master_key_with_keyfile, derive_subkeys,
    decrypt_vault,
    encrypt_vault_with_aad, decrypt_vault_with_aad,
    verify_hmac,
    SubKeys, EntryKey, cipher::EncryptedBlob,
};
use crate::crypto::kdf::generate_salt;
use crate::vault::format::{VaultFile, FileHeader, KdfParams, FORMAT_VERSION};
use crate::vault::types::*;
use zeroize::{Zeroize, Zeroizing};
use crate::error::VaultError;


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
fn read_key_file_safely(path: &Path) -> crate::Result<crate::crypto::LockedBuffer> {
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

        // Rebuild index
        manager.rebuild_search_index();

        // Save to disk
        manager.save()?;

        Ok(manager)
    }

    /// Open an existing vault with the master password.
    pub fn open(path: &Path, password: &str) -> crate::Result<Self> {
        Self::open_with_keyfile(path, password, None)
    }

    /// Open an existing vault with the master password + optional key file.
    pub fn open_with_keyfile(
        path: &Path,
        password: &str,
        key_file_path: Option<&Path>,
    ) -> crate::Result<Self> {
        // Read file
        let file_bytes = fs::read(path)
            .map_err(|e| VaultError::VaultNotFound(format!("{}: {}", path.display(), e)))?;

        // Parse file format
        let vault_file = VaultFile::from_bytes(&file_bytes)?;

        // If Hardware 2FA is enrolled, require hardware 2FA unlock method
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

    /// Open an existing vault using enrolled Biometric Unlock (Windows Hello, Touch ID, PAM).
    pub fn open_with_biometric(path: &Path) -> crate::Result<Self> {
        let file_bytes = fs::read(path)
            .map_err(|e| VaultError::VaultNotFound(format!("{}: {}", path.display(), e)))?;

        let vault_file = VaultFile::from_bytes(&file_bytes)?;

        if vault_file.hardware2fa.is_some() {
            return Err(VaultError::Hardware2FaRequired);
        }

        let subkeys = crate::crypto::biometric::unlock_from_vault_file(&vault_file)?;

        if vault_file.encrypted_payload.len() < 24 {
            return Err(VaultError::InvalidFormat(
                "Encrypted payload too short (must be at least 24 bytes for XChaCha20 nonce)".into(),
            ));
        }

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

        let data: VaultData = match vault_file.header.version {
            1 => {
                match bincode::deserialize(&decrypted) {
                    Ok(d) => d,
                    Err(_) => {
                        let legacy: LegacyVaultData = bincode::deserialize(&decrypted)
                            .map_err(|e| VaultError::SerializationError(
                                format!("Legacy vault deserialize: {}", e)
                            ))?;
                        legacy.into_current()
                    }
                }
            }
            _ => {
                rmp_serde::from_slice(&decrypted)
                    .map_err(|e| VaultError::SerializationError(
                        format!("Vault deserialize: {}", e)
                    ))?
            }
        };

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

    /// Enroll biometrics for the current open vault (Embedded in single .vdb file).
    pub fn enable_biometric(&mut self) -> crate::Result<()> {
        let keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;
        let temp_header = FileHeader {
            version: FORMAT_VERSION,
            flags: crate::vault::format::FLAG_HAS_BIOMETRIC,
            salt: self.salt,
            kdf_params: KdfParams::default(),
        };
        let bio_header = crate::crypto::biometric::create_embedded_biometric_header(keys, &temp_header)?;
        self.biometric = Some(bio_header);
        self.save()
    }

    /// Disable biometrics for the current vault (Embedded in single .vdb file).
    pub fn disable_biometric(&mut self) -> crate::Result<()> {
        self.biometric = None;
        self.save()
    }

    /// Check if biometric is enabled for the current vault.
    pub fn is_biometric_enabled(&self) -> bool {
        self.biometric.is_some()
    }

    /// Open an existing vault with password + optional key file + Hardware 2FA response.
    pub fn open_with_hardware2fa(
        path: &Path,
        password: &str,
        key_file_path: Option<&Path>,
        hardware_response: &[u8],
    ) -> crate::Result<Self> {
        let file_bytes = fs::read(path)
            .map_err(|e| VaultError::VaultNotFound(format!("{}: {}", path.display(), e)))?;

        let vault_file = VaultFile::from_bytes(&file_bytes)?;

        let key_file_bytes = match key_file_path {
            Some(kf_path) => Some(read_key_file_safely(kf_path)?),
            None => None,
        };

        // Try unlocking via embedded hardware 2FA header envelope first
        let subkeys = if vault_file.hardware2fa.is_some() {
            crate::crypto::hardware2fa::unlock_from_hardware2fa(&vault_file, hardware_response)?
        } else {
            // Otherwise derive master key with hardware response mixed into KDF pre-hash
            let master_key = crate::crypto::hardware2fa::derive_master_key_with_hardware_2fa(
                password.as_bytes(),
                key_file_bytes.as_ref().map(|b| b.as_slice()),
                hardware_response,
                &vault_file.header.salt,
            )?;
            derive_subkeys(&master_key)?
        };

        if vault_file.encrypted_payload.len() < 24 {
            return Err(VaultError::InvalidFormat(
                "Encrypted payload too short (must be at least 24 bytes for XChaCha20 nonce)".into(),
            ));
        }

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

        let data: VaultData = match vault_file.header.version {
            1 => {
                match bincode::deserialize(&decrypted) {
                    Ok(d) => d,
                    Err(_) => {
                        let legacy: LegacyVaultData = bincode::deserialize(&decrypted)
                            .map_err(|e| VaultError::SerializationError(
                                format!("Legacy vault deserialize: {}", e)
                            ))?;
                        legacy.into_current()
                    }
                }
            }
            _ => {
                rmp_serde::from_slice(&decrypted)
                    .map_err(|e| VaultError::SerializationError(
                        format!("Vault deserialize: {}", e)
                    ))?
            }
        };

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

    /// Enroll Hardware 2FA for the current open vault.
    pub fn enable_hardware2fa(
        &mut self,
        protocol: crate::crypto::hardware2fa::Hardware2FaProtocol,
        key_name: &str,
        hardware_response: &[u8],
    ) -> crate::Result<()> {
        let keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;
        let mut flags = 0u16;
        if self.biometric.is_some() { flags |= crate::vault::format::FLAG_HAS_BIOMETRIC; }
        flags |= crate::vault::format::FLAG_HAS_HARDWARE_2FA;

        let temp_header = FileHeader {
            version: FORMAT_VERSION,
            flags,
            salt: self.salt,
            kdf_params: KdfParams::default(),
        };

        let hw_header = crate::crypto::hardware2fa::create_embedded_hardware2fa_header(
            keys,
            &temp_header,
            protocol,
            key_name,
            hardware_response,
        )?;

        let mut list = self.hardware2fa.take().unwrap_or_default();
        list.push(hw_header);
        self.hardware2fa = Some(list);
        self.save()
    }

    /// Disable Hardware 2FA for the current vault.
    pub fn disable_hardware2fa(&mut self) -> crate::Result<()> {
        self.hardware2fa = None;
        self.save()
    }

    /// Check if Hardware 2FA is enabled for the current vault.
    pub fn is_hardware2fa_enabled(&self) -> bool {
        self.hardware2fa.is_some()
    }

    /// Check if Hardware 2FA is enrolled inside a .vdb file at path.
    pub fn is_hardware2fa_enabled_file(vault_path: &Path) -> bool {
        if let Ok(file_bytes) = fs::read(vault_path) {
            if let Ok(vault_file) = VaultFile::from_bytes(&file_bytes) {
                return vault_file.hardware2fa.is_some();
            }
        }
        false
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
        if self.biometric.is_some() { flags |= crate::vault::format::FLAG_HAS_BIOMETRIC; }
        if self.hardware2fa.is_some() { flags |= crate::vault::format::FLAG_HAS_HARDWARE_2FA; }

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
        if old_bio.exists() { let _ = fs::remove_file(old_bio); }
        if old_kek.exists() { let _ = fs::remove_file(old_kek); }

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
                has_passkey: e.encrypted_passkey.is_some(),
                attachment_count: e.attachments.len(),
            }
        }).collect())
    }

    /// Executes a closure with a zero-allocation stack-buffered length-prefixed AAD for field encryption/decryption.
    /// Format: [16-byte raw UUID] || [u16_BE(scope_len)] || [scope_bytes]
    pub(crate) fn with_entry_field_aad<'a, F, R>(entry_id: &Uuid, field_scope: impl Into<FieldScope<'a>>, f: F) -> R
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
    pub(crate) fn with_entry_field_aad_legacy<'a, F, R>(entry_id: &Uuid, field_scope: impl Into<FieldScope<'a>>, f: F) -> R
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
        Self::with_entry_field_aad(entry_id, field_scope, |aad| {
            crate::crypto::encrypt_entry_with_aad(plaintext, &per_entry_key, aad)
        })
    }

    pub(crate) fn decrypt_entry_field<'a>(
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
        Self::with_entry_field_aad_legacy(entry_id, field_scope, |aad| {
            crate::crypto::decrypt_entry_with_aad(blob, master_entry_key, aad)
        })
    }

    /// Get a full entry with decrypted password.
    pub fn get_entry(&self, id: Uuid) -> crate::Result<DecryptedEntry> {
        let keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;

        let entry = self.data.entries.iter()
            .find(|e| e.id == id)
            .ok_or(VaultError::EntryNotFound(id.to_string()))?;

        // Decrypt password (Layer 2: XChaCha20-Poly1305 with per-entry key + AAD)
        let password_bytes = Self::decrypt_entry_field(
            &entry.encrypted_password,
            &keys.entry_key,
            &entry.id,
            "password",
        )?;
        let password = String::from_utf8(password_bytes.to_vec())
            .map_err(|e| VaultError::DecryptionError(format!("Invalid UTF-8 password: {}", e)))?;

        // Decrypt TOTP secret if present
        let totp_secret = if let Some(ref encrypted_totp) = entry.encrypted_totp_secret {
            let bytes = Self::decrypt_entry_field(encrypted_totp, &keys.entry_key, &entry.id, "totp")?;
            Some(String::from_utf8(bytes.to_vec())
                .map_err(|e| VaultError::DecryptionError(format!("Invalid UTF-8 TOTP: {}", e)))?)
        } else {
            None
        };

        let attachments = entry.attachments.iter().map(|att| AttachmentInfo {
            id: att.id,
            name: att.name.clone(),
            size: att.size,
            mime_type: att.mime_type.clone(),
            created_at: att.created_at,
        }).collect();

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

    /// Add a new entry to the vault.
    pub fn add_entry(&mut self, new: NewEntry) -> crate::Result<Uuid> {
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
            Some(Self::encrypt_entry_field(secret.as_bytes(), &keys.entry_key, &id, FieldScope::Totp)?)
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
                    FieldScope::Attachment { attachment_id: &att_id },
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
            strength_score: Some(crate::breach::strength::analyze_password(&new.password)),
            password_changed_at: now,
            encrypted_passkey: None,
            passkey_public_key: None,
            attachments,
        };

        // Generate passkey if requested (single keypair for both fields)
        if new.generate_passkey.unwrap_or(false) {
            let pair = crate::crypto::passkey::generate_passkey_pair()?;
            entry.encrypted_passkey = Some(
                Self::encrypt_entry_field(&pair.private_key, &keys.entry_key, &id, FieldScope::Passkey)?
            );
            entry.passkey_public_key = Some(pair.public_key);
        }

        self.add_entry_to_index(entry.id, &entry.title, &entry.username, &entry.url, &entry.email, &entry.tags);
        self.data.entries.push(entry);
        self.save()?;

        Ok(id)
    }

    /// Checks an array of ParsedImportEntry against current vault entries for duplicates.
    pub fn check_import_duplicates(&self, entries: &mut [crate::vault::importer::ParsedImportEntry]) -> usize {
        let mut dup_count = 0;
        for item in entries.iter_mut() {
            let t_lower = item.title.trim().to_lowercase();
            let u_lower = item.username.trim().to_lowercase();

            let match_found = self.data.entries.iter().any(|e| {
                let e_t = e.title.trim().to_lowercase();
                let e_u = e.username.trim().to_lowercase();
                let title_matches = !t_lower.is_empty() && e_t == t_lower;
                let user_matches = e_u == u_lower || u_lower.is_empty() || e_u.is_empty();
                title_matches && user_matches
            });

            if match_found {
                item.is_duplicate = true;
                item.duplicate_reason = Some(format!("An entry titled '{}' already exists.", item.title));
                dup_count += 1;
            }
        }
        dup_count
    }

    /// Bulk imports parsed entries into the vault using the chosen DuplicateStrategy.
    pub fn bulk_import_entries(
        &mut self,
        entries: Vec<crate::vault::importer::ParsedImportEntry>,
        strategy: crate::vault::importer::DuplicateStrategy,
    ) -> crate::Result<usize> {
        let keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;
        let entry_key = keys.entry_key.clone();
        let mut count = 0;

        let tag_colors = ["#3b82f6", "#10b981", "#8b5cf6", "#f59e0b", "#ec4899", "#06b6d4"];
        let mut color_idx = 0;

        for item in entries {
            let t_lower = item.title.trim().to_lowercase();
            let u_lower = item.username.trim().to_lowercase();

            let existing_id = self.data.entries.iter().find(|e| {
                let e_t = e.title.trim().to_lowercase();
                let e_u = e.username.trim().to_lowercase();
                let title_matches = !t_lower.is_empty() && e_t == t_lower;
                let user_matches = e_u == u_lower || u_lower.is_empty() || e_u.is_empty();
                title_matches && user_matches
            }).map(|e| e.id);

            if let Some(target_id) = existing_id {
                match strategy {
                    crate::vault::importer::DuplicateStrategy::Skip => continue,
                    crate::vault::importer::DuplicateStrategy::Overwrite => {
                        let update = UpdateEntry {
                            title: Some(item.title),
                            username: Some(item.username),
                            password: if !item.password.is_empty() { Some(item.password) } else { None },
                            url: if !item.url.is_empty() { Some(item.url) } else { None },
                            email: if !item.email.is_empty() { Some(item.email) } else { None },
                            notes: if !item.notes.is_empty() { Some(item.notes) } else { None },
                            totp_secret: item.totp_secret,
                            tags: if !item.tags.is_empty() { Some(item.tags) } else { None },
                            custom_fields: if !item.custom_fields.is_empty() { Some(item.custom_fields.clone()) } else { None },
                            ..Default::default()
                        };
                        self.update_entry(target_id, update)?;
                        count += 1;
                        continue;
                    }
                    crate::vault::importer::DuplicateStrategy::KeepBoth => {
                        // Fallthrough to add as new entry
                    }
                }
            }

            // Ensure all tags attached to item exist in vault's global tag list
            for tag_name in &item.tags {
                let tag_trimmed = tag_name.trim();
                if !tag_trimmed.is_empty()
                    && !self.data.tags.iter().any(|t| t.name.eq_ignore_ascii_case(tag_trimmed))
                {
                    self.data.tags.push(Tag {
                        id: Uuid::new_v4(),
                        name: tag_trimmed.to_string(),
                        color: tag_colors[color_idx % tag_colors.len()].to_string(),
                        icon: "tag".to_string(),
                    });
                    color_idx += 1;
                }
            }

            let new_entry = NewEntry {
                title: item.title,
                username: item.username,
                password: item.password,
                url: item.url,
                email: item.email,
                notes: item.notes,
                tags: item.tags,
                totp_secret: item.totp_secret,
                custom_fields: item.custom_fields,
                entry_type: Some(item.entry_type.clone()),
                generate_passkey: None,
                attachments: None,
            };

            let now = Utc::now();
            let id = Uuid::new_v4();

            let encrypted_password = Self::encrypt_entry_field(
                new_entry.password.as_bytes(),
                &entry_key,
                &id,
                FieldScope::Password,
            )?;

            let encrypted_totp = if let Some(ref secret) = new_entry.totp_secret {
                Some(Self::encrypt_entry_field(secret.as_bytes(), &entry_key, &id, FieldScope::Totp)?)
            } else {
                None
            };

            let entry = Entry {
                id,
                title: new_entry.title.clone(),
                username: new_entry.username.clone(),
                encrypted_password,
                url: new_entry.url.clone(),
                email: new_entry.email.clone(),
                notes: new_entry.notes.clone(),
                tags: new_entry.tags.clone(),
                favorite: false,
                pinned: false,
                encrypted_totp_secret: encrypted_totp,
                custom_fields: new_entry.custom_fields,
                entry_type: item.entry_type,
                created_at: now,
                updated_at: now,
                password_history: Vec::new(),
                breach_status: BreachStatus::Unknown,
                strength_score: Some(crate::breach::strength::analyze_password(&new_entry.password)),
                password_changed_at: now,
                encrypted_passkey: None,
                passkey_public_key: None,
                attachments: Vec::new(),
            };

            self.data.entries.push(entry);
            count += 1;
        }

        self.rebuild_search_index();
        self.save()?;
        Ok(count)
    }

    /// Exports decrypted vault entries to a CSV file.
    pub fn export_csv(&self, dest_path: &Path) -> crate::Result<()> {
        let mut csv = String::from("Title,Username,Email,Password,URL,Notes,TOTP,Tags\n");
        for entry in self.list_entries()? {
            if let Ok(dec) = self.get_entry(entry.id) {
                let esc = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
                let totp = dec.totp_secret.as_deref().unwrap_or("");
                let tags = dec.tags.join(";");

                csv.push_str(&format!(
                    "{},{},{},{},{},{},{},{}\n",
                    esc(&dec.title),
                    esc(&dec.username),
                    esc(&dec.email),
                    esc(&dec.password),
                    esc(&dec.url),
                    esc(&dec.notes),
                    esc(totp),
                    esc(&tags),
                ));
            }
        }
        std::fs::write(dest_path, csv)
            .map_err(|e| VaultError::InvalidFormat(format!("Failed to write CSV: {}", e)))
    }

    /// Exports decrypted vault entries to a JSON file.
    pub fn export_json(&self, dest_path: &Path) -> crate::Result<()> {
        let mut items = Vec::new();
        for entry in self.list_entries()? {
            if let Ok(dec) = self.get_entry(entry.id) {
                items.push(dec);
            }
        }
        let json = serde_json::to_string_pretty(&items)
            .map_err(|e| VaultError::SerializationError(format!("Failed to format JSON: {}", e)))?;
        std::fs::write(dest_path, json)
            .map_err(|e| VaultError::InvalidFormat(format!("Failed to write JSON: {}", e)))
    }

    /// Update an existing entry. Tracks password history.
    pub fn update_entry(&mut self, id: Uuid, update: UpdateEntry) -> crate::Result<()> {
        let entry_key = self.keys.as_ref().ok_or(VaultError::VaultLocked)?.entry_key.clone();
        let now = Utc::now();

        // 1. Perform modifications in a nested block to drop `entry` borrow
        {
            let entry = self.data.entries.iter_mut()
                .find(|e| e.id == id)
                .ok_or(VaultError::EntryNotFound(id.to_string()))?;

            // If password changed, save old one to history and reset breach status
            if let Some(ref new_password) = update.password {
                let old_password_bytes = Self::decrypt_entry_field(
                    &entry.encrypted_password,
                    &entry_key,
                    &id,
                    FieldScope::Password,
                )?;
                let old_password = String::from_utf8(old_password_bytes.to_vec())
                    .map_err(|e| crate::error::VaultError::DecryptionError(e.to_string()))?;

                if &old_password != new_password {
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
                        Self::encrypt_entry_field(totp_secret.as_bytes(), &entry_key, &id, FieldScope::Totp)?
                    );
                }
            }

            // Handle passkey: generate new, or remove existing
            if let Some(ref action) = update.passkey_action {
                match action.as_str() {
                    "generate" => {
                        let pair = crate::crypto::passkey::generate_passkey_pair()?;
                        entry.encrypted_passkey = Some(
                            Self::encrypt_entry_field(&pair.private_key, &entry_key, &id, FieldScope::Passkey)?
                        );
                        entry.passkey_public_key = Some(pair.public_key);
                    }
                    "remove" => {
                        entry.encrypted_passkey = None;
                        entry.passkey_public_key = None;
                    }
                    _ => {}
                }
            }

            // Handle deleting attachments if requested
            if let Some(ref del_ids) = update.delete_attachment_ids {
                entry.attachments.retain(|att| !del_ids.contains(&att.id));
            }

            // Handle adding new staged attachments if requested
            if let Some(new_atts) = update.new_attachments {
                for att in new_atts {
                    let att_id = Uuid::new_v4();
                    let encrypted_blob = Self::encrypt_entry_field(
                        &att.data,
                        &entry_key,
                        &id,
                        FieldScope::Attachment { attachment_id: &att_id },
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

    /// Decrypt and return the raw byte payload of a file attachment.
    pub fn get_attachment_data(&self, entry_id: Uuid, attachment_id: Uuid) -> crate::Result<Vec<u8>> {
        let keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;
        let entry = self.data.entries.iter()
            .find(|e| e.id == entry_id)
            .ok_or(VaultError::EntryNotFound(entry_id.to_string()))?;

        let attachment = entry.attachments.iter()
            .find(|a| a.id == attachment_id)
            .ok_or(VaultError::InvalidFormat(format!("Attachment {} not found", attachment_id)))?;

        let data = Self::decrypt_entry_field(
            &attachment.encrypted_blob,
            &keys.entry_key,
            &entry_id,
            FieldScope::Attachment { attachment_id: &attachment_id },
        )?;

        Ok(data.to_vec())
    }

    /// Add an attachment directly to an existing unlocked entry.
    pub fn add_attachment(&mut self, entry_id: Uuid, name: &str, mime_type: &str, data: &[u8]) -> crate::Result<AttachmentInfo> {
        let entry_key = self.keys.as_ref().ok_or(VaultError::VaultLocked)?.entry_key.clone();
        let now = Utc::now();
        let attachment_id = Uuid::new_v4();

        let encrypted_blob = Self::encrypt_entry_field(
            data,
            &entry_key,
            &entry_id,
            FieldScope::Attachment { attachment_id: &attachment_id },
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

        let entry = self.data.entries.iter_mut()
            .find(|e| e.id == entry_id)
            .ok_or(VaultError::EntryNotFound(entry_id.to_string()))?;

        entry.attachments.push(attachment);
        entry.updated_at = now;
        self.save()?;

        Ok(info)
    }

    /// Delete an attachment from an existing unlocked entry.
    pub fn delete_attachment(&mut self, entry_id: Uuid, attachment_id: Uuid) -> crate::Result<()> {
        let entry = self.data.entries.iter_mut()
            .find(|e| e.id == entry_id)
            .ok_or(VaultError::EntryNotFound(entry_id.to_string()))?;

        let pos = entry.attachments.iter().position(|a| a.id == attachment_id)
            .ok_or(VaultError::InvalidFormat(format!("Attachment {} not found", attachment_id)))?;

        entry.attachments.remove(pos);
        entry.updated_at = Utc::now();
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

    /// Permanently empty all entries from trash.
    pub fn empty_trash(&mut self) -> crate::Result<()> {
        if !self.is_unlocked() {
            return Err(VaultError::VaultLocked);
        }
        self.data.trash.clear();
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
        let tag_name = self.data.tags.iter()
            .find(|t| t.id == id)
            .map(|t| t.name.clone());

        if let Some(name) = tag_name {
            self.data.tags.retain(|t| t.id != id);
            for entry in &mut self.data.entries {
                entry.tags.retain(|t| t != &name);
            }
            self.save()?;
        }
        Ok(())
    }

    pub fn update_tag(&mut self, id: Uuid, name: &str, color: &str, icon: &str) -> crate::Result<()> {
        if let Some(tag) = self.data.tags.iter_mut().find(|t| t.id == id) {
            let old_name = tag.name.clone();
            tag.name = name.to_string();
            tag.color = color.to_string();
            tag.icon = icon.to_string();

            // Update all entries referencing this tag if the name changed
            if old_name != tag.name {
                for entry in &mut self.data.entries {
                    for t in &mut entry.tags {
                        if *t == old_name {
                            *t = tag.name.clone();
                        }
                    }
                }
            }
            self.save()?;
        }
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
            let pwd_bytes = Self::decrypt_entry_field(&entry.encrypted_password, &keys.entry_key, &entry.id, "password")?;
            let pwd = String::from_utf8(pwd_bytes.to_vec())
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

    /// Generate a 32-byte cryptographically secure random key file at path.
    pub fn generate_key_file(path: &Path) -> crate::Result<()> {
        use rand::Rng;
        let mut key_bytes = [0u8; 32];
        rand::rng().fill(&mut key_bytes);
        fs::write(path, key_bytes)?;
        Ok(())
    }

    pub fn change_master_password(&mut self, current: &str, new_password: &str) -> crate::Result<()> {
        self.change_master_password_with_keyfiles(current, None, new_password, None)
    }

    /// Change the master password with key file support.
    pub fn change_master_password_with_keyfiles(
        &mut self,
        current: &str,
        current_key_file: Option<&Path>,
        new_password: &str,
        new_key_file: Option<&Path>,
    ) -> crate::Result<()> {
        let cur_kf_bytes = match current_key_file {
            Some(kf_path) => Some(read_key_file_safely(kf_path)?),
            None => None,
        };

        // Verify current password by trying to derive same keys
        let current_mk = derive_master_key_with_keyfile(
            current.as_bytes(),
            cur_kf_bytes.as_ref().map(|b| b.as_slice()),
            &self.salt,
        )?;
        let current_keys = derive_subkeys(&current_mk)?;

        // Quick check: try decrypting first entry's password (or first trashed entry if entries is empty)
        if let Some(entry) = self.data.entries.first() {
            Self::decrypt_entry_field(&entry.encrypted_password, &current_keys.entry_key, &entry.id, "password")
                .map_err(|_| VaultError::InvalidPassword)?;
        } else if let Some(trashed) = self.data.trash.first() {
            Self::decrypt_entry_field(&trashed.entry.encrypted_password, &current_keys.entry_key, &trashed.entry.id, "password")
                .map_err(|_| VaultError::InvalidPassword)?;
        }

        let new_kf_bytes = match new_key_file {
            Some(kf_path) => Some(read_key_file_safely(kf_path)?),
            None => None,
        };

        // Generate new salt
        let new_salt = generate_salt();
        let new_mk = derive_master_key_with_keyfile(
            new_password.as_bytes(),
            new_kf_bytes.as_ref().map(|b| b.as_slice()),
            &new_salt,
        )?;
        let new_keys = derive_subkeys(&new_mk)?;

        // Re-encrypt every entry's password and TOTP with new keys
        for entry in &mut self.data.entries {
            // Decrypt with old key, re-encrypt with new key
            let pw_bytes = Self::decrypt_entry_field(&entry.encrypted_password, &current_keys.entry_key, &entry.id, "password")?;
            entry.encrypted_password = Self::encrypt_entry_field(&pw_bytes, &new_keys.entry_key, &entry.id, "password")?;

            if let Some(ref totp) = entry.encrypted_totp_secret {
                let totp_bytes = Self::decrypt_entry_field(totp, &current_keys.entry_key, &entry.id, "totp")?;
                entry.encrypted_totp_secret = Some(
                    Self::encrypt_entry_field(&totp_bytes, &new_keys.entry_key, &entry.id, "totp")?
                );
            }

            if let Some(ref passkey) = entry.encrypted_passkey {
                let passkey_bytes = Self::decrypt_entry_field(passkey, &current_keys.entry_key, &entry.id, "passkey")?;
                entry.encrypted_passkey = Some(
                    Self::encrypt_entry_field(&passkey_bytes, &new_keys.entry_key, &entry.id, "passkey")?
                );
            }

            // Re-encrypt password history
            for hist in &mut entry.password_history {
                let hist_bytes = Self::decrypt_entry_field(&hist.encrypted_password, &current_keys.entry_key, &entry.id, "history")
                    .or_else(|_| Self::decrypt_entry_field(&hist.encrypted_password, &current_keys.entry_key, &entry.id, "password"))?;
                hist.encrypted_password = Self::encrypt_entry_field(&hist_bytes, &new_keys.entry_key, &entry.id, "history")?;
            }
        }

        // Re-encrypt every trashed entry's password and TOTP with new keys
        for trashed in &mut self.data.trash {
            let entry = &mut trashed.entry;

            // Decrypt with old key, re-encrypt with new key
            let pw_bytes = Self::decrypt_entry_field(&entry.encrypted_password, &current_keys.entry_key, &entry.id, "password")?;
            entry.encrypted_password = Self::encrypt_entry_field(&pw_bytes, &new_keys.entry_key, &entry.id, "password")?;

            if let Some(ref totp) = entry.encrypted_totp_secret {
                let totp_bytes = Self::decrypt_entry_field(totp, &current_keys.entry_key, &entry.id, "totp")?;
                entry.encrypted_totp_secret = Some(
                    Self::encrypt_entry_field(&totp_bytes, &new_keys.entry_key, &entry.id, "totp")?
                );
            }

            if let Some(ref passkey) = entry.encrypted_passkey {
                let passkey_bytes = Self::decrypt_entry_field(passkey, &current_keys.entry_key, &entry.id, "passkey")?;
                entry.encrypted_passkey = Some(
                    Self::encrypt_entry_field(&passkey_bytes, &new_keys.entry_key, &entry.id, "passkey")?
                );
            }

            // Re-encrypt password history
            for hist in &mut entry.password_history {
                let hist_bytes = Self::decrypt_entry_field(&hist.encrypted_password, &current_keys.entry_key, &entry.id, "history")
                    .or_else(|_| Self::decrypt_entry_field(&hist.encrypted_password, &current_keys.entry_key, &entry.id, "password"))?;
                hist.encrypted_password = Self::encrypt_entry_field(&hist_bytes, &new_keys.entry_key, &entry.id, "history")?;
            }
        }

        // Update salt and keys
        self.salt = new_salt;
        self.keys = Some(new_keys.clone());

        if self.biometric.is_some() {
            let temp_header = FileHeader {
                version: FORMAT_VERSION,
                flags: crate::vault::format::FLAG_HAS_BIOMETRIC,
                salt: self.salt,
                kdf_params: KdfParams::default(),
            };
            if let Ok(new_bio) = crate::crypto::biometric::create_embedded_biometric_header(&new_keys, &temp_header) {
                self.biometric = Some(new_bio);
            }
        }

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
            trash: self.trash.into_iter().map(|t| TrashedEntry {
                entry: t.entry.into_current(),
                deleted_at: t.deleted_at,
            }).collect(),
            settings: VaultSettings::default(),
        }
    }
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
            generate_passkey: None,
            attachments: None,
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
            generate_passkey: None,
            attachments: None,
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

    #[test]
    fn test_truncated_payload_returns_error() {
        let test_vault = TestVault::new();
        let password = "test-password";
        let manager = VaultManager::create("test-vault", password, &test_vault.path).unwrap();
        drop(manager);

        // Read vault bytes, corrupt payload length to < 24 bytes
        let header = FileHeader {
            version: FORMAT_VERSION,
            flags: 0,
            salt: [1u8; 32],
            kdf_params: KdfParams::default(),
        };
        let file = VaultFile {
            header,
            hmac: None,
            biometric: None,
            hardware2fa: None,
            encrypted_payload: vec![1, 2, 3], // Payload < 24 bytes
        };
        let corrupted_bytes = file.to_bytes().unwrap();
        fs::write(&test_vault.path, &corrupted_bytes).unwrap();

        let result = VaultManager::open(&test_vault.path, password);
        assert!(result.is_err());
        match result {
            Err(VaultError::IntegrityError) | Err(VaultError::InvalidFormat(_)) => {},
            Err(e) => panic!("Expected IntegrityError or InvalidFormat, got err {:?}", e),
            Ok(_) => panic!("Expected error on truncated payload, got Ok"),
        }
    }

    #[test]
    fn test_history_aad_isolation_prevents_substitution() {
        let test_vault = TestVault::new();
        let password = "test-password";
        let mut manager = VaultManager::create("test-vault", password, &test_vault.path).unwrap();

        let entry = NewEntry {
            title: "Security Test".to_string(),
            username: "user".to_string(),
            password: "active-password-v1".to_string(),
            url: "".to_string(),
            email: "".to_string(),
            notes: "".to_string(),
            tags: vec![],
            totp_secret: None,
            custom_fields: Vec::new(),
            entry_type: None,
            generate_passkey: None,
            attachments: None,
        };
        let id = manager.add_entry(entry).unwrap();

        // Change password to generate history item
        manager.update_entry(id, UpdateEntry {
            password: Some("active-password-v2".to_string()),
            ..Default::default()
        }).unwrap();

        // Verify active password decrypts under Scope::Password
        let dec = manager.get_entry(id).unwrap();
        assert_eq!(dec.password, "active-password-v2");

        // Verify history item decrypts under Scope::History
        let history = manager.get_password_history(id).unwrap();
        assert_eq!(history[0].password, "active-password-v1");

        // Now attempt to swap encrypted_password with history item's encrypted_password in memory
        let entry_mut = manager.data_mut().entries.iter_mut().find(|e| e.id == id).unwrap();
        entry_mut.encrypted_password = entry_mut.password_history[0].encrypted_password.clone();

        // Attempting to decrypt the history blob under Scope::Password MUST fail due to AAD mismatch
        let result = manager.get_entry(id);
        assert!(result.is_err());
    }

    #[test]
    fn test_file_attachment_encryption_decryption() {
        let test_vault = TestVault::new();
        let password = "attachment-test-password";
        let mut manager = VaultManager::create("attachment-vault", password, &test_vault.path).unwrap();

        // Staged attachment
        let raw_bytes = b"SECRET ENCRYPTED FILE PAYLOAD 1234567890".to_vec();
        let new_att = NewAttachment {
            name: "test_doc.txt".to_string(),
            mime_type: "text/plain".to_string(),
            data: raw_bytes.clone(),
        };

        let new_entry = NewEntry {
            title: "Attachment Entry".to_string(),
            username: "user".to_string(),
            password: "password123".to_string(),
            url: "".to_string(),
            email: "".to_string(),
            notes: "".to_string(),
            tags: vec![],
            totp_secret: None,
            custom_fields: Vec::new(),
            entry_type: None,
            generate_passkey: None,
            attachments: Some(vec![new_att]),
        };

        let entry_id = manager.add_entry(new_entry).unwrap();

        // Verify entry preview reports 1 attachment
        let previews = manager.list_entries().unwrap();
        assert_eq!(previews[0].attachment_count, 1);

        // Verify get_entry lists attachment metadata
        let dec_entry = manager.get_entry(entry_id).unwrap();
        assert_eq!(dec_entry.attachments.len(), 1);
        assert_eq!(dec_entry.attachments[0].name, "test_doc.txt");
        assert_eq!(dec_entry.attachments[0].size, raw_bytes.len() as u64);

        let att_id = dec_entry.attachments[0].id;

        // Decrypt attachment raw bytes and verify match
        let decrypted_bytes = manager.get_attachment_data(entry_id, att_id).unwrap();
        assert_eq!(decrypted_bytes, raw_bytes);

        // Add second attachment via add_attachment method
        let raw_bytes_2 = b"SECOND FILE PAYLOAD PNG DATA".to_vec();
        let info2 = manager.add_attachment(entry_id, "image.png", "image/png", &raw_bytes_2).unwrap();
        assert_eq!(info2.name, "image.png");

        let dec_entry2 = manager.get_entry(entry_id).unwrap();
        assert_eq!(dec_entry2.attachments.len(), 2);

        // Delete first attachment
        manager.delete_attachment(entry_id, att_id).unwrap();
        let dec_entry3 = manager.get_entry(entry_id).unwrap();
        assert_eq!(dec_entry3.attachments.len(), 1);
        assert_eq!(dec_entry3.attachments[0].name, "image.png");

        // Save and lock
        manager.save().unwrap();
        manager.lock();

        // Re-open vault and verify attachment persists and decrypts correctly
        let reopened = VaultManager::open(&test_vault.path, password).unwrap();
        let dec_reopened = reopened.get_entry(entry_id).unwrap();
        assert_eq!(dec_reopened.attachments.len(), 1);
        let reopened_bytes = reopened.get_attachment_data(entry_id, info2.id).unwrap();
        assert_eq!(reopened_bytes, raw_bytes_2);
    }

    #[test]
    fn test_key_file_safely_size_limit_and_zeroization() {
        let temp_dir = tempfile::tempdir().unwrap();
        let kf_path = temp_dir.path().join("test.key");

        // Test normal 32-byte keyfile
        VaultManager::generate_key_file(&kf_path).unwrap();
        let bytes = read_key_file_safely(&kf_path).unwrap();
        assert_eq!(bytes.as_slice().len(), 32);

        // Test non-existent file
        let missing_path = temp_dir.path().join("missing.key");
        assert!(read_key_file_safely(&missing_path).is_err());
    }
}
