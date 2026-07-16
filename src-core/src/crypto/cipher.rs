//! Multi-layer encryption engine
//!
//! Layer 1: XChaCha20-Poly1305 — Vault-level authenticated encryption
//! Layer 2: AES-256-GCM — Per-entry encryption
//! Layer 3: HMAC-SHA512 — Integrity verification

use chacha20poly1305::{XChaCha20Poly1305, aead::{Aead, KeyInit}};
use aes_gcm::{Aes256Gcm, Nonce as AesNonce};
use hmac::{Hmac, Mac};
use sha2::Sha512;
use rand::Rng;

use super::kdf::{VaultKey, EntryKey, HmacKey};
use crate::error::VaultError;

type HmacSha512 = Hmac<Sha512>;

/// Encrypted data with its nonce, ready for storage
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct EncryptedBlob {
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

// ─── Layer 1: XChaCha20-Poly1305 (Vault-level) ─────────────────────────

pub fn encrypt_vault(plaintext: &[u8], key: &VaultKey) -> crate::Result<EncryptedBlob> {
    let cipher = XChaCha20Poly1305::new_from_slice(&key.bytes)
        .map_err(|e| VaultError::EncryptionError(format!("XChaCha20 key init: {}", e)))?;

    let mut nonce_bytes = [0u8; 24];
    rand::rng().fill(&mut nonce_bytes);
    let nonce = chacha20poly1305::XNonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| VaultError::EncryptionError(format!("XChaCha20 encrypt: {}", e)))?;

    Ok(EncryptedBlob {
        nonce: nonce_bytes.to_vec(),
        ciphertext,
    })
}

pub fn decrypt_vault(blob: &EncryptedBlob, key: &VaultKey) -> crate::Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new_from_slice(&key.bytes)
        .map_err(|e| VaultError::DecryptionError(format!("XChaCha20 key init: {}", e)))?;

    if blob.nonce.len() != 24 {
        return Err(VaultError::DecryptionError(
            "Invalid XChaCha20 nonce length (expected 24 bytes)".into(),
        ));
    }

    let nonce = chacha20poly1305::XNonce::from_slice(&blob.nonce);
    cipher
        .decrypt(nonce, blob.ciphertext.as_ref())
        .map_err(|_| VaultError::InvalidPassword)
}

// ─── Layer 2: AES-256-GCM (Per-entry) ──────────────────────────────────

pub fn encrypt_entry(plaintext: &[u8], key: &EntryKey) -> crate::Result<EncryptedBlob> {
    let cipher = Aes256Gcm::new_from_slice(&key.bytes)
        .map_err(|e| VaultError::EncryptionError(format!("AES-GCM key init: {}", e)))?;

    let mut nonce_bytes = [0u8; 12];
    rand::rng().fill(&mut nonce_bytes);
    let nonce = AesNonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| VaultError::EncryptionError(format!("AES-GCM encrypt: {}", e)))?;

    Ok(EncryptedBlob {
        nonce: nonce_bytes.to_vec(),
        ciphertext,
    })
}

pub fn decrypt_entry(blob: &EncryptedBlob, key: &EntryKey) -> crate::Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(&key.bytes)
        .map_err(|e| VaultError::DecryptionError(format!("AES-GCM key init: {}", e)))?;

    if blob.nonce.len() != 12 {
        return Err(VaultError::DecryptionError(
            "Invalid AES-GCM nonce length (expected 12 bytes)".into(),
        ));
    }

    let nonce = AesNonce::from_slice(&blob.nonce);
    cipher
        .decrypt(nonce, blob.ciphertext.as_ref())
        .map_err(|_| VaultError::DecryptionError("AES-GCM auth tag mismatch".into()))
}

// ─── Layer 3: HMAC-SHA512 (Integrity) ───────────────────────────────────

pub fn compute_hmac(data: &[u8], key: &HmacKey) -> Vec<u8> {
    let mut mac = <HmacSha512 as Mac>::new_from_slice(&key.bytes)
        .expect("HMAC-SHA512 accepts any key size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

pub fn verify_hmac(data: &[u8], expected_mac: &[u8], key: &HmacKey) -> crate::Result<()> {
    let mut mac = <HmacSha512 as Mac>::new_from_slice(&key.bytes)
        .expect("HMAC-SHA512 accepts any key size");
    mac.update(data);
    mac.verify_slice(expected_mac)
        .map_err(|_| VaultError::IntegrityError)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::kdf::{VaultKey, EntryKey, HmacKey};

    fn test_vault_key() -> VaultKey {
        let mut bytes = [0u8; 32];
        rand::rng().fill(&mut bytes);
        VaultKey { bytes }
    }

    fn test_entry_key() -> EntryKey {
        let mut bytes = [0u8; 32];
        rand::rng().fill(&mut bytes);
        EntryKey { bytes }
    }

    fn test_hmac_key() -> HmacKey {
        let mut bytes = [0u8; 64];
        rand::rng().fill(&mut bytes);
        HmacKey { bytes }
    }

    #[test]
    fn test_vault_encrypt_decrypt_roundtrip() {
        let key = test_vault_key();
        let plaintext = b"Hello, Yntra Vault vault encryption!";
        let encrypted = encrypt_vault(plaintext, &key).unwrap();
        assert_ne!(&encrypted.ciphertext, plaintext);
        assert_eq!(encrypted.nonce.len(), 24);
        let decrypted = decrypt_vault(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_vault_wrong_key_fails() {
        let key1 = test_vault_key();
        let key2 = test_vault_key();
        let encrypted = encrypt_vault(b"secret data", &key1).unwrap();
        assert!(decrypt_vault(&encrypted, &key2).is_err());
    }

    #[test]
    fn test_vault_tampered_data_fails() {
        let key = test_vault_key();
        let mut encrypted = encrypt_vault(b"secret data", &key).unwrap();
        if let Some(byte) = encrypted.ciphertext.last_mut() {
            *byte ^= 0xFF;
        }
        assert!(decrypt_vault(&encrypted, &key).is_err());
    }

    #[test]
    fn test_entry_encrypt_decrypt_roundtrip() {
        let key = test_entry_key();
        let plaintext = b"individual entry password data";
        let encrypted = encrypt_entry(plaintext, &key).unwrap();
        assert_eq!(encrypted.nonce.len(), 12);
        let decrypted = decrypt_entry(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_unique_nonces() {
        let key = test_vault_key();
        let e1 = encrypt_vault(b"same data", &key).unwrap();
        let e2 = encrypt_vault(b"same data", &key).unwrap();
        assert_ne!(e1.nonce, e2.nonce);
        assert_ne!(e1.ciphertext, e2.ciphertext);
    }

    #[test]
    fn test_hmac_integrity() {
        let key = test_hmac_key();
        let data = b"vault file contents";
        let mac = compute_hmac(data, &key);
        assert_eq!(mac.len(), 64);
        assert!(verify_hmac(data, &mac, &key).is_ok());

        let mut tampered = data.to_vec();
        tampered[0] ^= 0xFF;
        assert!(verify_hmac(&tampered, &mac, &key).is_err());
    }

    #[test]
    fn test_large_data() {
        let key = test_vault_key();
        let plaintext = vec![0xABu8; 1_000_000];
        let encrypted = encrypt_vault(&plaintext, &key).unwrap();
        let decrypted = decrypt_vault(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}
