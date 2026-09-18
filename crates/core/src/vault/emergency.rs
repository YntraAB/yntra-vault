//! Cryptographic Emergency Kit generator for Yntra Vault.
//!
//! Generates a structured recovery sheet combining vault metadata and 2-of-3
//! Shamir Secret Sharing recovery shares for master password reconstruction.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::VaultError;
use crate::vault::manager::VaultManager;
use crate::vault::types::{EmergencyKitAudit, EmergencyKitAuditEntry};

/// A single Shamir recovery share with distribution label.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EmergencyShare {
    pub share_index: u8,
    pub label: String,
    pub share_data: String,
}

/// The complete Emergency Kit payload.
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

impl VaultManager {
    /// Generate a cryptographic Emergency Kit for the unlocked vault.
    /// Cryptographically validates the master password, splits it into 3 Shamir shares (2-of-3 threshold),
    /// records an audit log entry inside the encrypted vault settings, and saves the vault.
    pub fn generate_emergency_kit(&mut self, master_password: &str) -> crate::Result<EmergencyKit> {
        self.generate_emergency_kit_with_keyfile(master_password, None)
    }

    /// Generate a cryptographic Emergency Kit with optional key file bytes.
    pub fn generate_emergency_kit_with_keyfile(
        &mut self,
        master_password: &str,
        key_file_bytes: Option<&[u8]>,
    ) -> crate::Result<EmergencyKit> {
        if !self.is_unlocked() {
            return Err(VaultError::VaultLocked);
        }

        if master_password.trim().is_empty() {
            return Err(VaultError::InvalidFormat("Master password cannot be empty".into()));
        }

        // Cryptographically verify candidate password against active session keys
        if !self.verify_master_password_with_keyfile(master_password, key_file_bytes)? {
            return Err(VaultError::InvalidPassword);
        }

        let raw_shares = yntra_crypto::split_password(master_password)?;
        if raw_shares.len() < 3 {
            return Err(VaultError::EncryptionError("Failed to generate 3 Shamir shares".into()));
        }

        // Verify mathematical integrity before returning
        let reconstructed = yntra_crypto::reconstruct_password(&raw_shares[0], &raw_shares[1])?;
        if reconstructed != master_password {
            return Err(VaultError::EncryptionError("Emergency share self-verification failed".into()));
        }

        let now = Utc::now();
        let active_keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;
        let hmac_tag = yntra_crypto::compute_hmac(
            b"yntra-vault-emergency-kit-audit-fingerprint-v1",
            &active_keys.hmac_key,
        );
        let v_hash = data_encoding::HEXLOWER.encode(&hmac_tag[..8]);

        let labels = ["Share 1", "Share 2", "Share 3"];

        let shares: Vec<EmergencyShare> = raw_shares
            .into_iter()
            .enumerate()
            .map(|(i, share_data)| EmergencyShare {
                share_index: (i + 1) as u8,
                label: labels.get(i).unwrap_or(&"Share").to_string(),
                share_data,
            })
            .collect();

        let doc = format!(
r#"# YNTRA VAULT — EMERGENCY RECOVERY SHEET

## 1. Vault Identification
- **Vault Name**: {}
- **Vault ID**: `{}`
- **Generated**: {}
- **Format Version**: v{}
- **Total Entries**: {}
- **Integrity Checksum**: `{}`

## 2. Emergency Recovery Instructions
This document contains 3 recovery shares created using a 2-of-3 threshold Shamir's Secret Sharing scheme.
- Any **TWO (2)** shares can fully reconstruct your Master Password.
- A single share reveals **ZERO** information about your Master Password.
- Store each share in separate, secure physical locations.

## 3. Cryptographic Recovery Shares

### Share 1
```text
{}
```

### Share 2
```text
{}
```

### Share 3
```text
{}
```

## 4. How to Restore Access
1. Launch Yntra Vault and select **Emergency Recovery** on the login screen or run in terminal:
   `yntra recover --share-a <SHARE> --share-b <SHARE>`
2. Enter any two of the shares above.
3. Once restored, immediately unlock and create a new master password.
"#,
            self.data.metadata.name,
            self.data.metadata.id,
            now.format("%Y-%m-%d %H:%M:%S UTC"),
            self.data.metadata.version,
            self.data.entries.len(),
            v_hash,
            shares[0].share_data,
            shares[1].share_data,
            shares[2].share_data,
        );

        // Record audit trail inside encrypted vault settings
        let audit = self.data.settings.emergency_kit_audit.get_or_insert_with(EmergencyKitAudit::default);
        audit.active_fingerprint = v_hash.clone();
        audit.last_generated_at = now;
        audit.generation_count += 1;
        audit.history.push(EmergencyKitAuditEntry {
            timestamp: now,
            fingerprint: v_hash.clone(),
            action: "generated".into(),
        });
        if audit.history.len() > 50 {
            audit.history.remove(0);
        }
        let _ = self.save();

        Ok(EmergencyKit {
            vault_id: self.data.metadata.id,
            vault_name: self.data.metadata.name.clone(),
            created_at: self.data.metadata.created_at,
            generated_at: now,
            format_version: self.data.metadata.version,
            total_entries: self.data.entries.len(),
            shares,
            verification_hash: v_hash,
            document_markdown: doc,
        })
    }

    /// Retrieve the current emergency kit audit trail from the unlocked vault settings.
    pub fn get_emergency_kit_audit(&self) -> Option<EmergencyKitAudit> {
        self.data.settings.emergency_kit_audit.clone()
    }

    /// Reset/archive active emergency kit status in vault settings.
    pub fn reset_emergency_kit_audit(&mut self) -> crate::Result<()> {
        if !self.is_unlocked() {
            return Err(VaultError::VaultLocked);
        }
        if let Some(ref mut audit) = self.data.settings.emergency_kit_audit
            && !audit.active_fingerprint.is_empty() {
                audit.history.push(EmergencyKitAuditEntry {
                    timestamp: Utc::now(),
                    fingerprint: audit.active_fingerprint.clone(),
                    action: "reset".into(),
                });
                audit.active_fingerprint.clear();
            }
        self.save()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_generate_emergency_kit_lifecycle() {
        let file = NamedTempFile::new().unwrap();
        let password = "emergency-test-password-1234";
        let mut manager = VaultManager::create("Emergency Vault", password, file.path()).unwrap();

        // 1. Password verification check
        assert!(manager.verify_master_password(password).unwrap());
        assert!(!manager.verify_master_password("wrong-password-123").unwrap());
        assert!(!manager.verify_master_password("").unwrap());

        // 2. Reject generation when password is incorrect
        let err = manager.generate_emergency_kit("wrong-password-123");
        assert!(matches!(err, Err(VaultError::InvalidPassword)));
        // Ensure audit log is not created on failed verification
        assert!(manager.get_emergency_kit_audit().is_none());

        // 3. Valid generation succeeds
        let kit = manager.generate_emergency_kit(password).unwrap();
        assert_eq!(kit.shares.len(), 3);
        assert_eq!(kit.vault_name, "Emergency Vault");
        assert!(kit.document_markdown.contains("YNTRA VAULT — EMERGENCY RECOVERY SHEET"));
        assert!(kit.document_markdown.contains(&kit.shares[0].share_data));

        let audit = manager.get_emergency_kit_audit().unwrap();
        assert_eq!(audit.generation_count, 1);
        assert_eq!(audit.active_fingerprint, kit.verification_hash);
        assert_eq!(audit.history.len(), 1);

        manager.reset_emergency_kit_audit().unwrap();
        let audit2 = manager.get_emergency_kit_audit().unwrap();
        assert!(audit2.active_fingerprint.is_empty());
        assert_eq!(audit2.history.len(), 2);
        assert_eq!(audit2.history[1].action, "reset");

        // Reconstruct from share 1 and share 3
        let rec = yntra_crypto::reconstruct_password(&kit.shares[0].share_data, &kit.shares[2].share_data).unwrap();
        assert_eq!(rec, password);

        // Reconstruct from share 2 and share 3
        let rec2 = yntra_crypto::reconstruct_password(&kit.shares[1].share_data, &kit.shares[2].share_data).unwrap();
        assert_eq!(rec2, password);

        // 4. Locked vault returns VaultLocked error
        manager.lock();
        assert!(matches!(manager.verify_master_password(password), Err(VaultError::VaultLocked)));
        assert!(matches!(manager.generate_emergency_kit(password), Err(VaultError::VaultLocked)));
    }
}
