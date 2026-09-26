//! Authenticated local storage envelope. Never include this layer in sync payloads.

use crate::crypto::cipher::{EncryptedBlob, decrypt_vault_with_aad, encrypt_vault_with_aad};
use crate::crypto::{SubKeys, VaultKey, derive_master_key_with_keyfile, derive_subkeys};
use crate::error::VaultError;
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use uuid::Uuid;
use zeroize::Zeroizing;

pub const MAGIC: &[u8; 4] = b"YNS2";
const MAX_HEADER: usize = 16384;
const MAX_FILE: usize = 520 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
pub struct RecoverySlot {
    pub id: Uuid,
    pub wrapped: EncryptedBlob,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct StorageHeader {
    pub version: u16,
    pub replica_id: Uuid,
    pub salt: [u8; 32],
    pub usb_marker: Option<[u8; 32]>,
    pub password: EncryptedBlob,
    pub recovery: Option<RecoverySlot>,
}

#[derive(Clone)]
pub(crate) struct StorageSession {
    pub header: StorageHeader,
    pub key: VaultKey,
    pub unlock_key: VaultKey,
    pub recovery_key: Option<VaultKey>,
    pub disk_header: Option<[u8; 32]>,
}

pub fn is_protected(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

pub(crate) fn read_bounded(path: &std::path::Path) -> crate::Result<Vec<u8>> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() > MAX_FILE as u64 {
        return Err(invalid());
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_FILE {
        return Err(invalid());
    }
    Ok(bytes)
}

fn invalid() -> VaultError {
    VaultError::InvalidFormat("Invalid local protection envelope".into())
}

pub fn parse(bytes: &[u8]) -> crate::Result<(StorageHeader, EncryptedBlob)> {
    if bytes.len() < 12 || bytes.len() > MAX_FILE || !is_protected(bytes) {
        return Err(invalid());
    }
    let len = u32::from_le_bytes(bytes[4..8].try_into().map_err(|_| invalid())?) as usize;
    if len > MAX_HEADER || bytes.len() < 8 + len + 40 {
        return Err(invalid());
    }
    let header: StorageHeader =
        serde_json::from_slice(&bytes[8..8 + len]).map_err(|_| invalid())?;
    if header.version != 2 || header.replica_id.is_nil() {
        return Err(invalid());
    }
    check_blob(&header.password)?;
    if let Some(slot) = &header.recovery {
        if slot.id.is_nil() {
            return Err(invalid());
        }
        check_blob(&slot.wrapped)?;
    }
    let payload = &bytes[8 + len..];
    Ok((
        header,
        EncryptedBlob {
            nonce: payload[..24].to_vec(),
            ciphertext: payload[24..].to_vec(),
        },
    ))
}

fn check_blob(blob: &EncryptedBlob) -> crate::Result<()> {
    if blob.nonce.len() != 24 || blob.ciphertext.len() < 16 || blob.ciphertext.len() > 512 {
        return Err(invalid());
    }
    Ok(())
}

fn aad(header: &StorageHeader, purpose: &str) -> Vec<u8> {
    let mut out = b"yntra-local-envelope-v2".to_vec();
    out.extend_from_slice(header.replica_id.as_bytes());
    out.extend_from_slice(&header.salt);
    out.push(header.usb_marker.is_some() as u8);
    if let Some(marker) = header.usb_marker {
        out.extend_from_slice(&marker);
    }
    out.extend_from_slice(purpose.as_bytes());
    out
}

pub(crate) fn usb_marker(salt: &[u8; 32], serial: &str) -> [u8; 32] {
    *blake3::keyed_hash(salt, serial.as_bytes()).as_bytes()
}

fn unlock_key(
    header: &StorageHeader,
    password: &str,
    keyfile: Option<&[u8]>,
    serial: Option<&str>,
) -> crate::Result<VaultKey> {
    if password.is_empty() {
        return Err(VaultError::InvalidPassword);
    }
    let mut material = Zeroizing::new(Vec::new());
    material.extend_from_slice(b"yntra-local-factors-v2");
    let kf = keyfile.unwrap_or_default();
    material.extend_from_slice(&(kf.len() as u64).to_le_bytes());
    material.extend_from_slice(kf);
    if let Some(marker) = header.usb_marker {
        let serial = serial.ok_or_else(|| {
            VaultError::InvalidState("Connect the bound USB device or use recovery".into())
        })?;
        if !bool::from(marker.ct_eq(&usb_marker(&header.salt, serial))) {
            return Err(VaultError::InvalidPassword);
        }
        material.extend_from_slice(serial.as_bytes());
    }
    Ok(derive_subkeys(&derive_master_key_with_keyfile(
        password.as_bytes(),
        Some(&material),
        &header.salt,
    )?)?
    .vault_key
    .clone())
}

pub(crate) fn connected_serial(header: &StorageHeader) -> crate::Result<Option<String>> {
    let Some(marker) = header.usb_marker else {
        return Ok(None);
    };
    let matches: Vec<_> = super::usb::list_usb_devices()?
        .into_iter()
        .filter(|d| bool::from(marker.ct_eq(&usb_marker(&header.salt, &d.serial))))
        .collect();
    if matches.len() != 1 {
        return Err(VaultError::InvalidState(
            "Connect the bound USB device or use recovery".into(),
        ));
    }
    Ok(Some(matches[0].serial.clone()))
}

impl StorageSession {
    pub fn new(
        password: &str,
        keyfile: Option<&[u8]>,
        serial: Option<&str>,
        keys: &SubKeys,
    ) -> crate::Result<Self> {
        let salt = crate::crypto::kdf::generate_salt();
        let header = StorageHeader {
            version: 2,
            replica_id: Uuid::new_v4(),
            salt,
            usb_marker: serial.map(|s| usb_marker(&salt, s)),
            password: EncryptedBlob {
                nonce: vec![],
                ciphertext: vec![],
            },
            recovery: None,
        };
        let unlock_key = unlock_key(&header, password, keyfile, serial)?;
        let mut session = Self {
            header,
            key: VaultKey {
                bytes: crate::crypto::kdf::generate_salt(),
            },
            unlock_key,
            recovery_key: None,
            disk_header: None,
        };
        session.wrap_password(keys)?;
        Ok(session)
    }

    fn package(&self, keys: &SubKeys, include_recovery: bool) -> Zeroizing<Vec<u8>> {
        let mut out = Zeroizing::new(Vec::with_capacity(224));
        out.extend_from_slice(&self.key.bytes);
        let raw = Zeroizing::new(keys.to_bytes());
        out.extend_from_slice(&raw);
        if include_recovery {
            if let Some(key) = &self.recovery_key {
                out.extend_from_slice(&key.bytes);
            }
        }
        out
    }

    pub fn wrap_password(&mut self, keys: &SubKeys) -> crate::Result<()> {
        self.header.password = encrypt_vault_with_aad(
            &self.package(keys, true),
            &self.unlock_key,
            &aad(&self.header, "password"),
        )?;
        Ok(())
    }

    pub fn open(
        header: StorageHeader,
        password: &str,
        keyfile: Option<&[u8]>,
        serial: Option<&str>,
    ) -> crate::Result<(Self, SubKeys)> {
        let unlock_key = unlock_key(&header, password, keyfile, serial)?;
        let raw = Zeroizing::new(decrypt_vault_with_aad(
            &header.password,
            &unlock_key,
            &aad(&header, "password"),
        )?);
        if raw.len() != 192 && raw.len() != 224 {
            return Err(invalid());
        }
        let key = VaultKey {
            bytes: raw[..32].try_into().map_err(|_| invalid())?,
        };
        let keys = SubKeys::from_bytes(&raw[32..192])?;
        let recovery_key = if raw.len() == 224 {
            Some(VaultKey {
                bytes: raw[192..224].try_into().map_err(|_| invalid())?,
            })
        } else {
            None
        };
        if recovery_key.is_some() != header.recovery.is_some() {
            return Err(invalid());
        }
        let disk_header =
            Some(*blake3::hash(&serde_json::to_vec(&header).map_err(|_| invalid())?).as_bytes());
        Ok((
            Self {
                header,
                key,
                unlock_key,
                recovery_key,
                disk_header,
            },
            keys,
        ))
    }

    pub fn verify(&self, password: &str, keyfile: Option<&[u8]>) -> crate::Result<bool> {
        let serial = connected_serial(&self.header)?;
        let key = unlock_key(&self.header, password, keyfile, serial.as_deref())?;
        Ok(bool::from(key.ct_eq(&self.unlock_key)))
    }

    pub fn change_password(
        &mut self,
        password: &str,
        keyfile: Option<&[u8]>,
        keys: &SubKeys,
    ) -> crate::Result<()> {
        let serial = connected_serial(&self.header)?;
        self.unlock_key = unlock_key(&self.header, password, keyfile, serial.as_deref())?;
        self.wrap_password(keys)
    }

    pub fn seal(&self, inner: &[u8]) -> crate::Result<Vec<u8>> {
        let header = serde_json::to_vec(&self.header).map_err(|_| invalid())?;
        if header.len() > MAX_HEADER {
            return Err(invalid());
        }
        let mut out = MAGIC.to_vec();
        out.extend_from_slice(&(header.len() as u32).to_le_bytes());
        out.extend_from_slice(&header);
        let blob = encrypt_vault_with_aad(inner, &self.key, &out)?;
        out.extend_from_slice(&blob.nonce);
        out.extend_from_slice(&blob.ciphertext);
        Ok(out)
    }

    pub fn decrypt(&self, bytes: &[u8]) -> crate::Result<Zeroizing<Vec<u8>>> {
        let (header, blob) = parse(bytes)?;
        if serde_json::to_vec(&header).map_err(|_| invalid())?
            != serde_json::to_vec(&self.header).map_err(|_| invalid())?
        {
            return Err(VaultError::InvalidState(
                "Local protection changed on disk; lock and unlock the vault again".into(),
            ));
        }
        let header_len =
            u32::from_le_bytes(bytes[4..8].try_into().map_err(|_| invalid())?) as usize;
        Ok(decrypt_vault_with_aad(
            &blob,
            &self.key,
            &bytes[..8 + header_len],
        )?)
    }

    pub fn rotate_recovery(&mut self, keys: &SubKeys) -> crate::Result<(Uuid, Zeroizing<String>)> {
        self.key = VaultKey {
            bytes: crate::crypto::kdf::generate_salt(),
        };
        let key = VaultKey {
            bytes: crate::crypto::kdf::generate_salt(),
        };
        let secret = Zeroizing::new(data_encoding::HEXLOWER.encode(&key.bytes));
        let id = Uuid::new_v4();
        let wrapped = encrypt_vault_with_aad(
            &self.package(keys, false),
            &key,
            &aad(&self.header, &format!("recovery:{id}")),
        )?;
        self.header.recovery = Some(RecoverySlot { id, wrapped });
        self.recovery_key = Some(key);
        self.wrap_password(keys)?;
        Ok((id, secret))
    }

    pub fn revoke_recovery(&mut self, keys: &SubKeys) -> crate::Result<()> {
        self.key = VaultKey {
            bytes: crate::crypto::kdf::generate_salt(),
        };
        self.header.recovery = None;
        self.recovery_key = None;
        self.wrap_password(keys)
    }

    pub fn preserve_recovery(&mut self, old: &Self, keys: &SubKeys) -> crate::Result<()> {
        self.disk_header = old.disk_header;
        if let (Some(slot), Some(key)) = (&old.header.recovery, &old.recovery_key) {
            self.header.replica_id = old.header.replica_id;
            let wrapped = encrypt_vault_with_aad(
                &self.package(keys, false),
                key,
                &aad(&self.header, &format!("recovery:{}", slot.id)),
            )?;
            self.header.recovery = Some(RecoverySlot {
                id: slot.id,
                wrapped,
            });
            self.recovery_key = Some(key.clone());
            self.wrap_password(keys)?;
        }
        Ok(())
    }

    pub fn recover(
        header: StorageHeader,
        kit: Uuid,
        secret: &str,
    ) -> crate::Result<(Self, SubKeys)> {
        let slot = header
            .recovery
            .as_ref()
            .filter(|s| s.id == kit)
            .ok_or_else(|| {
                VaultError::InvalidState("Recovery kit was replaced or revoked".into())
            })?;
        let bytes = Zeroizing::new(
            data_encoding::HEXLOWER
                .decode(secret.as_bytes())
                .map_err(|_| invalid())?,
        );
        let recovery_key = VaultKey {
            bytes: bytes.as_slice().try_into().map_err(|_| invalid())?,
        };
        let raw = Zeroizing::new(decrypt_vault_with_aad(
            &slot.wrapped,
            &recovery_key,
            &aad(&header, &format!("recovery:{kit}")),
        )?);
        if raw.len() != 192 {
            return Err(invalid());
        }
        let key = VaultKey {
            bytes: raw[..32].try_into().map_err(|_| invalid())?,
        };
        let keys = SubKeys::from_bytes(&raw[32..])?;
        let disk_header =
            Some(*blake3::hash(&serde_json::to_vec(&header).map_err(|_| invalid())?).as_bytes());
        Ok((
            Self {
                header,
                key,
                unlock_key: VaultKey { bytes: [0; 32] },
                recovery_key: Some(recovery_key),
                disk_header,
            },
            keys,
        ))
    }

    pub fn check_disk_header(&self, path: &std::path::Path) -> crate::Result<()> {
        if !path.exists() {
            return Ok(());
        }
        let bytes = read_bounded(path)?;
        let current = if is_protected(&bytes) {
            let (header, _) = parse(&bytes)?;
            Some(*blake3::hash(&serde_json::to_vec(&header).map_err(|_| invalid())?).as_bytes())
        } else {
            None
        };
        if current != self.disk_header {
            return Err(VaultError::InvalidState(
                "Local protection changed in another session; lock and unlock before saving".into(),
            ));
        }
        Ok(())
    }

    pub fn mark_persisted(&mut self) -> crate::Result<()> {
        self.disk_header = Some(
            *blake3::hash(&serde_json::to_vec(&self.header).map_err(|_| invalid())?).as_bytes(),
        );
        Ok(())
    }
}

pub(crate) fn atomic_write(path: &std::path::Path, bytes: &[u8]) -> crate::Result<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path)
        .map_err(|e| VaultError::IoError(e.error))?;
    Ok(())
}

pub(crate) fn atomic_create(path: &std::path::Path, bytes: &[u8]) -> crate::Result<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist_noclobber(path)
        .map_err(|e| VaultError::IoError(e.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn keys() -> SubKeys {
        SubKeys::from_bytes(&[42u8; 160]).unwrap()
    }
    #[test]
    fn binding_is_cryptographic_and_recovery_survives_binding() {
        let keys = keys();
        let mut plain = StorageSession::new("password 123", None, None, &keys).unwrap();
        let (kit, secret) = plain.rotate_recovery(&keys).unwrap();
        let mut bound =
            StorageSession::new("password 123", None, Some("USB123456"), &keys).unwrap();
        bound.preserve_recovery(&plain, &keys).unwrap();
        let bytes = bound.seal(b"encrypted inner vault").unwrap();
        let (header, _) = parse(&bytes).unwrap();
        assert!(StorageSession::open(header.clone(), "password 123", None, None).is_err());
        assert!(
            StorageSession::open(header.clone(), "password 123", None, Some("USB654321")).is_err()
        );
        assert!(
            StorageSession::open(header.clone(), "wrong password", None, Some("USB123456"))
                .is_err()
        );
        let (opened, restored) =
            StorageSession::open(header.clone(), "password 123", None, Some("USB123456")).unwrap();
        assert!(bool::from(restored.ct_eq(&keys)));
        assert_eq!(&*opened.decrypt(&bytes).unwrap(), b"encrypted inner vault");
        let (recovery, _) = StorageSession::recover(header, kit, &secret).unwrap();
        assert_eq!(
            &*recovery.decrypt(&bytes).unwrap(),
            b"encrypted inner vault"
        );
        let mut changed = bytes.clone();
        *changed.last_mut().unwrap() ^= 1;
        assert!(opened.decrypt(&changed).is_err());
    }
    #[test]
    fn header_limits_and_nonce_lengths_reject_before_decryption() {
        assert!(parse(b"YNS2").is_err());
        assert!(parse(b"YNS2\xff\xff\xff\xffxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx").is_err());
        let s = StorageSession::new("password 123", None, None, &keys()).unwrap();
        let mut h = s.header.clone();
        h.password.nonce.clear();
        let raw = serde_json::to_vec(&h).unwrap();
        let mut bytes = MAGIC.to_vec();
        bytes.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        bytes.extend(raw);
        bytes.extend([0; 40]);
        assert!(parse(&bytes).is_err());
    }
    #[test]
    fn old_storage_key_cannot_open_rotated_payload() {
        let keys = keys();
        let mut s = StorageSession::new("password 123", None, None, &keys).unwrap();
        let (old_id, old_secret) = s.rotate_recovery(&keys).unwrap();
        let old_header = s.header.clone();
        s.rotate_recovery(&keys).unwrap();
        let bytes = s.seal(b"new contents").unwrap();
        let (old, _) = StorageSession::recover(old_header, old_id, &old_secret).unwrap();
        assert!(old.decrypt(&bytes).is_err());
        s.revoke_recovery(&keys).unwrap();
        assert!(s.header.recovery.is_none());
    }
}
