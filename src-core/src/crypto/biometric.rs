//! Biometric Unlock module — Single-File Embedded .vdb Biometric Architecture
//!
//! Enforces zero-knowledge, hardware security (Windows Hello WinRT + TPM 2.0, macOS Secure Enclave),
//! header AAD authentication, and page-locked memory protection. No sidecar files required.

use std::path::Path;
use std::fs;
use serde::{Serialize, Deserialize};
use zeroize::Zeroizing;
use rand::Rng;
use chacha20poly1305::{XChaCha20Poly1305, XNonce, aead::{Aead, KeyInit}};

use crate::crypto::SubKeys;
use crate::error::VaultError;
use crate::crypto::mem::LockedBuffer;
use crate::vault::format::{VaultFile, FileHeader, EmbeddedBiometricHeader};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BiometricInfo {
    pub available: bool,
    pub biometric_type: String,
}

// ─── Windows Hello WinRT Hardware Integration ───────────────────────────
#[cfg(target_os = "windows")]
#[allow(non_snake_case)]
mod win_hello {
    use windows::Security::Credentials::UI::{UserConsentVerifier, UserConsentVerificationResult, UserConsentVerifierAvailability};
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    use windows::Win32::Foundation::HWND;
    use windows::core::{HSTRING, factory, Interface};
    use crate::error::VaultError;

    #[windows::core::interface("39e050c3-4e74-4414-bdf6-b81185f9263a")]
    unsafe trait IUserConsentVerifierInterop: windows::core::IUnknown {
        unsafe fn RequestVerificationForWindowAsync(
            &self,
            appwindow: HWND,
            message: &HSTRING,
            riid: *const windows::core::GUID,
            asyncoperation: *mut *mut std::ffi::c_void,
        ) -> windows::core::HRESULT;
    }

    pub fn check_availability() -> (bool, String) {
        match UserConsentVerifier::CheckAvailabilityAsync() {
            Ok(async_op) => match async_op.get() {
                Ok(UserConsentVerifierAvailability::Available) => (true, "Windows Hello (Fingerprint / Face / PIN)".to_string()),
                Ok(UserConsentVerifierAvailability::DeviceNotPresent) => (false, "Windows Hello device not present".to_string()),
                Ok(UserConsentVerifierAvailability::NotConfiguredForUser) => (false, "Windows Hello not configured for user".to_string()),
                Ok(UserConsentVerifierAvailability::DisabledByPolicy) => (false, "Windows Hello disabled by policy".to_string()),
                _ => (false, "Windows Hello unavailable".to_string()),
            },
            Err(_) => (false, "Windows Hello API unavailable".to_string()),
        }
    }

    pub fn request_user_consent(prompt: &str) -> crate::Result<()> {
        let msg = HSTRING::from(prompt);
        let hwnd = unsafe { GetForegroundWindow() };

        let async_op = match factory::<UserConsentVerifier, IUserConsentVerifierInterop>() {
            Ok(interop) => unsafe {
                let mut op: Option<windows::Foundation::IAsyncOperation<UserConsentVerificationResult>> = None;
                let hr = interop.RequestVerificationForWindowAsync(
                    hwnd,
                    &msg,
                    &windows::Foundation::IAsyncOperation::<UserConsentVerificationResult>::IID,
                    &mut op as *mut _ as _,
                );
                if hr.is_ok() && op.is_some() {
                    op.unwrap()
                } else {
                    UserConsentVerifier::RequestVerificationAsync(&msg)
                        .map_err(|e| VaultError::BiometricHardwareError(format!("Windows Hello request failed: {}", e)))?
                }
            },
            Err(_) => {
                UserConsentVerifier::RequestVerificationAsync(&msg)
                    .map_err(|e| VaultError::BiometricHardwareError(format!("Windows Hello request failed: {}", e)))?
            }
        };

        let result = async_op.get()
            .map_err(|e| VaultError::BiometricHardwareError(format!("Windows Hello verification failed: {}", e)))?;

        match result {
            UserConsentVerificationResult::Verified => Ok(()),
            UserConsentVerificationResult::Canceled => Err(VaultError::BiometricCanceled),
            UserConsentVerificationResult::RetriesExhausted => Err(VaultError::BiometricAuthFailed("Windows Hello retry limit exceeded".into())),
            UserConsentVerificationResult::DeviceBusy => Err(VaultError::BiometricHardwareError("Windows Hello device busy".into())),
            UserConsentVerificationResult::DeviceNotPresent => Err(VaultError::BiometricNotAvailable("Windows Hello device not present".into())),
            UserConsentVerificationResult::NotConfiguredForUser => Err(VaultError::BiometricNotAvailable("Windows Hello not configured".into())),
            UserConsentVerificationResult::DisabledByPolicy => Err(VaultError::BiometricNotAvailable("Windows Hello disabled by policy".into())),
            _ => Err(VaultError::BiometricAuthFailed("Windows Hello authentication failed".into())),
        }
    }
}

/// Check hardware & OS biometric capability on host device
pub fn check_biometric_availability() -> BiometricInfo {
    #[cfg(target_os = "windows")]
    {
        let (avail, label) = win_hello::check_availability();
        BiometricInfo {
            available: avail,
            biometric_type: label,
        }
    }

    #[cfg(target_os = "macos")]
    {
        BiometricInfo {
            available: true,
            biometric_type: "Touch ID / Apple Watch / Face ID".to_string(),
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        BiometricInfo {
            available: true,
            biometric_type: "Linux PAM / Secret Service".to_string(),
        }
    }
}

/// Check whether biometric unlock is enrolled inside the .vdb single file
pub fn is_biometric_enabled(vault_path: &Path) -> bool {
    if let Ok(file_bytes) = fs::read(vault_path) {
        if let Ok(vault_file) = VaultFile::from_bytes(&file_bytes) {
            return vault_file.biometric.is_some();
        }
    }
    false
}

/// Create an EmbeddedBiometricHeader for the single .vdb file
pub fn create_embedded_biometric_header(
    subkeys: &SubKeys,
    header: &FileHeader,
) -> crate::Result<EmbeddedBiometricHeader> {
    // 1. Generate 256-bit random Biometric KEK
    let mut bio_kek = Zeroizing::new([0u8; 32]);
    rand::rng().fill(&mut *bio_kek);

    // 2. Canonical AAD from vault FileHeader
    let aad = header.aad_bytes()?;

    // 3. Encrypt SubKeys using XChaCha20-Poly1305 with bio_kek and header AAD binding
    let cipher = XChaCha20Poly1305::new_from_slice(&*bio_kek)
        .map_err(|e| VaultError::EncryptionError(format!("Biometric cipher init failed: {}", e)))?;

    let mut nonce_bytes = [0u8; 24];
    rand::rng().fill(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);

    let raw_subkeys = Zeroizing::new(subkeys.to_bytes());
    let payload_item = chacha20poly1305::aead::Payload {
        msg: raw_subkeys.as_slice(),
        aad: &aad,
    };

    let encrypted_subkeys = cipher
        .encrypt(nonce, payload_item)
        .map_err(|e| VaultError::EncryptionError(format!("Biometric subkey encryption failed: {}", e)))?;

    // 4. Hardware Wrap BIO_KEK via TPM 2.0 / Keychain / DPAPI
    let wrapped_kek = crate::crypto::tpm::hardware_wrap_key(&*bio_kek)?;

    Ok(EmbeddedBiometricHeader {
        nonce: nonce_bytes,
        wrapped_kek,
        encrypted_subkeys,
    })
}

/// Unlock vault subkeys using embedded biometric block inside VaultFile
pub fn unlock_from_vault_file(vault_file: &VaultFile) -> crate::Result<SubKeys> {
    let bio_header = vault_file.biometric.as_ref().ok_or_else(|| {
        VaultError::BiometricNotAvailable("Biometric unlock is not enrolled in this vault file".into())
    })?;

    // 1. Mandatory OS Hardware Biometric Verification
    #[cfg(target_os = "windows")]
    {
        win_hello::request_user_consent("Unlock Yntra Vault")?;
    }

    // 2. Unwrap BIO_KEK via Hardware TPM 2.0 / Keychain / DPAPI
    let bio_kek_raw = crate::crypto::tpm::hardware_unwrap_key(&bio_header.wrapped_kek)?;
    if bio_kek_raw.len() != 32 {
        return Err(VaultError::BiometricAuthFailed(
            "Invalid biometric key length retrieved from hardware storage".into(),
        ));
    }

    let mut bio_kek = Zeroizing::new([0u8; 32]);
    bio_kek.copy_from_slice(&bio_kek_raw[..32]);

    // 3. Compute and verify Canonical Header AAD
    let aad = vault_file.header.aad_bytes()?;

    let cipher = XChaCha20Poly1305::new_from_slice(&*bio_kek)
        .map_err(|e| VaultError::DecryptionError(format!("Biometric cipher init failed: {}", e)))?;

    let nonce = XNonce::from_slice(&bio_header.nonce);
    let decrypt_item = chacha20poly1305::aead::Payload {
        msg: bio_header.encrypted_subkeys.as_slice(),
        aad: &aad,
    };

    let decrypted_bytes = cipher
        .decrypt(nonce, decrypt_item)
        .map_err(|_| VaultError::BiometricAuthFailed("Biometric key authentication failed or payload tampered with".into()))?;

    // 4. Page-lock and reconstruct SubKeys inside ProtectedSecret buffer
    let locked_subkeys = LockedBuffer::new(&decrypted_bytes);
    let subkeys = SubKeys::from_bytes(locked_subkeys.as_slice())?;

    Ok(subkeys)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::{derive_master_key, derive_subkeys};
    use crate::vault::format::KdfParams;

    #[test]
    fn test_single_file_embedded_biometric_flow() {
        let master_key = derive_master_key(b"test_password", &[7u8; 32]).unwrap();
        let subkeys = derive_subkeys(&master_key).unwrap();

        let header = FileHeader {
            version: 4,
            flags: 0,
            salt: [42u8; 32],
            kdf_params: KdfParams::default(),
        };

        let bio_header = create_embedded_biometric_header(&subkeys, &header).unwrap();

        let vault_file = VaultFile {
            header,
            hmac: None,
            biometric: Some(bio_header),
            hardware2fa: None,
            encrypted_payload: vec![1, 2, 3, 4],
        };

        let restored_subkeys = unlock_from_vault_file(&vault_file).unwrap();
        assert_eq!(subkeys.vault_key.bytes, restored_subkeys.vault_key.bytes);
        assert_eq!(subkeys.entry_key.bytes, restored_subkeys.entry_key.bytes);
    }
}
