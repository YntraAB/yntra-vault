//! .vdb binary vault file format
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
pub const FORMAT_VERSION: u16 = 4;
pub const FLAG_HAS_BIOMETRIC: u16 = 0x0001;
pub const FLAG_HAS_HARDWARE_2FA: u16 = 0x0002;

/// Embedded biometric header block stored inside the .vdb file when biometrics is enrolled.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EmbeddedBiometricHeader {
    pub nonce: [u8; 24],
    pub wrapped_kek: Vec<u8>,
    pub encrypted_subkeys: Vec<u8>,
}

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

impl KdfParams {
    /// Minimum acceptable thresholds to prevent downgrade attacks via crafted .vdb files.
    const MIN_MEMORY_KB: u32 = 65_536;   // 64 MB absolute minimum
    const MIN_ITERATIONS: u32 = 2;
    const MIN_PARALLELISM: u32 = 1;
    const REQUIRED_OUTPUT_LEN: usize = 64;

    /// Reject KDF params below security minimums.
    pub fn validate(&self) -> crate::Result<()> {
        if self.memory_kb < Self::MIN_MEMORY_KB {
            return Err(VaultError::InvalidFormat(format!(
                "KDF memory_kb {} below minimum {}", self.memory_kb, Self::MIN_MEMORY_KB
            )));
        }
        if self.iterations < Self::MIN_ITERATIONS {
            return Err(VaultError::InvalidFormat(format!(
                "KDF iterations {} below minimum {}", self.iterations, Self::MIN_ITERATIONS
            )));
        }
        if self.parallelism < Self::MIN_PARALLELISM {
            return Err(VaultError::InvalidFormat(format!(
                "KDF parallelism {} below minimum {}", self.parallelism, Self::MIN_PARALLELISM
            )));
        }
        if self.output_len != Self::REQUIRED_OUTPUT_LEN {
            return Err(VaultError::InvalidFormat(format!(
                "KDF output_len {} must be {}", self.output_len, Self::REQUIRED_OUTPUT_LEN
            )));
        }
        Ok(())
    }
}

/// File header — written unencrypted at the start of the .vdb file.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileHeader {
    pub version: u16,
    pub flags: u16,
    pub salt: [u8; 32],
    pub kdf_params: KdfParams,
}

impl FileHeader {
    /// Canonical serialization of header fields for AEAD Additional Authenticated Data (AAD) binding.
    pub fn aad_bytes(&self) -> crate::Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(128);
        buf.write_all(MAGIC_BYTES)
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;
        buf.write_all(&self.version.to_le_bytes())
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;
        buf.write_all(&self.flags.to_le_bytes())
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;
        buf.write_all(&self.salt)
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;

        let kdf_bytes = bincode::serialize(&self.kdf_params)
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;
        let kdf_len = kdf_bytes.len() as u32;
        buf.write_all(&kdf_len.to_le_bytes())
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;
        buf.write_all(&kdf_bytes)
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;

        Ok(buf)
    }
}

/// Complete vault file structure.
pub struct VaultFile {
    pub header: FileHeader,
    pub hmac: Option<[u8; 64]>,
    pub biometric: Option<EmbeddedBiometricHeader>,
    pub hardware2fa: Option<Vec<crate::crypto::hardware2fa::EmbeddedHardware2FaHeader>>,
    pub encrypted_payload: Vec<u8>,
}

impl VaultFile {
    /// Serialize the vault file to bytes for writing to disk.
    pub fn to_bytes(&self) -> crate::Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(256 + self.encrypted_payload.len());

        let mut flags = self.header.flags;
        if self.biometric.is_some() {
            flags |= FLAG_HAS_BIOMETRIC;
        } else {
            flags &= !FLAG_HAS_BIOMETRIC;
        }
        if self.hardware2fa.is_some() {
            flags |= FLAG_HAS_HARDWARE_2FA;
        } else {
            flags &= !FLAG_HAS_HARDWARE_2FA;
        }

        // Magic bytes
        buf.write_all(MAGIC_BYTES)
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;

        // Version (u16 LE)
        buf.write_all(&self.header.version.to_le_bytes())
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;

        // Flags (u16 LE)
        buf.write_all(&flags.to_le_bytes())
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;

        // Salt (32 bytes)
        buf.write_all(&self.header.salt)
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;

        // HMAC (64 bytes) — included only for legacy v1 and v2 files
        if self.header.version <= 2 {
            let hmac_bytes = self.hmac.unwrap_or([0u8; 64]);
            buf.write_all(&hmac_bytes)
                .map_err(|e| VaultError::SerializationError(e.to_string()))?;
        }

        // KDF params (bincode serialized with length prefix)
        let kdf_bytes = bincode::serialize(&self.header.kdf_params)
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;
        let kdf_len = kdf_bytes.len() as u32;
        buf.write_all(&kdf_len.to_le_bytes())
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;
        buf.write_all(&kdf_bytes)
            .map_err(|e| VaultError::SerializationError(e.to_string()))?;

        // Embedded Biometric container block (if FLAG_HAS_BIOMETRIC)
        if let Some(ref bio) = self.biometric {
            let bio_bytes = bincode::serialize(bio)
                .map_err(|e| VaultError::SerializationError(format!("Biometric serialize: {}", e)))?;
            let bio_len = bio_bytes.len() as u32;
            buf.write_all(&bio_len.to_le_bytes())
                .map_err(|e| VaultError::SerializationError(e.to_string()))?;
            buf.write_all(&bio_bytes)
                .map_err(|e| VaultError::SerializationError(e.to_string()))?;
        }

        // Embedded Hardware 2FA container block (if FLAG_HAS_HARDWARE_2FA)
        if let Some(ref hw) = self.hardware2fa {
            let hw_bytes = bincode::serialize(hw)
                .map_err(|e| VaultError::SerializationError(format!("Hardware 2FA serialize: {}", e)))?;
            let hw_len = hw_bytes.len() as u32;
            buf.write_all(&hw_len.to_le_bytes())
                .map_err(|e| VaultError::SerializationError(e.to_string()))?;
            buf.write_all(&hw_bytes)
                .map_err(|e| VaultError::SerializationError(e.to_string()))?;
        }

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
                "Not a valid .vdb file (wrong magic bytes)".into(),
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

        // HMAC (64 bytes) for legacy v1 and v2 files
        let hmac = if version <= 2 {
            let mut hmac_buf = [0u8; 64];
            cursor.read_exact(&mut hmac_buf)
                .map_err(|_| VaultError::InvalidFormat("Failed to read legacy HMAC".into()))?;
            Some(hmac_buf)
        } else {
            None
        };

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

        // Reject weakened KDF parameters (prevents downgrade attacks)
        kdf_params.validate()?;

        // Embedded Biometric container block (if FLAG_HAS_BIOMETRIC)
        let biometric = if flags & FLAG_HAS_BIOMETRIC != 0 {
            let mut bio_len_bytes = [0u8; 4];
            cursor.read_exact(&mut bio_len_bytes)
                .map_err(|_| VaultError::InvalidFormat("Failed to read biometric block length".into()))?;
            let bio_len = u32::from_le_bytes(bio_len_bytes) as usize;

            let mut bio_bytes = vec![0u8; bio_len];
            cursor.read_exact(&mut bio_bytes)
                .map_err(|_| VaultError::InvalidFormat("Failed to read biometric block".into()))?;
            let bio_header: EmbeddedBiometricHeader = bincode::deserialize(&bio_bytes)
                .map_err(|e| VaultError::InvalidFormat(format!("Invalid biometric block: {}", e)))?;
            Some(bio_header)
        } else {
            None
        };

        // Embedded Hardware 2FA container block (if FLAG_HAS_HARDWARE_2FA)
        let hardware2fa = if flags & FLAG_HAS_HARDWARE_2FA != 0 {
            let mut hw_len_bytes = [0u8; 4];
            cursor.read_exact(&mut hw_len_bytes)
                .map_err(|_| VaultError::InvalidFormat("Failed to read hardware 2FA block length".into()))?;
            let hw_len = u32::from_le_bytes(hw_len_bytes) as usize;

            let mut hw_bytes = vec![0u8; hw_len];
            cursor.read_exact(&mut hw_bytes)
                .map_err(|_| VaultError::InvalidFormat("Failed to read hardware 2FA block".into()))?;
            match bincode::deserialize::<Vec<crate::crypto::hardware2fa::EmbeddedHardware2FaHeader>>(&hw_bytes) {
                Ok(hw_vec) => Some(hw_vec),
                Err(_) => {
                    let single: crate::crypto::hardware2fa::EmbeddedHardware2FaHeader = bincode::deserialize(&hw_bytes)
                        .map_err(|e| VaultError::InvalidFormat(format!("Invalid hardware 2FA block: {}", e)))?;
                    Some(vec![single])
                }
            }
        } else {
            None
        };

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
            biometric,
            hardware2fa,
            encrypted_payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_v3_roundtrip_file_format() {
        let file = VaultFile {
            header: FileHeader {
                version: FORMAT_VERSION,
                flags: 0,
                salt: [42u8; 32],
                kdf_params: KdfParams::default(),
            },
            hmac: None,
            biometric: None,
            hardware2fa: None,
            encrypted_payload: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };

        let bytes = file.to_bytes().unwrap();
        let parsed = VaultFile::from_bytes(&bytes).unwrap();

        assert_eq!(parsed.header.version, FORMAT_VERSION);
        assert_eq!(parsed.header.salt, [42u8; 32]);
        assert!(parsed.hmac.is_none());
        assert!(parsed.biometric.is_none());
        assert!(parsed.hardware2fa.is_none());
        assert_eq!(parsed.encrypted_payload, vec![1, 2, 3, 4, 5, 6, 7, 8]);

        // AAD bytes generation test
        let aad = file.header.aad_bytes().unwrap();
        assert!(!aad.is_empty());
    }

    #[test]
    fn test_v4_embedded_biometric_roundtrip_file_format() {
        let bio = EmbeddedBiometricHeader {
            nonce: [7u8; 24],
            wrapped_kek: vec![10, 20, 30],
            encrypted_subkeys: vec![1, 2, 3, 4, 5],
        };

        let file = VaultFile {
            header: FileHeader {
                version: FORMAT_VERSION,
                flags: FLAG_HAS_BIOMETRIC,
                salt: [99u8; 32],
                kdf_params: KdfParams::default(),
            },
            hmac: None,
            biometric: Some(bio.clone()),
            hardware2fa: None,
            encrypted_payload: vec![9, 8, 7, 6],
        };

        let bytes = file.to_bytes().unwrap();
        let parsed = VaultFile::from_bytes(&bytes).unwrap();

        assert_eq!(parsed.header.version, FORMAT_VERSION);
        assert_eq!(parsed.header.flags & FLAG_HAS_BIOMETRIC, FLAG_HAS_BIOMETRIC);
        assert_eq!(parsed.biometric, Some(bio));
        assert!(parsed.hardware2fa.is_none());
        assert_eq!(parsed.encrypted_payload, vec![9, 8, 7, 6]);
    }

    #[test]
    fn test_v5_hardware2fa_roundtrip_file_format() {
        let hw = crate::crypto::hardware2fa::EmbeddedHardware2FaHeader {
            protocol: crate::crypto::hardware2fa::Hardware2FaProtocol::YubiKeyChallengeResponse,
            credential_id: vec![1, 2, 3, 4],
            challenge_salt: [5u8; 32],
            nonce: [9u8; 24],
            wrapped_kek: vec![11, 22, 33],
            encrypted_subkeys: vec![44, 55, 66],
            key_name: "Test YubiKey".to_string(),
        };

        let file = VaultFile {
            header: FileHeader {
                version: FORMAT_VERSION,
                flags: FLAG_HAS_HARDWARE_2FA,
                salt: [88u8; 32],
                kdf_params: KdfParams::default(),
            },
            hmac: None,
            biometric: None,
            hardware2fa: Some(vec![hw.clone()]),
            encrypted_payload: vec![3, 2, 1],
        };

        let bytes = file.to_bytes().unwrap();
        let parsed = VaultFile::from_bytes(&bytes).unwrap();

        assert_eq!(parsed.header.version, FORMAT_VERSION);
        assert_eq!(parsed.header.flags & FLAG_HAS_HARDWARE_2FA, FLAG_HAS_HARDWARE_2FA);
        assert_eq!(parsed.hardware2fa, Some(vec![hw]));
        assert_eq!(parsed.encrypted_payload, vec![3, 2, 1]);
    }

    #[test]
    fn test_v2_legacy_roundtrip_file_format() {
        let file = VaultFile {
            header: FileHeader {
                version: 2,
                flags: 0,
                salt: [42u8; 32],
                kdf_params: KdfParams::default(),
            },
            hmac: Some([0xAB; 64]),
            biometric: None,
            hardware2fa: None,
            encrypted_payload: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };

        let bytes = file.to_bytes().unwrap();
        let parsed = VaultFile::from_bytes(&bytes).unwrap();

        assert_eq!(parsed.header.version, 2);
        assert_eq!(parsed.hmac, Some([0xAB; 64]));
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

