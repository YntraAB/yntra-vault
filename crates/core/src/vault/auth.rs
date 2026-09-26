//! Biometric and Hardware 2FA authentication operations for VaultManager.
//!
//! Orchestrates biometric unlock (Windows Hello, Touch ID, PAM) and
//! Hardware 2FA (YubiKey challenge-response, FIDO2 HMAC-secret) enrollment and unlocks.

use std::path::Path;

use crate::crypto::derive_subkeys;
use crate::error::VaultError;
use crate::vault::format::{FileHeader, KdfParams, VaultFile, FORMAT_VERSION};
use crate::vault::manager::{read_key_file_safely, VaultManager};

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct Hardware2FaChallengeInfo {
    pub enabled: bool,
    pub payload_key_bound: bool,
    pub protocol: crate::crypto::hardware2fa::Hardware2FaProtocol,
    pub key_name: String,
    pub challenge_salt: Vec<u8>,
    pub credential_id: Vec<u8>,
}

impl VaultManager {
    /// Open an existing vault using enrolled Biometric Unlock (Windows Hello, Touch ID, PAM).
    pub fn open_with_biometric(path: &Path) -> crate::Result<Self> {
        Self::open_with_biometric_with_hwnd(path, None)
    }

    /// Open an existing vault using enrolled Biometric Unlock with optional parent window HWND.
    pub fn open_with_biometric_with_hwnd(path: &Path, hwnd_override: Option<isize>) -> crate::Result<Self> {
        let file_bytes = super::storage::read_bounded(path)
            .map_err(|e| VaultError::VaultNotFound(format!("{}: {}", path.display(), e)))?;

        let vault_file = VaultFile::from_bytes(&file_bytes)?;

        if vault_file.hardware2fa.is_some() {
            return Err(VaultError::Hardware2FaRequired);
        }

        let bio_header = vault_file
            .biometric
            .as_ref()
            .ok_or_else(|| VaultError::BiometricNotAvailable("No biometric data in vault file".into()))?;
        let aad = vault_file.header.aad_bytes()?;
        let subkeys = crate::crypto::biometric::unlock_from_embedded_header_with_hwnd(bio_header, &aad, hwnd_override)?;
        Self::from_decrypted_vault_file(path, vault_file, subkeys)
    }

    /// Enroll biometrics for the current open vault (Embedded in single .vdb file).
    /// Enforces mutual exclusivity with Hardware 2FA to prevent conflicting security levels.
    pub fn enable_biometric(&mut self) -> crate::Result<()> {
        self.enable_biometric_with_hwnd(None)
    }

    /// Enroll biometrics for the current open vault with optional parent window HWND.
    /// Prompts for Windows Hello / OS biometric verification to confirm user identity before enrolling.
    pub fn enable_biometric_with_hwnd(&mut self, hwnd_override: Option<isize>) -> crate::Result<()> {
        if self.storage.is_some() { return Err(VaultError::InvalidState("Local USB/recovery protection requires password unlock; biometric fallback is unavailable".into())); }
        if self.hardware2fa.is_some() {
            return Err(VaultError::InvalidState(
                "Cannot enable Biometric unlock while Hardware 2FA is active. Disable Hardware 2FA first.".into()
            ));
        }

        // Prompt for OS biometric confirmation before enrolling
        crate::crypto::biometric::request_user_consent_with_hwnd(
            "Confirm your identity to enable biometric unlock",
            hwnd_override,
        )?;

        let keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;
        let flags = crate::vault::format::FLAG_HAS_BIOMETRIC;
        let temp_header = FileHeader {
            version: FORMAT_VERSION,
            flags,
            salt: self.salt,
            kdf_params: KdfParams::default(),
        };
        let aad = temp_header.aad_bytes()?;
        let bio_header = crate::crypto::biometric::create_embedded_biometric_header(keys, &aad)?;
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
        if password.is_empty() {
            return Err(VaultError::InvalidPassword);
        }
        if hardware_response.is_empty() {
            return Err(VaultError::Hardware2FaRequired);
        }

        let file_bytes = super::storage::read_bounded(path)
            .map_err(|e| VaultError::VaultNotFound(format!("{}: {}", path.display(), e)))?;

        let vault_file = VaultFile::from_bytes(&file_bytes)?;

        let key_file_bytes = match key_file_path {
            Some(kf_path) => Some(read_key_file_safely(kf_path)?),
            None => None,
        };

        let subkeys = if let Some(ref hw_headers) = vault_file.hardware2fa {
            let aad = vault_file.header.aad_bytes()?;
            crate::crypto::hardware2fa::unlock_from_embedded_hardware2fa_headers(
                hw_headers,
                &aad,
                password.as_bytes(),
                key_file_bytes.as_ref().map(|b| b.as_slice()),
                hardware_response,
            )?
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

        Self::from_decrypted_vault_file(path, vault_file, subkeys)
    }

    /// Enroll Hardware 2FA for the current open vault.
    /// Replaced by `enable_hardware2fa_with_password` which enforces Factor 1 verification.
    pub fn enable_hardware2fa(
        &mut self,
        _protocol: crate::crypto::hardware2fa::Hardware2FaProtocol,
        _key_name: &str,
        _hardware_response: &[u8],
    ) -> crate::Result<()> {
        Err(VaultError::InvalidState(
            "Master password verification is required to enroll Hardware 2FA. Use enable_hardware2fa_with_password.".into(),
        ))
    }

    /// Enroll Hardware 2FA for the current open vault with master password verification.
    /// Requires verification of master password (Factor 1) and hardware response (Factor 2).
    /// Disables and cleans up Biometric unlock to enforce two-factor security and prevent AAD tag divergence.
    #[allow(clippy::too_many_arguments)]
    pub fn enable_hardware2fa_with_password(
        &mut self,
        password: &str,
        key_file_path: Option<&Path>,
        protocol: crate::crypto::hardware2fa::Hardware2FaProtocol,
        key_name: &str,
        challenge_salt: [u8; 32],
        credential_id: Vec<u8>,
        hardware_response: &[u8],
    ) -> crate::Result<()> {
        if self.storage.is_some() { return Err(VaultError::InvalidState("Legacy hardware 2FA cannot be combined with local USB/recovery protection".into())); }
        if password.is_empty() {
            return Err(VaultError::InvalidPassword);
        }
        let old_keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?.clone();
        let key_file_bytes = key_file_path.map(read_key_file_safely).transpose()?;
        if !self.verify_master_password_with_keyfile(password, key_file_bytes.as_ref().map(|b| b.as_slice()))? {
            return Err(VaultError::InvalidPassword);
        }
        let old_data = self.data.clone();
        let old_bio = self.biometric.clone();
        let old_hardware = self.hardware2fa.clone();
        let result = (|| {
            let mut keys = old_keys.clone();
            // A password-derived payload key can bypass a hardware envelope entirely.
            // On first activation (or explicit re-enrollment of a legacy vault), rotate it.
            if self.data.settings.hardware_password_key.is_none() {
                keys.vault_key.bytes = crate::crypto::kdf::generate_salt();
                self.data.settings.hardware_password_key = Some(crate::crypto::encrypt_vault_with_aad(
                    &old_keys.vault_key.bytes, &keys.vault_key, b"yntra-hardware-password-key-v1",
                )?);
                // Old slots wrap the obsolete key. Re-enroll any additional keys explicitly.
                self.hardware2fa = None;
                self.data.settings.trusted_devices.clear();
            }
            self.biometric = None;
            let header = FileHeader { version: super::format::HARDWARE_BOUND_VERSION, flags: crate::vault::format::FLAG_HAS_HARDWARE_2FA,
                salt: self.salt, kdf_params: KdfParams::default() };
            let slot = crate::crypto::hardware2fa::create_embedded_hardware2fa_header(
                &keys, &header.aad_bytes()?, protocol, key_name, challenge_salt, credential_id,
                password.as_bytes(), key_file_bytes.as_ref().map(|b| b.as_slice()), hardware_response,
            )?;
            self.hardware2fa.get_or_insert_with(Vec::new).push(slot);
            self.keys = Some(keys);
            self.save()
        })();
        if result.is_err() {
            self.keys = Some(old_keys); self.data = old_data;
            self.biometric = old_bio; self.hardware2fa = old_hardware;
        }
        result
    }

    /// Remove the hardware factor only from an already authenticated, unlocked session.
    pub fn disable_hardware2fa(&mut self) -> crate::Result<()> {
        let old_keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?.clone();
        let old_data = self.data.clone();
        let old_hardware = self.hardware2fa.clone();
        let result = (|| {
            let mut keys = old_keys.clone();
            if let Some(blob) = &self.data.settings.hardware_password_key {
                let original = crate::crypto::decrypt_vault_with_aad(blob, &keys.vault_key, b"yntra-hardware-password-key-v1")?;
                keys.vault_key.bytes = original.as_slice().try_into().map_err(|_| VaultError::InvalidFormat("Invalid password key length".into()))?;
            }
            self.data.settings.hardware_password_key = None;
            self.data.settings.trusted_devices.clear();
            self.hardware2fa = None;
            self.keys = Some(keys);
            self.save()
        })();
        if result.is_err() { self.keys = Some(old_keys); self.data = old_data; self.hardware2fa = old_hardware; }
        result
    }

    /// Retrieve Hardware 2FA challenge configuration from an existing vault file.
    pub fn get_hardware2fa_challenge_info(path: &Path) -> crate::Result<Option<Hardware2FaChallengeInfo>> {
        let file_bytes = super::storage::read_bounded(path)
            .map_err(|e| VaultError::VaultNotFound(format!("{}: {}", path.display(), e)))?;
        if super::storage::is_protected(&file_bytes) {
            super::storage::parse(&file_bytes)?;
            return Ok(None);
        }
        let vault_file = VaultFile::from_bytes(&file_bytes)?;

        if let Some(ref hw_headers) = vault_file.hardware2fa
            && let Some(first) = hw_headers.first() {
                return Ok(Some(Hardware2FaChallengeInfo {
                    enabled: true,
                    payload_key_bound: vault_file.header.version >= super::format::HARDWARE_BOUND_VERSION,
                    protocol: first.protocol,
                    key_name: first.key_name.clone(),
                    challenge_salt: first.challenge_salt.to_vec(),
                    credential_id: first.credential_id.clone(),
                }));
            }
        Ok(None)
    }

    /// Check if Hardware 2FA is enabled for the current vault.
    pub fn is_hardware2fa_enabled(&self) -> bool {
        self.hardware2fa.is_some()
    }

    /// Check if Hardware 2FA is enrolled inside a .vdb file at path.
    pub fn is_hardware2fa_enabled_file(vault_path: &Path) -> bool {
        if let Ok(file_bytes) = super::storage::read_bounded(vault_path)
            && let Ok(vault_file) = VaultFile::from_bytes(&file_bytes) {
                return vault_file.hardware2fa.is_some();
            }
        false
    }

    /// Check whether biometric unlock is enrolled inside a .vdb file at path.
    pub fn is_biometric_enabled_file(vault_path: &Path) -> bool {
        if let Ok(file_bytes) = super::storage::read_bounded(vault_path)
            && let Ok(vault_file) = VaultFile::from_bytes(&file_bytes) {
                return vault_file.biometric.is_some();
            }
        false
    }
}
