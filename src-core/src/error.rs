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

    #[error("Unsupported vault format version: {0}")]
    UnsupportedVersion(u32),

    #[error("Vault not found: {0}")]
    VaultNotFound(String),

    #[error("Vault file already exists: {0}")]
    VaultAlreadyExists(String),

    #[error("Vault is already open")]
    VaultAlreadyOpen,

    #[error("Entry not found: {0}")]
    EntryNotFound(String),

    #[error("TOTP error: {0}")]
    TotpError(String),

    #[error("Passkey error: {0}")]
    PasskeyError(String),

    #[error("Breach check failed: {0}")]
    BreachCheckError(String),

    #[error("Network request failed: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Bincode error: {0}")]
    BincodeError(#[from] bincode::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Import failed: {0}")]
    ImportError(String),

    #[error("Export failed: {0}")]
    ExportError(String),

    #[error("Synchronization failed: {0}")]
    SyncError(String),

    #[error("Auto-type error: {0}")]
    AutoTypeError(String),

    #[error("Clipboard operation failed: {0}")]
    ClipboardError(String),

    #[error("Protected memory locking failed: {0}")]
    MemoryLockError(String),

    #[error("TPM / Keychain error: {0}")]
    TpmError(String),

    #[error("Password generator error: {0}")]
    GeneratorError(String),

    #[error("Vault is locked")]
    VaultLocked,

    #[error("Integrity check failed - vault may be corrupted or tampered with")]
    IntegrityError,

    #[error("Biometric authentication not available on this device: {0}")]
    BiometricNotAvailable(String),

    #[error("Biometric authentication failed: {0}")]
    BiometricAuthFailed(String),

    #[error("Biometric authentication canceled by user")]
    BiometricCanceled,

    #[error("Biometric hardware error: {0}")]
    BiometricHardwareError(String),

    #[error("Hardware 2FA / YubiKey required to unlock this vault")]
    Hardware2FaRequired,

    #[error("Hardware 2FA authentication failed: {0}")]
    Hardware2FaAuthFailed(String),

    #[error("Hardware 2FA / YubiKey not available: {0}")]
    Hardware2FaNotAvailable(String),

    #[error("Hardware 2FA prompt canceled by user")]
    Hardware2FaCanceled,
}

impl VaultError {
    /// Returns the frontend i18n translation key corresponding to this error variant.
    pub fn translation_key(&self) -> &'static str {
        match self {
            VaultError::EncryptionError(_) => "error.encryption_failed",
            VaultError::DecryptionError(_) => "error.decryption_failed",
            VaultError::InvalidPassword => "error.invalid_password",
            VaultError::KdfError(_) => "error.kdf_failed",
            VaultError::InvalidFormat(_) => "error.invalid_format",
            VaultError::UnsupportedVersion(_) => "error.unsupported_version",
            VaultError::VaultNotFound(_) => "error.vault_not_found",
            VaultError::VaultAlreadyExists(_) => "error.vault_already_exists",
            VaultError::VaultAlreadyOpen => "error.vault_already_open",
            VaultError::EntryNotFound(_) => "error.entry_not_found",
            VaultError::TotpError(_) => "error.totp_error",
            VaultError::PasskeyError(_) => "error.passkey_error",
            VaultError::BreachCheckError(_) | VaultError::NetworkError(_) => "error.breach_check_failed",
            VaultError::IoError(_) => "error.io_error",
            VaultError::SerializationError(_) | VaultError::BincodeError(_) | VaultError::JsonError(_) => "error.serialization_error",
            VaultError::ImportError(_) => "error.import_failed",
            VaultError::ExportError(_) => "error.export_failed",
            VaultError::SyncError(_) => "error.sync_failed",
            VaultError::AutoTypeError(_) => "error.autotype_failed",
            VaultError::ClipboardError(_) => "error.clipboard_error",
            VaultError::MemoryLockError(_) => "error.memory_lock_failed",
            VaultError::TpmError(_) => "error.tpm_error",
            VaultError::GeneratorError(_) => "error.generator_error",
            VaultError::VaultLocked => "error.vault_locked",
            VaultError::IntegrityError => "error.integrity_failed",
            VaultError::BiometricNotAvailable(_) => "error.biometric_not_available",
            VaultError::BiometricAuthFailed(_) => "error.biometric_auth_failed",
            VaultError::BiometricCanceled => "error.biometric_canceled",
            VaultError::BiometricHardwareError(_) => "error.biometric_hardware_error",
            VaultError::Hardware2FaRequired => "error.hardware_2fa_required",
            VaultError::Hardware2FaAuthFailed(_) => "error.hardware_2fa_auth_failed",
            VaultError::Hardware2FaNotAvailable(_) => "error.hardware_2fa_not_available",
            VaultError::Hardware2FaCanceled => "error.hardware_2fa_canceled",
        }
    }

    /// Returns true if the error represents an authentication failure.
    pub fn is_auth_error(&self) -> bool {
        matches!(
            self,
            VaultError::InvalidPassword
                | VaultError::BiometricAuthFailed(_)
                | VaultError::Hardware2FaAuthFailed(_)
                | VaultError::Hardware2FaRequired
        )
    }

    /// Returns true if the user canceled an interactive authentication prompt.
    pub fn is_user_canceled(&self) -> bool {
        matches!(
            self,
            VaultError::BiometricCanceled | VaultError::Hardware2FaCanceled
        )
    }

    /// Returns true if the error indicates data tampering or integrity corruption.
    pub fn is_integrity_error(&self) -> bool {
        matches!(self, VaultError::IntegrityError)
    }
}


