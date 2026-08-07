//! Multi-layer encryption engine
//!
//! Layer 1: XChaCha20-Poly1305 — Vault-level authenticated encryption
//! Layer 2: XChaCha20-Poly1305 — Per-entry encryption (with legacy AES-256-GCM fallback)
//! Layer 3: HMAC-SHA512 — Integrity verification

use chacha20poly1305::{XChaCha20Poly1305, aead::{Aead, KeyInit, Payload}};
use aes_gcm::{Aes256Gcm, Nonce as AesNonce, aead::Payload as AesPayload};
use hmac::{Hmac, Mac};
use sha2::Sha512;
use rand::Rng;
use zeroize::Zeroizing;

use super::kdf::{VaultKey, EntryKey, HmacKey};
use crate::error::VaultError;

type HmacSha512 = Hmac<Sha512>;

/// Encrypted data with its nonce, ready for storage
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct EncryptedBlob {
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

/// Generate a 24-byte (192-bit) CSPRNG nonce for XChaCha20-Poly1305.
///
/// Follows SOTA RFC 8439 / libsodium standard. Extended 192-bit nonces have a collision
/// bound of 2^-96, eliminating birthday collision risks under random generation.
pub fn generate_nonce_24() -> [u8; 24] {
    let mut nonce = [0u8; 24];
    rand::rng().fill(&mut nonce);
    nonce
}

// ─── Layer 1: XChaCha20-Poly1305 (Vault-level with AAD Header Binding) ─────────

pub fn encrypt_vault_with_aad(
    plaintext: &[u8],
    key: &VaultKey,
    aad: &[u8],
) -> crate::Result<EncryptedBlob> {
    let cipher = XChaCha20Poly1305::new_from_slice(&key.bytes)
        .map_err(|e| VaultError::EncryptionError(format!("XChaCha20 key init: {}", e)))?;

    let nonce_bytes = generate_nonce_24();
    let nonce = chacha20poly1305::XNonce::from_slice(&nonce_bytes);

    let payload = Payload {
        msg: plaintext,
        aad,
    };

    let ciphertext = cipher
        .encrypt(nonce, payload)
        .map_err(|e| VaultError::EncryptionError(format!("XChaCha20 encrypt: {}", e)))?;

    Ok(EncryptedBlob {
        nonce: nonce_bytes.to_vec(),
        ciphertext,
    })
}

pub fn encrypt_vault(plaintext: &[u8], key: &VaultKey) -> crate::Result<EncryptedBlob> {
    encrypt_vault_with_aad(plaintext, key, b"")
}

pub fn decrypt_vault_with_aad(
    blob: &EncryptedBlob,
    key: &VaultKey,
    aad: &[u8],
) -> crate::Result<Zeroizing<Vec<u8>>> {
    let cipher = XChaCha20Poly1305::new_from_slice(&key.bytes)
        .map_err(|e| VaultError::DecryptionError(format!("XChaCha20 key init: {}", e)))?;

    if blob.nonce.len() != 24 {
        return Err(VaultError::DecryptionError(
            "Invalid XChaCha20 nonce length (expected 24 bytes)".into(),
        ));
    }

    let nonce = chacha20poly1305::XNonce::from_slice(&blob.nonce);
    let payload = Payload {
        msg: blob.ciphertext.as_ref(),
        aad,
    };

    let plaintext = cipher
        .decrypt(nonce, payload)
        .map_err(|_| VaultError::InvalidPassword)?;

    Ok(Zeroizing::new(plaintext))
}

pub fn decrypt_vault(blob: &EncryptedBlob, key: &VaultKey) -> crate::Result<Zeroizing<Vec<u8>>> {
    decrypt_vault_with_aad(blob, key, b"")
}

// ─── Layer 2: XChaCha20-Poly1305 (Per-entry, 24-byte Hedged Nonce + AAD Binding) ──────

pub fn encrypt_entry_with_aad(
    plaintext: &[u8],
    key: &EntryKey,
    aad: &[u8],
) -> crate::Result<EncryptedBlob> {
    let cipher = XChaCha20Poly1305::new_from_slice(&key.bytes)
        .map_err(|e| VaultError::EncryptionError(format!("XChaCha20 key init: {}", e)))?;

    let nonce_bytes = generate_nonce_24();
    let nonce = chacha20poly1305::XNonce::from_slice(&nonce_bytes);

    let payload = Payload {
        msg: plaintext,
        aad,
    };

    let ciphertext = cipher
        .encrypt(nonce, payload)
        .map_err(|e| VaultError::EncryptionError(format!("XChaCha20 entry encrypt: {}", e)))?;

    Ok(EncryptedBlob {
        nonce: nonce_bytes.to_vec(),
        ciphertext,
    })
}

pub fn encrypt_entry(plaintext: &[u8], key: &EntryKey) -> crate::Result<EncryptedBlob> {
    encrypt_entry_with_aad(plaintext, key, b"")
}

pub fn decrypt_entry_with_aad(
    blob: &EncryptedBlob,
    key: &EntryKey,
    aad: &[u8],
) -> crate::Result<Zeroizing<Vec<u8>>> {
    match blob.nonce.len() {
        24 => {
            let cipher = XChaCha20Poly1305::new_from_slice(&key.bytes)
                .map_err(|e| VaultError::DecryptionError(format!("XChaCha20 key init: {}", e)))?;
            let nonce = chacha20poly1305::XNonce::from_slice(&blob.nonce);

            let payload = Payload {
                msg: blob.ciphertext.as_ref(),
                aad,
            };
            if let Ok(plaintext) = cipher.decrypt(nonce, payload) {
                return Ok(Zeroizing::new(plaintext));
            }

            if !aad.is_empty() {
                let empty_payload = Payload {
                    msg: blob.ciphertext.as_ref(),
                    aad: b"",
                };
                if let Ok(plaintext) = cipher.decrypt(nonce, empty_payload) {
                    return Ok(Zeroizing::new(plaintext));
                }
            }

            Err(VaultError::DecryptionError("XChaCha20 auth tag mismatch (data/AAD tampered)".into()))
        }
        12 => {
            let cipher = Aes256Gcm::new_from_slice(&key.bytes)
                .map_err(|e| VaultError::DecryptionError(format!("AES-GCM key init: {}", e)))?;
            let nonce = AesNonce::from_slice(&blob.nonce);

            let payload = AesPayload {
                msg: blob.ciphertext.as_ref(),
                aad,
            };
            if let Ok(plaintext) = cipher.decrypt(nonce, payload) {
                return Ok(Zeroizing::new(plaintext));
            }

            if !aad.is_empty() {
                let empty_payload = AesPayload {
                    msg: blob.ciphertext.as_ref(),
                    aad: b"",
                };
                if let Ok(plaintext) = cipher.decrypt(nonce, empty_payload) {
                    return Ok(Zeroizing::new(plaintext));
                }
            }

            Err(VaultError::DecryptionError("AES-GCM auth tag mismatch".into()))
        }
        _ => Err(VaultError::DecryptionError(
            format!("Invalid entry nonce length: expected 24 or 12 bytes, got {}", blob.nonce.len())
        )),
    }
}

pub fn decrypt_entry(blob: &EncryptedBlob, key: &EntryKey) -> crate::Result<Zeroizing<Vec<u8>>> {
    decrypt_entry_with_aad(blob, key, b"")
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
        assert_eq!(*decrypted, plaintext);
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
        assert_eq!(encrypted.nonce.len(), 24);
        let decrypted = decrypt_entry(&encrypted, &key).unwrap();
        assert_eq!(*decrypted, plaintext);
    }

    #[test]
    fn test_aad_binding_isolation_and_fallback() {
        let key = test_entry_key();
        let aad_entry_a = b"entry-uuid-A-password";
        let aad_entry_b = b"entry-uuid-B-password";
        let plaintext = b"sensitive-password-123";

        // Encrypt with Entry A's AAD
        let encrypted = encrypt_entry_with_aad(plaintext, &key, aad_entry_a).unwrap();

        // Decrypting with correct AAD succeeds
        let decrypted = decrypt_entry_with_aad(&encrypted, &key, aad_entry_a).unwrap();
        assert_eq!(*decrypted, plaintext);

        // Decrypting with wrong AAD (transplant attack) MUST fail
        let wrong_aad_result = decrypt_entry_with_aad(&encrypted, &key, aad_entry_b);
        assert!(wrong_aad_result.is_err());

        // Decrypting legacy blob without AAD with empty AAD succeeds
        let legacy_unbound = encrypt_entry(plaintext, &key).unwrap();
        let decrypted_fallback = decrypt_entry_with_aad(&legacy_unbound, &key, aad_entry_a).unwrap();
        assert_eq!(*decrypted_fallback, plaintext);
    }

    #[test]
    fn test_legacy_aes_gcm_entry_fallback() {
        let key = test_entry_key();
        let plaintext = b"legacy aes-256-gcm entry password";

        // Manually construct legacy 12-byte AES-256-GCM blob
        let cipher = Aes256Gcm::new_from_slice(&key.bytes).unwrap();
        let mut nonce_bytes = [0u8; 12];
        rand::rng().fill(&mut nonce_bytes);
        let nonce = AesNonce::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, plaintext.as_ref()).unwrap();

        let legacy_blob = EncryptedBlob {
            nonce: nonce_bytes.to_vec(),
            ciphertext,
        };
        assert_eq!(legacy_blob.nonce.len(), 12);

        // Verify decrypt_entry automatically falls back and decrypts 12-byte AES-GCM blob
        let decrypted = decrypt_entry(&legacy_blob, &key).unwrap();
        assert_eq!(*decrypted, plaintext);
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
    fn test_random_nonce_generation() {
        let n1 = generate_nonce_24();
        let n2 = generate_nonce_24();

        assert_eq!(n1.len(), 24);
        assert_ne!(n1, n2);
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
        assert_eq!(*decrypted, plaintext);
    }
}

