//! Local policy transitions are transactional and never merged from peers.
use super::storage::StorageSession;
use crate::crypto::SubKeys;
use crate::error::VaultError;
use crate::vault::VaultManager;

#[derive(serde::Serialize)]
pub struct ProtectionInfo {
    pub protected: bool,
    pub usb_bound: bool,
    pub recovery_enabled: bool,
}

impl VaultManager {
    pub(crate) fn replace_paired_data(
        &mut self,
        mut data: super::types::VaultData,
        salt: [u8; 32],
        keys: SubKeys,
    ) -> crate::Result<()> {
        data.settings.emergency_kit_audit = self.data.settings.emergency_kit_audit.clone();
        let mut next = self.storage.clone().ok_or(VaultError::VaultLocked)?;
        let old = next.clone();
        next.preserve_recovery(&old, &keys)?;
        next.wrap_password(&keys)?;
        let old_data = std::mem::replace(&mut self.data, data);
        let old_keys = self.keys.replace(keys);
        let old_salt = self.salt;
        let old_storage = self.storage.replace(next);
        self.salt = salt;
        if let Err(e) = self.save() {
            self.data = old_data;
            self.keys = old_keys;
            self.salt = old_salt;
            self.storage = old_storage;
            return Err(e);
        }
        self.rebuild_search_index();
        Ok(())
    }
    pub fn protection_info(&self) -> ProtectionInfo {
        ProtectionInfo {
            protected: self.storage.is_some(),
            usb_bound: self
                .storage
                .as_ref()
                .is_some_and(|s| s.header.usb_marker.is_some()),
            recovery_enabled: self
                .storage
                .as_ref()
                .is_some_and(|s| s.header.recovery.is_some()),
        }
    }

    pub(crate) fn ensure_local_protection(
        &mut self,
        password: &str,
        keyfile: Option<&[u8]>,
    ) -> crate::Result<()> {
        if self.storage.is_some() {
            return Ok(());
        }
        if self.hardware2fa.is_some() {
            return Err(VaultError::InvalidState("Disable legacy hardware 2FA with its physical key before migrating local protection".into()));
        }
        let keys = self.keys.as_mut().ok_or(VaultError::VaultLocked)?;
        // The inner sync representation must also resist known-password copying.
        keys.vault_key.bytes = crate::crypto::kdf::generate_salt();
        self.storage = Some(StorageSession::new(password, keyfile, None, keys)?);
        self.biometric = None;
        Ok(())
    }

    pub fn set_usb_binding(
        &mut self,
        password: &str,
        keyfile: Option<&[u8]>,
        usb_id: Option<&str>,
    ) -> crate::Result<()> {
        if !self.verify_master_password_with_keyfile(password, keyfile)? {
            return Err(VaultError::InvalidPassword);
        }
        let serial = usb_id.map(super::usb::selected_serial).transpose()?;
        let previous = self.storage.clone();
        let old_keys = self.keys.clone();
        let old_bio = self.biometric.clone();
        let result = (|| {
            self.ensure_local_protection(password, keyfile)?;
            let current = self.storage.as_ref().ok_or(VaultError::VaultLocked)?;
            if current.header.recovery.is_none() {
                return Err(VaultError::InvalidState(
                    "Create and save a recovery kit before enabling USB binding".into(),
                ));
            }
            let keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;
            let mut session = StorageSession::new(password, keyfile, serial.as_deref(), keys)?;
            // Preserve the same recovery secret while binding its new slot to the new replica header.
            session.preserve_recovery(current, keys)?;
            self.storage = Some(session);
            self.save()
        })();
        if result.is_err() {
            self.storage = previous;
            self.keys = old_keys;
            self.biometric = old_bio;
        }
        result
    }

    /// Encrypted transport snapshot without local unlock slots or recovery audit records.
    pub fn sync_bytes(&self) -> crate::Result<Vec<u8>> {
        let keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;
        let mut data = self.data.clone();
        data.settings.emergency_kit_audit = None;
        data.settings.hardware_password_key = None;
        let header = super::format::FileHeader {
            version: super::format::FORMAT_VERSION,
            flags: 0,
            salt: self.salt,
            kdf_params: Default::default(),
        };
        let plain = zeroize::Zeroizing::new(
            rmp_serde::to_vec(&data).map_err(|e| VaultError::SerializationError(e.to_string()))?,
        );
        let blob =
            crate::crypto::encrypt_vault_with_aad(&plain, &keys.vault_key, &header.aad_bytes()?)?;
        let mut payload = blob.nonce;
        payload.extend(blob.ciphertext);
        super::format::VaultFile {
            header,
            hmac: None,
            biometric: None,
            hardware2fa: None,
            encrypted_payload: payload,
        }
        .to_bytes()
    }

    pub fn from_sync_data(
        path: &std::path::Path,
        data: super::types::VaultData,
        salt: [u8; 32],
        keys: SubKeys,
        password: &str,
    ) -> crate::Result<Self> {
        let storage = StorageSession::new(password, None, None, &keys)?;
        let mut manager = Self {
            path: path.to_owned(),
            data,
            keys: Some(keys),
            salt,
            biometric: None,
            hardware2fa: None,
            storage: Some(storage),
            search_index: Default::default(),
        };
        manager.data.settings.emergency_kit_audit = None;
        manager.rebuild_search_index();
        manager.save()?;
        Ok(manager)
    }
}
