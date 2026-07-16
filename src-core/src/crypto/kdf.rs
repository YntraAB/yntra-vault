//! Key Derivation Function pipeline
//!
//! Master Password → Argon2id (256MB RAM, 4 passes) → HKDF-SHA512 → 3 subkeys

use argon2::{Argon2, Algorithm, Version, Params};
use hkdf::Hkdf;
use sha2::Sha512;
use zeroize::{Zeroize, ZeroizeOnDrop};
use rand::Rng;
use crate::error::VaultError;

/// Argon2id parameters — deliberately aggressive for maximum security
const ARGON2_MEMORY_KB: u32 = 262_144; // 256 MB
const ARGON2_ITERATIONS: u32 = 4;
const ARGON2_PARALLELISM: u32 = 4;
const ARGON2_OUTPUT_LEN: usize = 64;

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct MasterKey {
    bytes: Vec<u8>,
}

impl MasterKey {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

pub struct SubKeys {
    pub vault_key: VaultKey,
    pub entry_key: EntryKey,
    pub hmac_key: HmacKey,
}

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct VaultKey {
    pub bytes: [u8; 32],
}

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct EntryKey {
    pub bytes: [u8; 32],
}

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct HmacKey {
    pub bytes: [u8; 64],
}

/// Generate a cryptographically secure random salt (32 bytes).
pub fn generate_salt() -> [u8; 32] {
    let mut salt = [0u8; 32];
    rand::rng().fill(&mut salt);
    salt
}

/// Derive the master key from password + salt using Argon2id.
pub fn derive_master_key(password: &[u8], salt: &[u8; 32]) -> crate::Result<MasterKey> {
    let params = Params::new(
        ARGON2_MEMORY_KB,
        ARGON2_ITERATIONS,
        ARGON2_PARALLELISM,
        Some(ARGON2_OUTPUT_LEN),
    ).map_err(|e| VaultError::KdfError(format!("Invalid Argon2 params: {}", e)))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut output = vec![0u8; ARGON2_OUTPUT_LEN];
    argon2
        .hash_password_into(password, salt, &mut output)
        .map_err(|e| VaultError::KdfError(format!("Argon2id failed: {}", e)))?;

    Ok(MasterKey { bytes: output })
}

/// Derive 3 purpose-separated subkeys from the master key using HKDF-SHA512.
pub fn derive_subkeys(master_key: &MasterKey) -> crate::Result<SubKeys> {
    let hk = Hkdf::<Sha512>::new(None, master_key.as_bytes());

    let mut vault_key_bytes = [0u8; 32];
    hk.expand(b"yntra-vault-encryption-key-v1", &mut vault_key_bytes)
        .map_err(|e| VaultError::KdfError(format!("HKDF expand (vault): {}", e)))?;

    let mut entry_key_bytes = [0u8; 32];
    hk.expand(b"yntra-vault-entry-encryption-key-v1", &mut entry_key_bytes)
        .map_err(|e| VaultError::KdfError(format!("HKDF expand (entry): {}", e)))?;

    let mut hmac_key_bytes = [0u8; 64];
    hk.expand(b"yntra-vault-hmac-integrity-key-v1", &mut hmac_key_bytes)
        .map_err(|e| VaultError::KdfError(format!("HKDF expand (hmac): {}", e)))?;

    Ok(SubKeys {
        vault_key: VaultKey { bytes: vault_key_bytes },
        entry_key: EntryKey { bytes: entry_key_bytes },
        hmac_key: HmacKey { bytes: hmac_key_bytes },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_derivation_deterministic() {
        let password = b"test-master-password";
        let salt = [42u8; 32];
        let key1 = derive_master_key(password, &salt).unwrap();
        let key2 = derive_master_key(password, &salt).unwrap();
        assert_eq!(key1.as_bytes(), key2.as_bytes());
        assert_eq!(key1.as_bytes().len(), ARGON2_OUTPUT_LEN);
    }

    #[test]
    fn test_different_passwords_different_keys() {
        let salt = [42u8; 32];
        let key1 = derive_master_key(b"password1", &salt).unwrap();
        let key2 = derive_master_key(b"password2", &salt).unwrap();
        assert_ne!(key1.as_bytes(), key2.as_bytes());
    }

    #[test]
    fn test_different_salts_different_keys() {
        let password = b"same-password";
        let key1 = derive_master_key(password, &[1u8; 32]).unwrap();
        let key2 = derive_master_key(password, &[2u8; 32]).unwrap();
        assert_ne!(key1.as_bytes(), key2.as_bytes());
    }

    #[test]
    fn test_subkey_derivation() {
        let password = b"test-password";
        let salt = [42u8; 32];
        let master_key = derive_master_key(password, &salt).unwrap();
        let subkeys = derive_subkeys(&master_key).unwrap();
        assert_ne!(&subkeys.vault_key.bytes[..], &subkeys.entry_key.bytes[..]);
        assert_ne!(&subkeys.vault_key.bytes[..], &subkeys.hmac_key.bytes[..32]);
    }

    #[test]
    fn test_subkey_deterministic() {
        let password = b"test-password";
        let salt = [42u8; 32];
        let mk1 = derive_master_key(password, &salt).unwrap();
        let sk1 = derive_subkeys(&mk1).unwrap();
        let mk2 = derive_master_key(password, &salt).unwrap();
        let sk2 = derive_subkeys(&mk2).unwrap();
        assert_eq!(sk1.vault_key.bytes, sk2.vault_key.bytes);
        assert_eq!(sk1.entry_key.bytes, sk2.entry_key.bytes);
        assert_eq!(sk1.hmac_key.bytes, sk2.hmac_key.bytes);
    }
}

