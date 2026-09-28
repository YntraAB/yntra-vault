//! Publisher authentication independent of the release download channel.
use crate::{Result, VaultError};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;

const PUBLIC_KEY: &str = include_str!("../../../../update-signing-public-key.hex");
const MAX_LIFETIME: u64 = 90 * 86400;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope { schema: u32, key_id: String, issued_at: u64, expires_at: u64, signature: String }

pub(super) fn verify(bytes: &[u8], envelope: &[u8]) -> Result<()> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| invalid())?.as_secs();
    let public = data_encoding::HEXLOWER.decode(PUBLIC_KEY.trim().as_bytes()).map_err(|_| invalid())?;
    let public: [u8; 32] = public.try_into().map_err(|_| invalid())?;
    verify_with_key(bytes, envelope, &public, now)
}

fn invalid() -> VaultError { VaultError::UpdateError("Missing, invalid or expired publisher signature; update rejected".into()) }

fn verify_with_key(bytes: &[u8], encoded: &[u8], public: &[u8; 32], now: u64) -> Result<()> {
    let envelope: Envelope = serde_json::from_slice(encoded).map_err(|_| invalid())?;
    if envelope.schema != 1 || envelope.key_id != "yntra-update-1"
        || envelope.issued_at > now.saturating_add(300) || envelope.expires_at <= now
        || envelope.expires_at <= envelope.issued_at
        || envelope.expires_at - envelope.issued_at > MAX_LIFETIME { return Err(invalid()); }
    let context = format!("yntra-update-v1\n{}\n{}\n{}\n", envelope.key_id, envelope.issued_at, envelope.expires_at);
    let mut message = context.into_bytes();
    message.extend_from_slice(bytes);
    let signature = data_encoding::BASE64.decode(envelope.signature.as_bytes()).map_err(|_| invalid())?;
    let signature = Signature::from_slice(&signature).map_err(|_| invalid())?;
    VerifyingKey::from_bytes(public).map_err(|_| invalid())?.verify_strict(&message, &signature).map_err(|_| invalid())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    #[test]
    fn node_release_signer_interoperates_with_native_verifier() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../../tests/fixtures/update-signature.json")).unwrap();
        let key: [u8;32] = data_encoding::HEXLOWER.decode(fixture["publicKey"].as_str().unwrap().as_bytes()).unwrap().try_into().unwrap();
        let envelope = serde_json::to_vec(&fixture["envelope"]).unwrap();
        let bytes = fixture["manifest"].as_str().unwrap().as_bytes();
        assert!(verify_with_key(bytes, &envelope, &key, 1001).is_ok());
        assert!(verify_with_key(bytes, &envelope, &key, 1000 + MAX_LIFETIME).is_err());
    }
    #[test]
    fn authentication_binds_bytes_identity_and_expiry() {
        let key = SigningKey::from_bytes(&[42; 32]); // Test-only identity, never the production pin.
        let bytes = br#"{"version":"0.2.5","sha256":"abc"}"#;
        let mut message = b"yntra-update-v1\nyntra-update-1\n1000\n2000\n".to_vec();
        message.extend_from_slice(bytes);
        let signature = data_encoding::BASE64.encode(&key.sign(&message).to_bytes());
        let envelope = serde_json::json!({"schema":1,"key_id":"yntra-update-1","issued_at":1000,"expires_at":2000,"signature":signature});
        let encoded = serde_json::to_vec(&envelope).unwrap();
        let public = key.verifying_key().to_bytes();
        assert!(verify_with_key(bytes, &encoded, &public, 1500).is_ok());
        assert!(verify_with_key(b"tampered", &encoded, &public, 1500).is_err());
        assert!(verify_with_key(bytes, &encoded, &SigningKey::from_bytes(&[43;32]).verifying_key().to_bytes(), 1500).is_err());
        assert!(verify_with_key(bytes, &encoded, &public, 2000).is_err());
        assert!(verify_with_key(bytes, &encoded, &public, 0).is_err());
        let mut forged = envelope;
        forged["expires_at"] = 3000.into();
        assert!(verify_with_key(bytes, &serde_json::to_vec(&forged).unwrap(), &public, 2500).is_err());
        assert!(verify_with_key(bytes, b"{}", &public, 1500).is_err());
        assert!(verify(bytes, &encoded).is_err()); // Production pin differs.
    }
}
