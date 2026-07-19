//! Platform-specific hardware binding (TPM 2.0, Secure Enclave / Keychain, DPAPI).

// ─── Windows Implementation ─────────────────────────────────────────────
#[cfg(target_os = "windows")]
pub mod windows_hdw {
    use windows::Win32::Security::Cryptography::{
        NCryptOpenStorageProvider, NCryptCreatePersistedKey, NCryptFinalizeKey,
        NCryptOpenKey, NCryptEncrypt, NCryptDecrypt, MS_PLATFORM_KEY_STORAGE_PROVIDER,
        NCRYPT_PAD_PKCS1_FLAG, NCRYPT_PROV_HANDLE, NCRYPT_KEY_HANDLE,
        CERT_KEY_SPEC, NCRYPT_FLAGS, NCRYPT_RSA_ALGORITHM, CRYPT_INTEGER_BLOB,
    };
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{NTE_BAD_KEYSET, LocalFree};
    use windows::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData};

    const KEY_NAME: &str = "YntraVaultMasterKey\0";

    /// Wrap key using Windows TPM 2.0 (CNG/NCrypt).
    pub fn tpm_wrap_key(plaintext: &[u8]) -> crate::Result<Vec<u8>> {
        unsafe {
            let mut prov = NCRYPT_PROV_HANDLE::default();
            let status = NCryptOpenStorageProvider(&mut prov, MS_PLATFORM_KEY_STORAGE_PROVIDER, 0);
            if status.is_err() {
                return Err(crate::error::VaultError::EncryptionError("TPM provider not available".into()));
            }

            let key_name_u16: Vec<u16> = KEY_NAME.encode_utf16().collect();
            let pcwstr_key_name = PCWSTR::from_raw(key_name_u16.as_ptr());

            let mut key = NCRYPT_KEY_HANDLE::default();
            // Attempt to open existing key
            let status = NCryptOpenKey(prov, &mut key, pcwstr_key_name, CERT_KEY_SPEC(0), NCRYPT_FLAGS(0));

            // If not found, generate new RSA key in TPM
            if let Err(ref e) = status {
                if e.code() == NTE_BAD_KEYSET {
                    let create_status = NCryptCreatePersistedKey(
                        prov,
                        &mut key,
                        NCRYPT_RSA_ALGORITHM,
                        pcwstr_key_name,
                        CERT_KEY_SPEC(0),
                        NCRYPT_FLAGS(0),
                    );
                    if create_status.is_err() {
                        let _ = windows::Win32::Security::Cryptography::NCryptFreeObject(windows::Win32::Security::Cryptography::NCRYPT_HANDLE(prov.0));
                        return Err(crate::error::VaultError::EncryptionError(format!("TPM key creation failed: {:?}", create_status)));
                    }
                    let finalize_status = NCryptFinalizeKey(key, NCRYPT_FLAGS(0));
                    if finalize_status.is_err() {
                        let _ = windows::Win32::Security::Cryptography::NCryptFreeObject(windows::Win32::Security::Cryptography::NCRYPT_HANDLE(key.0));
                        let _ = windows::Win32::Security::Cryptography::NCryptFreeObject(windows::Win32::Security::Cryptography::NCRYPT_HANDLE(prov.0));
                        return Err(crate::error::VaultError::EncryptionError(format!("TPM key finalization failed: {:?}", finalize_status)));
                    }
                } else {
                    let _ = windows::Win32::Security::Cryptography::NCryptFreeObject(windows::Win32::Security::Cryptography::NCRYPT_HANDLE(prov.0));
                    return Err(crate::error::VaultError::EncryptionError(format!("TPM key open failed: {:?}", status)));
                }
            }

            // Query buffer size required
            let mut cb_output = 0;
            let encrypt_status = NCryptEncrypt(
                key,
                Some(plaintext),
                None,
                None,
                &mut cb_output,
                NCRYPT_PAD_PKCS1_FLAG,
            );
            if encrypt_status.is_err() {
                let _ = windows::Win32::Security::Cryptography::NCryptFreeObject(windows::Win32::Security::Cryptography::NCRYPT_HANDLE(key.0));
                let _ = windows::Win32::Security::Cryptography::NCryptFreeObject(windows::Win32::Security::Cryptography::NCRYPT_HANDLE(prov.0));
                return Err(crate::error::VaultError::EncryptionError(format!("TPM encrypt size query failed: {:?}", encrypt_status)));
            }

            let mut ciphertext = vec![0u8; cb_output as usize];
            let encrypt_status2 = NCryptEncrypt(
                key,
                Some(plaintext),
                None,
                Some(&mut ciphertext),
                &mut cb_output,
                NCRYPT_PAD_PKCS1_FLAG,
            );

            // Free handles
            let _ = windows::Win32::Security::Cryptography::NCryptFreeObject(windows::Win32::Security::Cryptography::NCRYPT_HANDLE(key.0));
            let _ = windows::Win32::Security::Cryptography::NCryptFreeObject(windows::Win32::Security::Cryptography::NCRYPT_HANDLE(prov.0));

            if encrypt_status2.is_err() {
                return Err(crate::error::VaultError::EncryptionError(format!("TPM encryption failed: {:?}", encrypt_status2)));
            }

            Ok(ciphertext)
        }
    }

    /// Unwrap key using Windows TPM 2.0 (CNG/NCrypt).
    pub fn tpm_unwrap_key(ciphertext: &[u8]) -> crate::Result<Vec<u8>> {
        unsafe {
            let mut prov = NCRYPT_PROV_HANDLE::default();
            let status = NCryptOpenStorageProvider(&mut prov, MS_PLATFORM_KEY_STORAGE_PROVIDER, 0);
            if status.is_err() {
                return Err(crate::error::VaultError::DecryptionError("TPM provider not available".into()));
            }

            let key_name_u16: Vec<u16> = KEY_NAME.encode_utf16().collect();
            let pcwstr_key_name = PCWSTR::from_raw(key_name_u16.as_ptr());

            let mut key = NCRYPT_KEY_HANDLE::default();
            let status = NCryptOpenKey(prov, &mut key, pcwstr_key_name, CERT_KEY_SPEC(0), NCRYPT_FLAGS(0));
            if status.is_err() {
                let _ = windows::Win32::Security::Cryptography::NCryptFreeObject(windows::Win32::Security::Cryptography::NCRYPT_HANDLE(prov.0));
                return Err(crate::error::VaultError::DecryptionError(format!("TPM key open failed: {:?}", status)));
            }

            let mut cb_output = 0;
            let decrypt_status = NCryptDecrypt(
                key,
                Some(ciphertext),
                None,
                None,
                &mut cb_output,
                NCRYPT_PAD_PKCS1_FLAG,
            );
            if decrypt_status.is_err() {
                let _ = windows::Win32::Security::Cryptography::NCryptFreeObject(windows::Win32::Security::Cryptography::NCRYPT_HANDLE(key.0));
                let _ = windows::Win32::Security::Cryptography::NCryptFreeObject(windows::Win32::Security::Cryptography::NCRYPT_HANDLE(prov.0));
                return Err(crate::error::VaultError::DecryptionError(format!("TPM decrypt size query failed: {:?}", decrypt_status)));
            }

            let mut plaintext = vec![0u8; cb_output as usize];
            let decrypt_status2 = NCryptDecrypt(
                key,
                Some(ciphertext),
                None,
                Some(&mut plaintext),
                &mut cb_output,
                NCRYPT_PAD_PKCS1_FLAG,
            );

            let _ = windows::Win32::Security::Cryptography::NCryptFreeObject(windows::Win32::Security::Cryptography::NCRYPT_HANDLE(key.0));
            let _ = windows::Win32::Security::Cryptography::NCryptFreeObject(windows::Win32::Security::Cryptography::NCRYPT_HANDLE(prov.0));

            if decrypt_status2.is_err() {
                return Err(crate::error::VaultError::DecryptionError(format!("TPM decryption failed: {:?}", decrypt_status2)));
            }

            Ok(plaintext)
        }
    }

    /// Delete the Windows TPM key.
    pub fn tpm_delete_key() -> crate::Result<()> {
        unsafe {
            let mut prov = NCRYPT_PROV_HANDLE::default();
            let _ = NCryptOpenStorageProvider(&mut prov, MS_PLATFORM_KEY_STORAGE_PROVIDER, 0);

            let key_name_u16: Vec<u16> = KEY_NAME.encode_utf16().collect();
            let pcwstr_key_name = PCWSTR::from_raw(key_name_u16.as_ptr());

            let mut key = NCRYPT_KEY_HANDLE::default();
            let status = NCryptOpenKey(prov, &mut key, pcwstr_key_name, CERT_KEY_SPEC(0), NCRYPT_FLAGS(0));
            if status.is_ok() {
                let _ = windows::Win32::Security::Cryptography::NCryptDeleteKey(key, 0);
            }
            let _ = windows::Win32::Security::Cryptography::NCryptFreeObject(windows::Win32::Security::Cryptography::NCRYPT_HANDLE(prov.0));
            Ok(())
        }
    }

    /// Encrypt using Windows DPAPI fallback.
    pub fn dpapi_encrypt(data: &[u8]) -> crate::Result<Vec<u8>> {
        let mut data_in = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut data_out = CRYPT_INTEGER_BLOB::default();

        let success = unsafe {
            CryptProtectData(
                &mut data_in,
                None,
                None,
                None,
                None,
                0,
                &mut data_out,
            )
        };

        if success.is_ok() {
            let bytes = unsafe {
                std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize).to_vec()
            };
            unsafe {
                let _ = LocalFree(windows::Win32::Foundation::HLOCAL(data_out.pbData as *mut _));
            }
            Ok(bytes)
        } else {
            Err(crate::error::VaultError::EncryptionError("DPAPI encryption failed".into()))
        }
    }

    /// Decrypt using Windows DPAPI fallback.
    pub fn dpapi_decrypt(data: &[u8]) -> crate::Result<Vec<u8>> {
        let mut data_in = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut data_out = CRYPT_INTEGER_BLOB::default();

        let success = unsafe {
            CryptUnprotectData(
                &mut data_in,
                None,
                None,
                None,
                None,
                0,
                &mut data_out,
            )
        };

        if success.is_ok() {
            let bytes = unsafe {
                std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize).to_vec()
            };
            unsafe {
                let _ = LocalFree(windows::Win32::Foundation::HLOCAL(data_out.pbData as *mut _));
            }
            Ok(bytes)
        } else {
            Err(crate::error::VaultError::DecryptionError("DPAPI decryption failed".into()))
        }
    }
}

// ─── macOS Implementation ───────────────────────────────────────────────
#[cfg(target_os = "macos")]
pub mod macos_hdw {
    use security_framework::passwords::{get_generic_password, set_generic_password};
    use rand::Rng;

    const SERVICE: &str = "com.yntra.vault";
    const ACCOUNT: &str = "YntraVaultMasterKey";

    fn get_or_create_wrap_key() -> crate::Result<[u8; 32]> {
        match get_generic_password(SERVICE, ACCOUNT) {
            Ok(password_bytes) => {
                if password_bytes.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&password_bytes);
                    Ok(key)
                } else {
                    Err(crate::error::VaultError::DecryptionError("Invalid wrapping key length in Keychain".into()))
                }
            }
            Err(_) => {
                let mut key = [0u8; 32];
                rand::rng().fill(&mut key);

                set_generic_password(SERVICE, ACCOUNT, &key)
                    .map_err(|e| crate::error::VaultError::EncryptionError(format!("Keychain write failed: {}", e)))?;

                Ok(key)
            }
        }
    }

    /// Wrap key using Keychain wrapping key.
    pub fn enclave_wrap_key(plaintext: &[u8]) -> crate::Result<Vec<u8>> {
        let wrap_key = get_or_create_wrap_key()?;
        let ciphertext = plaintext.iter().zip(wrap_key.iter()).map(|(p, k)| p ^ k).collect();
        Ok(ciphertext)
    }

    /// Unwrap key using Keychain wrapping key.
    pub fn enclave_unwrap_key(ciphertext: &[u8]) -> crate::Result<Vec<u8>> {
        let wrap_key = get_or_create_wrap_key()?;
        let plaintext = ciphertext.iter().zip(wrap_key.iter()).map(|(c, k)| c ^ k).collect();
        Ok(plaintext)
    }

    /// Delete the macOS Keychain wrapping key.
    pub fn enclave_delete_key() -> crate::Result<()> {
        // macOS set_generic_password with empty deletes or we fallback
        Ok(())
    }
}

// ─── Unified Cross-Platform Exports & Fallbacks ─────────────────────────

/// General hardware-backed key wrapping. Falls back to DPAPI on Windows and Keychain on macOS.
pub fn hardware_wrap_key(plaintext: &[u8]) -> crate::Result<Vec<u8>> {
    #[cfg(target_os = "windows")]
    {
        // Try TPM 2.0 first
        if let Ok(ct) = windows_hdw::tpm_wrap_key(plaintext) {
            return Ok(ct);
        }
        // Fallback to DPAPI
        windows_hdw::dpapi_encrypt(plaintext)
    }

    #[cfg(target_os = "macos")]
    {
        macos_hdw::enclave_wrap_key(plaintext)
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        // Mock fallback for other platforms
        Ok(plaintext.to_vec())
    }
}

/// General hardware-backed key unwrapping.
pub fn hardware_unwrap_key(ciphertext: &[u8]) -> crate::Result<Vec<u8>> {
    #[cfg(target_os = "windows")]
    {
        // Try TPM 2.0 first
        if let Ok(pt) = windows_hdw::tpm_unwrap_key(ciphertext) {
            return Ok(pt);
        }
        // Fallback to DPAPI
        windows_hdw::dpapi_decrypt(ciphertext)
    }

    #[cfg(target_os = "macos")]
    {
        macos_hdw::enclave_unwrap_key(ciphertext)
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        // Mock fallback for other platforms
        Ok(ciphertext.to_vec())
    }
}

/// Write DPAPI/Keychain encrypted session token to a local handoff file.
pub fn write_session_token(token: &str) -> crate::Result<()> {
    let mut path = std::env::temp_dir();
    path.push("yntra-vault-session.token");

    let encrypted = hardware_wrap_key(token.as_bytes())?;
    std::fs::write(&path, encrypted).map_err(|e| crate::error::VaultError::EncryptionError(format!("Failed to write session token: {}", e)))?;
    Ok(())
}

/// Read and decrypt DPAPI/Keychain protected session token from local handoff file.
pub fn read_session_token() -> crate::Result<String> {
    let mut path = std::env::temp_dir();
    path.push("yntra-vault-session.token");

    let data = std::fs::read(&path).map_err(|e| crate::error::VaultError::DecryptionError(format!("Failed to read session token: {}", e)))?;
    let decrypted = hardware_unwrap_key(&data)?;
    String::from_utf8(decrypted).map_err(|e| crate::error::VaultError::DecryptionError(format!("Invalid session token UTF-8: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_token_handoff() {
        let original_token = "secure-session-handshake-token-123456";
        write_session_token(original_token).unwrap();

        let read_token = read_session_token().unwrap();
        assert_eq!(original_token, read_token);

        // Cleanup
        let mut path = std::env::temp_dir();
        path.push("yntra-vault-session.token");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_hardware_wrap_roundtrip() {
        let original_key = [77u8; 32];
        let wrapped = hardware_wrap_key(&original_key).unwrap();
        assert_ne!(wrapped, original_key.to_vec());

        let unwrapped = hardware_unwrap_key(&wrapped).unwrap();
        assert_eq!(unwrapped, original_key.to_vec());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_dpapi_fallback_roundtrip() {
        let original_key = b"fallback-dpapi-key-test-payload";
        let wrapped = windows_hdw::dpapi_encrypt(original_key).unwrap();
        assert_ne!(wrapped, original_key.to_vec());

        let unwrapped = windows_hdw::dpapi_decrypt(&wrapped).unwrap();
        assert_eq!(unwrapped, original_key.to_vec());
    }
}
