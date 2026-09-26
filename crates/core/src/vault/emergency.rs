//! Versioned recovery secrets, independent of the user's master password.
use crate::{
    error::VaultError,
    vault::{
        VaultManager,
        types::{EmergencyKitAudit, EmergencyKitAuditEntry},
    },
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

#[derive(Serialize, Deserialize, Clone, PartialEq, Zeroize, ZeroizeOnDrop)]
pub struct EmergencyShare {
    pub share_index: u8,
    pub label: String,
    pub share_data: String,
}
impl std::fmt::Debug for EmergencyShare {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmergencyShare")
            .field("share_index", &self.share_index)
            .field("share_data", &"[redacted]")
            .finish()
    }
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EmergencyKit {
    pub vault_id: Uuid,
    pub vault_name: String,
    pub created_at: chrono::DateTime<Utc>,
    pub generated_at: chrono::DateTime<Utc>,
    pub format_version: u16,
    pub total_entries: usize,
    pub shares: Vec<EmergencyShare>,
    pub verification_hash: String,
    pub document_markdown: String,
}
fn share_error() -> VaultError {
    VaultError::InvalidFormat("Invalid, mixed, or damaged recovery shares".into())
}
pub fn export_recovery_share(path: &std::path::Path, share: &str) -> crate::Result<()> {
    let document = recovery_share_document(share)?;
    super::storage::atomic_create(path, document.as_bytes())
}
pub fn recovery_share_document(share: &str) -> crate::Result<Zeroizing<String>> {
    let _ = decode_share(share)?;
    let document = Zeroizing::new(format!(
        "Yntra Vault recovery v2\n\n{}\n\nKeep this share separately from the other shares and your vault. Two different shares from this kit and a copy of the vault file restore access. Replaced kits may still open older backups.\n",
        share.trim()
    ));
    Ok(document)
}
fn encode_share(replica: Uuid, kit: Uuid, raw: &str) -> String {
    let body = Zeroizing::new(format!("YNTRA2:{replica}:{kit}:{raw}"));
    format!(
        "{}:{}",
        body.as_str(),
        data_encoding::HEXLOWER.encode(&blake3::hash(body.as_bytes()).as_bytes()[..8])
    )
}
fn decode_share(value: &str) -> crate::Result<(Uuid, Uuid, Zeroizing<String>)> {
    if value.len() > 512 {
        return Err(share_error());
    }
    let parts: Vec<_> = value.trim().split(':').collect();
    if parts.len() != 5 || parts[0] != "YNTRA2" {
        return Err(share_error());
    }
    let body = Zeroizing::new(parts[..4].join(":"));
    let check = data_encoding::HEXLOWER.encode(&blake3::hash(body.as_bytes()).as_bytes()[..8]);
    if !bool::from(check.as_bytes().ct_eq(parts[4].as_bytes())) {
        return Err(share_error());
    }
    let (_, bytes) = crate::crypto::sharing::parse_share(parts[3])?;
    let bytes = Zeroizing::new(bytes);
    if bytes.len() != 32 {
        return Err(share_error());
    }
    Ok((
        Uuid::parse_str(parts[1]).map_err(|_| share_error())?,
        Uuid::parse_str(parts[2]).map_err(|_| share_error())?,
        Zeroizing::new(parts[3].to_owned()),
    ))
}
fn reconstruct(a: &str, b: &str) -> crate::Result<(Uuid, Uuid, Zeroizing<String>)> {
    let (replica, kit, a) = decode_share(a)?;
    let (replica_b, kit_b, b) = decode_share(b)?;
    if replica != replica_b || kit != kit_b {
        return Err(share_error());
    }
    let secret = Zeroizing::new(crate::crypto::sharing::reconstruct_secret(&a, &b)?);
    Ok((
        replica,
        kit,
        Zeroizing::new(data_encoding::HEXLOWER.encode(&secret)),
    ))
}
impl VaultManager {
    pub fn generate_emergency_kit(&mut self, password: &str) -> crate::Result<EmergencyKit> {
        self.generate_emergency_kit_with_keyfile(password, None)
    }
    pub fn generate_emergency_kit_with_keyfile(
        &mut self,
        password: &str,
        keyfile: Option<&[u8]>,
    ) -> crate::Result<EmergencyKit> {
        if !self.verify_master_password_with_keyfile(password, keyfile)? {
            return Err(VaultError::InvalidPassword);
        }
        let previous = self.storage.clone();
        let old_keys = self.keys.clone();
        let old_bio = self.biometric.clone();
        let old_audit = self.data.settings.emergency_kit_audit.clone();
        let result = (|| {
            self.ensure_local_protection(password, keyfile)?;
            let session = self.storage.as_mut().ok_or(VaultError::VaultLocked)?;
            let (kit, secret) =
                session.rotate_recovery(self.keys.as_ref().ok_or(VaultError::VaultLocked)?)?;
            let raw_secret = Zeroizing::new(
                data_encoding::HEXLOWER
                    .decode(secret.as_bytes())
                    .map_err(|_| share_error())?,
            );
            let raw_shares = Zeroizing::new(crate::crypto::sharing::split_secret(&raw_secret)?);
            let shares: Vec<_> = raw_shares
                .iter()
                .enumerate()
                .map(|(i, raw)| EmergencyShare {
                    share_index: (i + 1) as u8,
                    label: format!("Recovery share {}", i + 1),
                    share_data: encode_share(session.header.replica_id, kit, raw),
                })
                .collect();
            let (_, _, check) = reconstruct(&shares[0].share_data, &shares[1].share_data)?;
            if !bool::from(check.as_bytes().ct_eq(secret.as_bytes())) {
                return Err(share_error());
            }
            let now = Utc::now();
            let fingerprint = kit.to_string();
            let audit = self
                .data
                .settings
                .emergency_kit_audit
                .get_or_insert_with(EmergencyKitAudit::default);
            audit.active_fingerprint = fingerprint.clone();
            audit.last_generated_at = now;
            audit.generation_count += 1;
            audit.history.push(EmergencyKitAuditEntry {
                timestamp: now,
                fingerprint: fingerprint.clone(),
                action: "generated-v2".into(),
            });
            if audit.history.len() > 50 {
                audit.history.remove(0);
            }
            self.save()?;
            Ok(EmergencyKit {vault_id:self.data.metadata.id,vault_name:self.data.metadata.name.clone(),created_at:self.data.metadata.created_at,generated_at:now,format_version:2,total_entries:self.data.entries.len(),shares,verification_hash:fingerprint,document_markdown:"Recovery v2: save each share separately. Any two shares restore access to this vault copy without the old password or USB. A vault backup is also required. Replacing the kit revokes it for the updated file, not old backups. Never store all shares with the vault.".into()})
        })();
        if result.is_err() {
            self.storage = previous;
            self.keys = old_keys;
            self.biometric = old_bio;
            self.data.settings.emergency_kit_audit = old_audit;
        }
        result
    }
    pub fn get_emergency_kit_audit(&self) -> Option<EmergencyKitAudit> {
        self.data.settings.emergency_kit_audit.clone()
    }
    pub fn reset_emergency_kit_audit(&mut self) -> crate::Result<()> {
        Err(VaultError::InvalidState(
            "Enter the master password to revoke recovery".into(),
        ))
    }
    pub fn revoke_emergency_kit(
        &mut self,
        password: &str,
        keyfile: Option<&[u8]>,
    ) -> crate::Result<()> {
        if !self.verify_master_password_with_keyfile(password, keyfile)? {
            return Err(VaultError::InvalidPassword);
        }
        let previous = self.storage.clone();
        let old_audit = self.data.settings.emergency_kit_audit.clone();
        let result = (|| {
            let session = self.storage.as_mut().ok_or_else(|| {
                VaultError::InvalidState(
                    "Legacy password shares cannot be revoked; migrate to recovery v2".into(),
                )
            })?;
            if session.header.usb_marker.is_some() {
                return Err(VaultError::InvalidState("Replace the recovery kit while USB binding is enabled; do not remove the last recovery route".into()));
            }
            session.revoke_recovery(self.keys.as_ref().ok_or(VaultError::VaultLocked)?)?;
            if let Some(a) = self.data.settings.emergency_kit_audit.as_mut() {
                a.history.push(EmergencyKitAuditEntry {
                    timestamp: Utc::now(),
                    fingerprint: a.active_fingerprint.clone(),
                    action: "revoked-v2".into(),
                });
                a.active_fingerprint.clear();
            }
            self.save()
        })();
        if result.is_err() {
            self.storage = previous;
            self.data.settings.emergency_kit_audit = old_audit;
        }
        result
    }
    pub fn recover_with_shares(
        path: &std::path::Path,
        a: &str,
        b: &str,
        new_password: &str,
    ) -> crate::Result<Self> {
        if new_password.chars().count() < 12 {
            return Err(VaultError::InvalidState(
                "Choose a new password of at least 12 characters".into(),
            ));
        }
        let bytes = super::storage::read_bounded(path)?;
        if !super::storage::is_protected(&bytes) {
            if a.trim().starts_with("YNTRA2:") {
                return Err(share_error());
            }
            let old = Zeroizing::new(crate::crypto::reconstruct_password(a, b)?);
            let mut manager = Self::open(path, &old)?;
            manager.change_master_password(&old, new_password)?;
            return Ok(manager);
        }
        let (replica, kit, secret) = reconstruct(a, b)?;
        let (header, _) = super::storage::parse(&bytes)?;
        if header.replica_id != replica {
            return Err(share_error());
        }
        let (session, keys) = super::storage::StorageSession::recover(header, kit, &secret)?;
        let inner = session.decrypt(&bytes)?;
        let mut manager = Self::from_decrypted_vault_file(
            path,
            super::format::VaultFile::from_bytes(&inner)?,
            keys,
        )?;
        let mut recovered_storage = super::storage::StorageSession::new(
            new_password,
            None,
            None,
            manager.keys.as_ref().ok_or(VaultError::VaultLocked)?,
        )?;
        recovered_storage.disk_header = session.disk_header;
        manager.storage = Some(recovered_storage);
        manager.biometric = None;
        manager.hardware2fa = None;
        if let Some(a) = manager.data.settings.emergency_kit_audit.as_mut() {
            a.active_fingerprint.clear();
            a.history.push(EmergencyKitAuditEntry {
                timestamp: Utc::now(),
                fingerprint: kit.to_string(),
                action: "recovered-v2".into(),
            });
        }
        manager.save()?;
        Ok(manager)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_session_cannot_restore_a_replaced_recovery_kit() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("vault.vdb");
        let mut current = VaultManager::create("Test", "test password 2026", &path).unwrap();
        current
            .generate_emergency_kit("test password 2026")
            .unwrap();
        let mut stale = VaultManager::open(&path, "test password 2026").unwrap();
        current
            .generate_emergency_kit("test password 2026")
            .unwrap();
        let newest = std::fs::read(&path).unwrap();
        assert!(stale.save().is_err());
        assert!(stale.reload().is_err());
        assert_eq!(std::fs::read(&path).unwrap(), newest);
    }

    #[test]
    fn failed_recovery_rotation_restores_session_and_existing_kit() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("vault.vdb");
        let mut manager = VaultManager::create("Test", "test password 2026", &path).unwrap();
        let kit = manager
            .generate_emergency_kit("test password 2026")
            .unwrap();
        let before = std::fs::read(&path).unwrap();
        manager.path = directory.path().to_owned();
        assert!(
            manager
                .generate_emergency_kit("test password 2026")
                .is_err()
        );
        manager.path = path.clone();
        assert_eq!(std::fs::read(&path).unwrap(), before);
        manager.save().unwrap();
        assert!(
            VaultManager::recover_with_shares(
                &path,
                &kit.shares[0].share_data,
                &kit.shares[1].share_data,
                "new password 2026"
            )
            .is_ok()
        );
    }

    #[test]
    fn recovery_rotation_password_change_and_consumption() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.vdb");
        let mut manager = VaultManager::create("Test", "old password 123", &path).unwrap();
        assert!(manager.generate_emergency_kit("wrong").is_err());
        let first = manager.generate_emergency_kit("old password 123").unwrap();
        let second = manager.generate_emergency_kit("old password 123").unwrap();
        let export = dir.path().join("share.txt");
        export_recovery_share(&export, &second.shares[1].share_data).unwrap();
        let exported = std::fs::read_to_string(&export).unwrap();
        assert!(exported.contains(&second.shares[1].share_data));
        assert!(!exported.contains(&second.shares[0].share_data));
        assert!(!exported.contains(&second.shares[2].share_data));
        assert!(export_recovery_share(&export, &second.shares[2].share_data).is_err());
        assert!(
            VaultManager::recover_with_shares(
                &path,
                &first.shares[0].share_data,
                &first.shares[1].share_data,
                "new password 123"
            )
            .is_err()
        );
        assert!(reconstruct(&first.shares[0].share_data, &second.shares[1].share_data).is_err());
        assert!(reconstruct(&second.shares[0].share_data, &second.shares[0].share_data).is_err());
        manager
            .change_master_password("old password 123", "changed password 123")
            .unwrap();
        assert!(VaultManager::open(&path, "old password 123").is_err());
        assert!(VaultManager::open(&path, "changed password 123").is_ok());
        let recovered = VaultManager::recover_with_shares(
            &path,
            &second.shares[0].share_data,
            &second.shares[2].share_data,
            "new password 123",
        )
        .unwrap();
        assert!(!recovered.protection_info().recovery_enabled);
        assert!(VaultManager::open(&path, "new password 123").is_ok());
        assert!(
            VaultManager::recover_with_shares(
                &path,
                &second.shares[0].share_data,
                &second.shares[2].share_data,
                "new password 123"
            )
            .is_err()
        );
    }
}
