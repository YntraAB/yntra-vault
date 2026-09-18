//! OpenSSH wire format framing, key parsing, and challenge signing engine.
//!
//! Complies strictly with:
//! - RFC 4251 (SSH Protocol Architecture - Data Type Representations)
//! - RFC 5656 (Elliptic Curve Algorithm Integration - ecdsa-sha2-nistp256)
//! - RFC 8709 (Ed25519 in SSH)

use data_encoding::BASE64;
use ed25519_dalek::Signer as DalekSigner;
use p256::ecdsa::SigningKey as P256SigningKey;
use zeroize::Zeroizing;

use crate::error::VaultError;

/// Encodes raw bytes as an OpenSSH length-prefixed string (RFC 4251 Section 5).
/// Format: `[uint32_BE length] || [bytes]`
pub fn encode_ssh_string(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + bytes.len());
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(bytes);
    out
}

/// Encodes an unsigned integer / scalar as an OpenSSH `mpint` (RFC 4251 Section 5).
/// Format: big-endian two's complement integer. If the most significant bit is set,
/// a leading `0x00` byte MUST be prepended to represent a positive number.
pub fn encode_ssh_mpint(scalar: &[u8]) -> Vec<u8> {
    // Skip leading zeros
    let mut start = 0;
    while start < scalar.len() && scalar[start] == 0 {
        start += 1;
    }

    if start == scalar.len() {
        // Zero is represented as 4 zero length bytes
        return vec![0, 0, 0, 0];
    }

    let significant = &scalar[start..];
    let needs_padding = (significant[0] & 0x80) != 0;

    let len = if needs_padding {
        significant.len() + 1
    } else {
        significant.len()
    };

    let mut out = Vec::with_capacity(4 + len);
    out.extend_from_slice(&(len as u32).to_be_bytes());
    if needs_padding {
        out.push(0x00);
    }
    out.extend_from_slice(significant);
    out
}

/// Constructs an OpenSSH binary public key wire blob for `ssh-ed25519` (RFC 8709).
/// Total length: 51 bytes.
pub fn encode_ed25519_pubkey(pubkey_bytes: &[u8; 32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(51);
    out.extend_from_slice(&encode_ssh_string(b"ssh-ed25519"));
    out.extend_from_slice(&encode_ssh_string(pubkey_bytes));
    out
}

/// Constructs an OpenSSH binary public key wire blob for `ecdsa-sha2-nistp256` (RFC 5656).
/// `sec1_bytes` must be the 65-byte uncompressed EC point (`0x04 || X || Y`).
/// Total length: 104 bytes.
pub fn encode_p256_pubkey(sec1_bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(104);
    out.extend_from_slice(&encode_ssh_string(b"ecdsa-sha2-nistp256"));
    out.extend_from_slice(&encode_ssh_string(b"nistp256"));
    out.extend_from_slice(&encode_ssh_string(sec1_bytes));
    out
}

/// Constructs an OpenSSH signature blob for `ssh-ed25519` (RFC 8709).
/// Format: `[string "ssh-ed25519"] || [string signature (64 bytes)]`
pub fn encode_ed25519_signature(sig_bytes: &[u8; 64]) -> Vec<u8> {
    let mut out = Vec::with_capacity(83);
    out.extend_from_slice(&encode_ssh_string(b"ssh-ed25519"));
    out.extend_from_slice(&encode_ssh_string(sig_bytes));
    out
}

/// Constructs an OpenSSH signature blob for `ecdsa-sha2-nistp256` (RFC 5656).
/// Format: `[string "ecdsa-sha2-nistp256"] || [string (mpint r || mpint s)]`
pub fn encode_p256_signature(r: &[u8], s: &[u8]) -> Vec<u8> {
    let mut inner = Vec::with_capacity(72);
    inner.extend_from_slice(&encode_ssh_mpint(r));
    inner.extend_from_slice(&encode_ssh_mpint(s));

    let mut out = Vec::with_capacity(4 + 19 + 4 + inner.len());
    out.extend_from_slice(&encode_ssh_string(b"ecdsa-sha2-nistp256"));
    out.extend_from_slice(&encode_ssh_string(&inner));
    out
}

/// Parsed representation of an SSH key found in a vault entry.
#[derive(Clone, Debug)]
pub enum ParsedSshKey {
    Ed25519 {
        pubkey_blob: Vec<u8>,
        seed: Zeroizing<[u8; 32]>,
    },
    P256 {
        pubkey_blob: Vec<u8>,
        private_key: Zeroizing<Vec<u8>>,
    },
    PublicKeyOnly {
        pubkey_blob: Vec<u8>,
    },
}

impl ParsedSshKey {
    pub fn pubkey_blob(&self) -> &[u8] {
        match self {
            ParsedSshKey::Ed25519 { pubkey_blob, .. } => pubkey_blob,
            ParsedSshKey::P256 { pubkey_blob, .. } => pubkey_blob,
            ParsedSshKey::PublicKeyOnly { pubkey_blob } => pubkey_blob,
        }
    }

    pub fn can_sign(&self) -> bool {
        !matches!(self, ParsedSshKey::PublicKeyOnly { .. })
    }

    /// Sign SSH challenge data. Returns the OpenSSH wire signature blob.
    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>, VaultError> {
        match self {
            ParsedSshKey::Ed25519 { seed, .. } => {
                let signing_key = ed25519_dalek::SigningKey::from_bytes(seed);
                let sig = signing_key.sign(data);
                Ok(encode_ed25519_signature(&sig.to_bytes()))
            }
            ParsedSshKey::P256 { private_key, .. } => {
                let signing_key = P256SigningKey::from_slice(private_key)
                    .map_err(|e| VaultError::EncryptionError(format!("Invalid P-256 key: {}", e)))?;
                let signature: p256::ecdsa::Signature = signing_key.sign(data);
                let (r, s) = signature.split_bytes();
                Ok(encode_p256_signature(&r, &s))
            }
            ParsedSshKey::PublicKeyOnly { .. } => {
                Err(VaultError::DecryptionError("Key does not contain private signing material".into()))
            }
        }
    }
}

/// Parses an OpenSSH public key line (`ssh-ed25519 AAAAC3... comment`).
/// Returns the binary wire public key blob.
pub fn parse_openssh_public_key(line: &str) -> Option<Vec<u8>> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }

    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    let key_type = parts[0];
    let b64_blob = parts[1];

    if !["ssh-ed25519", "ecdsa-sha2-nistp256", "ecdsa-sha2-nistp384", "ecdsa-sha2-nistp521", "ssh-rsa"].contains(&key_type) {
        return None;
    }

    let decoded = BASE64.decode(b64_blob.as_bytes()).ok()?;
    // Check that decoded starts with length-prefixed key_type
    let expected_prefix = encode_ssh_string(key_type.as_bytes());
    if decoded.starts_with(&expected_prefix) {
        Some(decoded)
    } else {
        None
    }
}

/// Parses OpenSSH private key text (`-----BEGIN OPENSSH PRIVATE KEY-----` or raw seed).
pub fn parse_openssh_private_key(text: &str) -> Option<ParsedSshKey> {
    let trimmed = text.trim();

    // Check for OpenSSH private key PEM block
    if trimmed.contains("BEGIN OPENSSH PRIVATE KEY") {
        return parse_openssh_v1_block(trimmed);
    }

    // Check if it's a 64-character hex string (32-byte raw Ed25519 seed)
    // Check if it's a 64 or 128 character hex string (32-byte seed or 64-byte keypair)
    if (trimmed.len() == 64 || trimmed.len() == 128)
        && trimmed.chars().all(|c| c.is_ascii_hexdigit())
        && let Ok(seed_bytes) = data_encoding::HEXLOWER.decode(trimmed[..64].to_lowercase().as_bytes())
        && seed_bytes.len() == 32
    {
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&seed_bytes);
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed);
        let pubkey_bytes = signing_key.verifying_key().to_bytes();
        let pubkey_blob = encode_ed25519_pubkey(&pubkey_bytes);
        return Some(ParsedSshKey::Ed25519 {
            pubkey_blob,
            seed: Zeroizing::new(seed),
        });
    }

    // Check for single-line public key
    for line in text.lines() {
        if let Some(blob) = parse_openssh_public_key(line) {
            return Some(ParsedSshKey::PublicKeyOnly { pubkey_blob: blob });
        }
    }

    None
}

/// Decodes unencrypted `openssh-key-v1` format.
fn parse_openssh_v1_block(pem_text: &str) -> Option<ParsedSshKey> {
    let mut base64_lines = String::new();
    let mut in_block = false;

    for line in pem_text.lines() {
        let l = line.trim();
        if l.contains("BEGIN OPENSSH PRIVATE KEY") {
            in_block = true;
            continue;
        }
        if l.contains("END OPENSSH PRIVATE KEY") {
            break;
        }
        if in_block {
            base64_lines.push_str(l);
        }
    }

    if base64_lines.is_empty() {
        return None;
    }

    let bytes = BASE64.decode(base64_lines.as_bytes()).ok()?;
    let mut cursor = 0;

    // Magic: "openssh-key-v1\0" (15 bytes)
    const MAGIC: &[u8] = b"openssh-key-v1\0";
    if bytes.len() < MAGIC.len() || &bytes[..MAGIC.len()] != MAGIC {
        return None;
    }
    cursor += MAGIC.len();

    // ciphername
    let cipher = read_ssh_string(&bytes, &mut cursor)?;
    // kdfname
    let _kdf = read_ssh_string(&bytes, &mut cursor)?;
    // kdfoptions
    let _kdf_opts = read_ssh_string(&bytes, &mut cursor)?;
    // num_keys (uint32)
    let num_keys = read_u32(&bytes, &mut cursor)?;
    if num_keys != 1 {
        return None;
    }

    // Public key blob
    let pubkey_blob = read_ssh_string(&bytes, &mut cursor)?.to_vec();

    // If unencrypted ("none"), extract private key
    if cipher == b"none" {
        let privkey_blob = read_ssh_string(&bytes, &mut cursor)?;
        let mut priv_cursor = 0;

        let checkint1 = read_u32(privkey_blob, &mut priv_cursor)?;
        let checkint2 = read_u32(privkey_blob, &mut priv_cursor)?;
        if checkint1 != checkint2 {
            return None;
        }

        let key_type = read_ssh_string(privkey_blob, &mut priv_cursor)?;
        if key_type == b"ssh-ed25519" {
            let _pub_bytes = read_ssh_string(privkey_blob, &mut priv_cursor)?;
            let priv_bytes = read_ssh_string(privkey_blob, &mut priv_cursor)?;
            // In OpenSSH, priv_bytes is 64 bytes: 32-byte seed + 32-byte public key
            if priv_bytes.len() >= 32 {
                let mut seed = [0u8; 32];
                seed.copy_from_slice(&priv_bytes[..32]);
                return Some(ParsedSshKey::Ed25519 {
                    pubkey_blob,
                    seed: Zeroizing::new(seed),
                });
            }
        } else if key_type == b"ecdsa-sha2-nistp256" {
            let _curve = read_ssh_string(privkey_blob, &mut priv_cursor)?;
            let _pub = read_ssh_string(privkey_blob, &mut priv_cursor)?;
            let priv_scalar = read_ssh_string(privkey_blob, &mut priv_cursor)?;
            // Normalize mpint scalar to exactly 32 bytes for P256SigningKey
            let mut normalized = [0u8; 32];
            if priv_scalar.len() == 33 && priv_scalar[0] == 0 {
                normalized.copy_from_slice(&priv_scalar[1..33]);
            } else if priv_scalar.len() == 32 {
                normalized.copy_from_slice(priv_scalar);
            } else if !priv_scalar.is_empty() && priv_scalar.len() < 32 {
                let offset = 32 - priv_scalar.len();
                normalized[offset..].copy_from_slice(priv_scalar);
            } else {
                return None;
            }

            return Some(ParsedSshKey::P256 {
                pubkey_blob,
                private_key: Zeroizing::new(normalized.to_vec()),
            });
        }
    }

    // Fallback: at least return the public key identity
    Some(ParsedSshKey::PublicKeyOnly { pubkey_blob })
}

fn read_u32(buf: &[u8], cursor: &mut usize) -> Option<u32> {
    let end = cursor.checked_add(4)?;
    if end > buf.len() {
        return None;
    }
    let val = u32::from_be_bytes(buf[*cursor..end].try_into().ok()?);
    *cursor = end;
    Some(val)
}

fn read_ssh_string<'a>(buf: &'a [u8], cursor: &mut usize) -> Option<&'a [u8]> {
    let len = read_u32(buf, cursor)? as usize;
    let end = cursor.checked_add(len)?;
    if end > buf.len() {
        return None;
    }
    let slice = &buf[*cursor..end];
    *cursor = end;
    Some(slice)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_ssh_string_basic() {
        let encoded = encode_ssh_string(b"hello");
        assert_eq!(encoded, vec![0, 0, 0, 5, b'h', b'e', b'l', b'l', b'o']);
    }

    #[test]
    fn test_encode_ssh_mpint_positive_padding() {
        // High bit set -> requires 0x00 padding
        let scalar = [0x80, 0x01, 0x02];
        let encoded = encode_ssh_mpint(&scalar);
        assert_eq!(encoded, vec![0, 0, 0, 4, 0x00, 0x80, 0x01, 0x02]);

        // High bit not set -> no padding
        let scalar2 = [0x7f, 0x01, 0x02];
        let encoded2 = encode_ssh_mpint(&scalar2);
        assert_eq!(encoded2, vec![0, 0, 0, 3, 0x7f, 0x01, 0x02]);

        // Zero
        let zero = [0x00, 0x00];
        assert_eq!(encode_ssh_mpint(&zero), vec![0, 0, 0, 0]);
    }

    #[test]
    fn test_ed25519_pubkey_and_signing_roundtrip() {
        let seed = [42u8; 32];
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed);
        let pubkey_bytes = signing_key.verifying_key().to_bytes();
        let pubkey_blob = encode_ed25519_pubkey(&pubkey_bytes);

        assert_eq!(pubkey_blob.len(), 51);
        assert!(pubkey_blob.starts_with(&encode_ssh_string(b"ssh-ed25519")));

        let key = ParsedSshKey::Ed25519 {
            pubkey_blob: pubkey_blob.clone(),
            seed: Zeroizing::new(seed),
        };

        let challenge = b"OpenSSH agent test challenge payload";
        let sig_blob = key.sign(challenge).unwrap();
        assert!(sig_blob.starts_with(&encode_ssh_string(b"ssh-ed25519")));
    }

    #[test]
    fn test_p256_pubkey_and_signing_roundtrip() {
        let pair = crate::passkey::generate_passkey_pair().unwrap();
        let pubkey_blob = encode_p256_pubkey(&pair.public_key);
        assert_eq!(pubkey_blob.len(), 104);

        let key = ParsedSshKey::P256 {
            pubkey_blob: pubkey_blob.clone(),
            private_key: pair.private_key,
        };

        let challenge = b"P-256 SSH challenge payload";
        let sig_blob = key.sign(challenge).unwrap();
        assert!(sig_blob.starts_with(&encode_ssh_string(b"ecdsa-sha2-nistp256")));
    }

    #[test]
    fn test_parse_openssh_public_key() {
        let seed = [77u8; 32];
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed);
        let pubkey_bytes = signing_key.verifying_key().to_bytes();
        let pubkey_blob = encode_ed25519_pubkey(&pubkey_bytes);
        let b64 = BASE64.encode(&pubkey_blob);
        let raw = format!("ssh-ed25519 {} user@machine", b64);
        let blob = parse_openssh_public_key(&raw);
        assert_eq!(blob, Some(pubkey_blob));
    }

    #[test]
    fn test_parse_hex_seed() {
        let seed = [99u8; 32];
        let hex_64 = data_encoding::HEXLOWER.encode(&seed);
        let key_64 = parse_openssh_private_key(&hex_64);
        assert!(key_64.is_some());
        assert!(key_64.unwrap().can_sign());

        // 128 char hex (seed + public key)
        let hex_128 = format!("{}{}", hex_64, hex_64);
        let key_128 = parse_openssh_private_key(&hex_128);
        assert!(key_128.is_some());
        assert!(key_128.unwrap().can_sign());
    }
}
