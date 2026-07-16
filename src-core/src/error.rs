use thiserror::Error;

#[derive(Error, Debug)]
pub enum VaultError {
    #[error("Encryption failed: {0}")]
    EncryptionError(String),

    #[error("Decryption failed: {0}")]
    DecryptionError(String),

    #[error("Invalid master password")]
    InvalidPassword,

    #[error("Key derivation failed: {0}")]
    KdfError(String),

    #[error("Invalid vault format: {0}")]
    InvalidFormat(String),

    #[error("Vault not found: {0}")]
    VaultNotFound(String),

    #[error("Entry not found: {0}")]
    EntryNotFound(String),

    #[error("TOTP error: {0}")]
    TotpError(String),

    #[error("Breach check failed: {0}")]
    BreachCheckError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Vault is locked")]
    VaultLocked,

    #[error("Integrity check failed - vault may be corrupted or tampered with")]
    IntegrityError,
}
