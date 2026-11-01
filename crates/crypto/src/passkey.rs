//! ECDSA P-256 (ES256) Passkey Authenticator Engine.

use p256::ecdsa::{SigningKey, VerifyingKey, Signature};
use p256::ecdsa::signature::{Signer, Verifier};
use p256::elliptic_curve::Generate;
use zeroize::Zeroizing;

pub struct PasskeyPair {
    pub private_key: Zeroizing<Vec<u8>>,
    pub public_key: Vec<u8>,
}

/// Generate a new ECDSA P-256 (ES256) keypair for a Passkey.
/// Returns private key (raw 32-byte scalar) and public key (uncompressed SEC1 format).
pub fn generate_passkey_pair() -> crate::Result<PasskeyPair> {
    let signing_key = SigningKey::try_generate()
        .map_err(|error| crate::error::VaultError::EncryptionError(format!("Passkey entropy unavailable: {error}")))?;
    let verifying_key = VerifyingKey::from(&signing_key);

    Ok(PasskeyPair {
        private_key: Zeroizing::new(signing_key.to_bytes().to_vec()),
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
    fn p256_preserves_existing_key_signature_and_ecdh_formats() {
        // Fixed scalars 1 and 2, independently checked with OpenSSL via cryptography.
        let decode = |value: &str| data_encoding::HEXLOWER.decode(value.as_bytes()).unwrap();
        let public = decode("046b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c2964fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5");
        let mut scalar = [0u8; 32];
        scalar[31] = 1;
        let signing_key = SigningKey::from_slice(&scalar).unwrap();
        assert_eq!(signing_key.verifying_key().to_sec1_bytes().as_ref(), public);
        let signature = decode("3045022100a53f6004980547220536fa38969f8e3b2c7196cf0cc987a6e796339bfaee3cc702205ce19fb3003cfa5914755a5b0e267f2b2c9aa01742dd5e8c01ee306d107794d6");
        assert!(verify_assertion(&public, b"legacy-authenticator", b"legacy-client-hash", &signature).unwrap());
        assert!(!verify_assertion(&public, b"changed-authenticator", b"legacy-client-hash", &signature).unwrap());
        let peer = p256::PublicKey::from_sec1_bytes(&decode("047cf27b188d034f7e8a52380304b51ac3c08969e277f21b35a60b48fc4766997807775510db8ed040293d9ac69f7430dbba7dade63ce982299e04b79d227873d1")).unwrap();
        let secret = p256::SecretKey::from_slice(&scalar).unwrap();
        let shared = p256::ecdh::diffie_hellman(secret.to_nonzero_scalar(), peer.as_affine());
        assert_eq!(shared.raw_secret_bytes().as_slice(), decode("7cf27b188d034f7e8a52380304b51ac3c08969e277f21b35a60b48fc47669978"));
    }

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
