use thiserror::Error;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Encryption failed: {0}")]
    EncryptionError(String),

    #[error("Decryption failed: {0}")]
    DecryptionError(String),

    #[error("Invalid master password")]
    InvalidPassword,

    #[error("Key derivation failed: {0}")]
    KdfError(String),

    #[error("Protected memory locking failed: {0}")]
    MemoryLockError(String),

    #[error("Clipboard operation failed: {0}")]
    ClipboardError(String),

    #[error("Integrity check failed - data may be corrupted or tampered with")]
    IntegrityError,

    #[error("Biometric authentication not available on this device: {0}")]
    BiometricNotAvailable(String),

    #[error("Biometric authentication failed: {0}")]
    BiometricAuthFailed(String),

    #[error("Biometric authentication canceled by user")]
    BiometricCanceled,

    #[error("Biometric hardware error: {0}")]
    BiometricHardwareError(String),

    #[error("Hardware 2FA device not available: {0}")]
    Hardware2FaNotAvailable(String),

    #[error("Hardware 2FA authentication failed: {0}")]
    Hardware2FaAuthFailed(String),

    #[error("Hardware 2FA required for this vault")]
    Hardware2FaRequired,

    #[error("TPM / Keychain error: {0}")]
    TpmError(String),

    #[error("Passkey error: {0}")]
    PasskeyError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

pub type Result<T> = std::result::Result<T, CryptoError>;
pub type VaultError = CryptoError;
