//! .yntravault binary vault file format
//!
//! ┌──────────────────────────────────┐
//! │  Magic: "YNTR" (4 bytes)         │
//! │  Version: u16 (2 bytes)          │
//! │  Flags: u16 (2 bytes)            │
//! │  Salt: [u8; 32] (32 bytes)       │
//! │  Nonce: [u8; 24] (24 bytes)      │
//! │  HMAC: [u8; 64] (64 bytes)       │
//! │  KDF Params (serialized)         │
//! │  Payload Length: u64 (8 bytes)    │
//! │  ────────────────────────────────│
//! │  Encrypted Payload (bincode)     │
//! └──────────────────────────────────┘

use std::io::{Read, Write, Cursor};
use serde::{Deserialize, Serialize};
use crate::error::VaultError;

pub const MAGIC_BYTES: &[u8; 4] = b"YNTR";
pub const FORMAT_VERSION: u16 = 1;

/// KDF parameters stored in the file so we can always decrypt.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct KdfParams {
    pub memory_kb: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub output_len: usize,
}

impl Default for KdfParams {
    fn default() -> Self {
        KdfParams {
            memory_kb: 262_144,  // 256 MB
            iterations: 4,
            parallelism: 4,
            output_len: 64,
        }
    }
}

/// File header — written unencrypted at the start of the .yntravault file.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileHeader {
    pub version: u16,
    pub flags: u16,
    pub salt: [u8; 32],
    pub kdf_params: KdfParams,
}

/// Complete vault file structure.
pub struct VaultFile {
    pub header: FileHeader,
    pub hmac: [u8; 64],
    pub encrypted_payload: Vec<u8>,
}

impl VaultFile {
    /// Serialize the vault file to bytes for writing to disk.
    pub fn to_bytes(&self) -> crate::Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(256 + self.encrypted_payload.len());

        // Magic bytes
        buf.write_all(MAGIC_BYTES)
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;

        // Version (u16 LE)
        buf.write_all(&self.header.version.to_le_bytes())
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;

        // Flags (u16 LE)
        buf.write_all(&self.header.flags.to_le_bytes())
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;

        // Salt (32 bytes)
        buf.write_all(&self.header.salt)
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;

        // HMAC (64 bytes)
        buf.write_all(&self.hmac)
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;

        // KDF params (bincode serialized with length prefix)
        let kdf_bytes = bincode::serialize(&self.header.kdf_params)
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;
        let kdf_len = kdf_bytes.len() as u32;
        buf.write_all(&kdf_len.to_le_bytes())
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;
        buf.write_all(&kdf_bytes)
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;

        // Encrypted payload length (u64 LE)
        let payload_len = self.encrypted_payload.len() as u64;
        buf.write_all(&payload_len.to_le_bytes())
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;

        // Encrypted payload
        buf.write_all(&self.encrypted_payload)
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;

        Ok(buf)
    }

    /// Parse a vault file from bytes.
    pub fn from_bytes(data: &[u8]) -> crate::Result<Self> {
        let mut cursor = Cursor::new(data);

        // Magic bytes
        let mut magic = [0u8; 4];
        cursor.read_exact(&mut magic)
            .map_err(|_| VaultError::InvalidFormat("Too short to read magic bytes".into()))?;
        if &magic != MAGIC_BYTES {
            return Err(VaultError::InvalidFormat(
                "Not a valid .yntravault file (wrong magic bytes)".into(),
            ));
        }

        // Version
        let mut version_bytes = [0u8; 2];
        cursor.read_exact(&mut version_bytes)
            .map_err(|_| VaultError::InvalidFormat("Failed to read version".into()))?;
        let version = u16::from_le_bytes(version_bytes);

        if version > FORMAT_VERSION {
            return Err(VaultError::InvalidFormat(format!(
                "Unsupported vault version {} (max supported: {})",
                version, FORMAT_VERSION
            )));
        }

        // Flags
        let mut flags_bytes = [0u8; 2];
        cursor.read_exact(&mut flags_bytes)
            .map_err(|_| VaultError::InvalidFormat("Failed to read flags".into()))?;
        let flags = u16::from_le_bytes(flags_bytes);

        // Salt
        let mut salt = [0u8; 32];
        cursor.read_exact(&mut salt)
            .map_err(|_| VaultError::InvalidFormat("Failed to read salt".into()))?;

        // HMAC
        let mut hmac = [0u8; 64];
        cursor.read_exact(&mut hmac)
            .map_err(|_| VaultError::InvalidFormat("Failed to read HMAC".into()))?;

        // KDF params length
        let mut kdf_len_bytes = [0u8; 4];
        cursor.read_exact(&mut kdf_len_bytes)
            .map_err(|_| VaultError::InvalidFormat("Failed to read KDF params length".into()))?;
        let kdf_len = u32::from_le_bytes(kdf_len_bytes) as usize;

        // KDF params
        let mut kdf_bytes = vec![0u8; kdf_len];
        cursor.read_exact(&mut kdf_bytes)
            .map_err(|_| VaultError::InvalidFormat("Failed to read KDF params".into()))?;
        let kdf_params: KdfParams = bincode::deserialize(&kdf_bytes)
            .map_err(|e| VaultError::InvalidFormat(format!("Invalid KDF params: {}", e)))?;

        // Payload length
        let mut payload_len_bytes = [0u8; 8];
        cursor.read_exact(&mut payload_len_bytes)
            .map_err(|_| VaultError::InvalidFormat("Failed to read payload length".into()))?;
        let payload_len = u64::from_le_bytes(payload_len_bytes) as usize;

        // Encrypted payload
        let mut encrypted_payload = vec![0u8; payload_len];
        cursor.read_exact(&mut encrypted_payload)
            .map_err(|_| VaultError::InvalidFormat("Failed to read encrypted payload".into()))?;

        Ok(VaultFile {
            header: FileHeader {
                version,
                flags,
                salt,
                kdf_params,
            },
            hmac,
            encrypted_payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_file_format() {
        let file = VaultFile {
            header: FileHeader {
                version: FORMAT_VERSION,
                flags: 0,
                salt: [42u8; 32],
                kdf_params: KdfParams::default(),
            },
            hmac: [0xAB; 64],
            encrypted_payload: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };

        let bytes = file.to_bytes().unwrap();
        let parsed = VaultFile::from_bytes(&bytes).unwrap();

        assert_eq!(parsed.header.version, FORMAT_VERSION);
        assert_eq!(parsed.header.salt, [42u8; 32]);
        assert_eq!(parsed.hmac, [0xAB; 64]);
        assert_eq!(parsed.encrypted_payload, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn test_invalid_magic() {
        let data = b"NOPE rest of file";
        let result = VaultFile::from_bytes(data);
        assert!(result.is_err());
    }

    #[test]
    fn test_truncated_file() {
        let result = VaultFile::from_bytes(b"YN");
        assert!(result.is_err());
    }
}
