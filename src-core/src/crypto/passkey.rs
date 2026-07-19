//! ECDSA P-256 (ES256) Passkey Authenticator Engine.

use p256::ecdsa::{SigningKey, VerifyingKey, Signature};
use p256::ecdsa::signature::{Signer, Verifier};
use p256::elliptic_curve::rand_core::OsRng;

pub struct PasskeyPair {
    pub private_key: Vec<u8>,
    pub public_key: Vec<u8>,
}

/// Generate a new ECDSA P-256 (ES256) keypair for a Passkey.
/// Returns private key (raw 32-byte scalar) and public key (uncompressed SEC1 format).
pub fn generate_passkey_pair() -> crate::Result<PasskeyPair> {
    let signing_key = SigningKey::random(&mut OsRng);
    let verifying_key = VerifyingKey::from(&signing_key);

    Ok(PasskeyPair {
        private_key: signing_key.to_bytes().to_vec(),
        public_key: verifying_key.to_sec1_bytes().to_vec(),
    })
}

/// Sign a WebAuthn assertion challenge.
/// Signs the concatenation of authenticator_data and client_data_hash.
pub fn sign_assertion(
    private_key_bytes: &[u8],
    authenticator_data: &[u8],
    client_data_hash: &[u8],
) -> crate::Result<Vec<u8>> {
    let signing_key = SigningKey::from_slice(private_key_bytes)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("Invalid passkey private key: {}", e)))?;

    // WebAuthn signs authenticatorData || clientDataHash
    let mut data_to_sign = Vec::with_capacity(authenticator_data.len() + client_data_hash.len());
    data_to_sign.extend_from_slice(authenticator_data);
    data_to_sign.extend_from_slice(client_data_hash);

    let signature: Signature = signing_key.sign(&data_to_sign);
    Ok(signature.to_der().to_bytes().to_vec())
}

/// Verify a WebAuthn assertion signature using the public key.
pub fn verify_assertion(
    public_key_bytes: &[u8],
    authenticator_data: &[u8],
    client_data_hash: &[u8],
    signature_der: &[u8],
) -> crate::Result<bool> {
    let verifying_key = VerifyingKey::from_sec1_bytes(public_key_bytes)
        .map_err(|e| crate::error::VaultError::DecryptionError(format!("Invalid passkey public key: {}", e)))?;

    let mut signed_data = Vec::with_capacity(authenticator_data.len() + client_data_hash.len());
    signed_data.extend_from_slice(authenticator_data);
    signed_data.extend_from_slice(client_data_hash);

    let signature = Signature::from_der(signature_der)
        .map_err(|e| crate::error::VaultError::DecryptionError(format!("Invalid DER signature: {}", e)))?;

    Ok(verifying_key.verify(&signed_data, &signature).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_passkey_generation_and_assertion_roundtrip() {
        let pair = generate_passkey_pair().unwrap();
        assert_eq!(pair.private_key.len(), 32);
        // Uncompressed SEC1 P-256 public key is 65 bytes
        assert_eq!(pair.public_key.len(), 65);

        let auth_data = b"authenticator-data-context";
        let client_hash = b"client-data-hash-32bytes-exactly";

        // Sign
        let signature = sign_assertion(&pair.private_key, auth_data, client_hash).unwrap();
        assert!(!signature.is_empty());

        // Verify
        let is_valid = verify_assertion(&pair.public_key, auth_data, client_hash, &signature).unwrap();
        assert!(is_valid);

        // Verify with tampered data
        let is_valid_tampered = verify_assertion(&pair.public_key, auth_data, b"wrong-hash", &signature).unwrap();
        assert!(!is_valid_tampered);
    }
}
