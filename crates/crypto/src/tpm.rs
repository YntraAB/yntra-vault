//! Platform-specific hardware binding (TPM 2.0, Secure Enclave / Keychain, DPAPI).

// ─── Windows Implementation ─────────────────────────────────────────────
#[cfg(target_os = "windows")]
pub mod windows_hdw {
    use zeroize::Zeroize;
    use windows::Win32::Security::Cryptography::{
        NCryptOpenStorageProvider, NCryptCreatePersistedKey, NCryptFinalizeKey,
        NCryptOpenKey, NCryptEncrypt, NCryptDecrypt, MS_PLATFORM_KEY_STORAGE_PROVIDER,
        NCRYPT_PAD_PKCS1_FLAG, NCRYPT_PAD_OAEP_FLAG, NCRYPT_PROV_HANDLE, NCRYPT_KEY_HANDLE,
        CERT_KEY_SPEC, NCRYPT_FLAGS, NCRYPT_RSA_ALGORITHM, CRYPT_INTEGER_BLOB,
    };
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{NTE_BAD_KEYSET, LocalFree};
    use windows::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData};

    const KEY_NAME: &str = "YntraVaultMasterKey\0";
    pub const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;

    /// Wrap key using Windows TPM 2.0 (CNG/NCrypt) with OAEP/PKCS1 fallback.
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

            // Two-stage fallback: attempt full OAEP (size query + encrypt), then fallback to PKCS#1 v1.5
            let try_encrypt = |pad_flag: NCRYPT_FLAGS| -> windows::core::Result<Vec<u8>> {
                let mut cb_output = 0;
                NCryptEncrypt(
                    key,
                    Some(plaintext),
                    None,
                    None,
                    &mut cb_output,
                    pad_flag,
                )?;
                let mut buf = vec![0u8; cb_output as usize];
                NCryptEncrypt(
                    key,
                    Some(plaintext),
                    None,
                    Some(&mut buf),
                    &mut cb_output,
                    pad_flag,
                )?;
                buf.truncate(cb_output as usize);
                Ok(buf)
            };

            let res = try_encrypt(NCRYPT_PAD_OAEP_FLAG)
                .or_else(|_| try_encrypt(NCRYPT_PAD_PKCS1_FLAG));

            // Free handles
            let _ = windows::Win32::Security::Cryptography::NCryptFreeObject(windows::Win32::Security::Cryptography::NCRYPT_HANDLE(key.0));
            let _ = windows::Win32::Security::Cryptography::NCryptFreeObject(windows::Win32::Security::Cryptography::NCRYPT_HANDLE(prov.0));

            res.map_err(|e| crate::error::VaultError::EncryptionError(format!("TPM encryption failed: {:?}", e)))
        }
    }

    /// Unwrap key using Windows TPM 2.0 (CNG/NCrypt) with OAEP/PKCS1 fallback.
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

            // Two-stage fallback: attempt full OAEP (size query + decrypt), then fallback to PKCS#1 v1.5
            let try_decrypt = |pad_flag: NCRYPT_FLAGS| -> windows::core::Result<Vec<u8>> {
                let mut cb_output = 0;
                NCryptDecrypt(
                    key,
                    Some(ciphertext),
                    None,
                    None,
                    &mut cb_output,
                    pad_flag,
                )?;
                let mut buf = vec![0u8; cb_output as usize];
                NCryptDecrypt(
                    key,
                    Some(ciphertext),
                    None,
                    Some(&mut buf),
                    &mut cb_output,
                    pad_flag,
                )?;
                buf.truncate(cb_output as usize);
                Ok(buf)
            };

            let res = try_decrypt(NCRYPT_PAD_OAEP_FLAG)
                .or_else(|_| try_decrypt(NCRYPT_PAD_PKCS1_FLAG));

            let _ = windows::Win32::Security::Cryptography::NCryptFreeObject(windows::Win32::Security::Cryptography::NCRYPT_HANDLE(key.0));
            let _ = windows::Win32::Security::Cryptography::NCryptFreeObject(windows::Win32::Security::Cryptography::NCRYPT_HANDLE(prov.0));

            res.map_err(|e| crate::error::VaultError::DecryptionError(format!("TPM decryption failed: {:?}", e)))
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

    /// Derive deterministic, installation/user-bound entropy for App-Bound DPAPI protection.
    ///
    /// Binds encryption to Yntra Vault's unique application identity and user profile,
    /// preventing generic infostealers from calling CryptUnprotectData with NULL entropy.
    pub fn get_app_bound_entropy() -> [u8; 32] {
        let mut hasher = blake3::Hasher::new_derive_key("yntra-vault-app-bound-dpapi-entropy-v1");
        if let Ok(appdata) = std::env::var("LOCALAPPDATA") {
            let normalized = appdata.replace('/', "\\").trim_end_matches('\\').to_lowercase();
            hasher.update(normalized.as_bytes());
        } else if let Ok(user) = std::env::var("USERNAME") {
            hasher.update(user.trim().to_lowercase().as_bytes());
        } else if let Ok(profile) = std::env::var("USERPROFILE") {
            let normalized = profile.replace('/', "\\").trim_end_matches('\\').to_lowercase();
            hasher.update(normalized.as_bytes());
        }
        hasher.update(b"yntra-vault-local-secret-isolation-salt");
        *hasher.finalize().as_bytes()
    }

    /// Encrypt using Windows App-Bound DPAPI with domain-separated entropy and UI forbidden.
    pub fn dpapi_encrypt(data: &[u8]) -> crate::Result<Vec<u8>> {
        let mut entropy = get_app_bound_entropy();
        let data_in = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let entropy_blob = CRYPT_INTEGER_BLOB {
            cbData: entropy.len() as u32,
            pbData: entropy.as_mut_ptr(),
        };
        let mut data_out = CRYPT_INTEGER_BLOB::default();

        let success = unsafe {
            CryptProtectData(
                &data_in,
                None,
                Some(&entropy_blob),
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut data_out,
            )
        };
        entropy.zeroize();

        if success.is_ok() {
            let bytes = unsafe {
                std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize).to_vec()
            };
            unsafe {
                let _ = LocalFree(windows::Win32::Foundation::HLOCAL(data_out.pbData as *mut _));
            }
            Ok(bytes)
        } else {
            Err(crate::error::VaultError::EncryptionError("App-Bound DPAPI encryption failed".into()))
        }
    }

    /// Decrypt using Windows App-Bound DPAPI with domain-separated entropy,
    /// falling back to legacy NULL entropy for seamless backward compatibility.
    pub fn dpapi_decrypt(data: &[u8]) -> crate::Result<Vec<u8>> {
        let mut entropy = get_app_bound_entropy();
        let data_in = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let entropy_blob = CRYPT_INTEGER_BLOB {
            cbData: entropy.len() as u32,
            pbData: entropy.as_mut_ptr(),
        };
        let mut data_out = CRYPT_INTEGER_BLOB::default();

        // 1. Primary: Attempt App-Bound unprotect with entropy
        let success = unsafe {
            CryptUnprotectData(
                &data_in,
                None,
                Some(&entropy_blob),
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut data_out,
            )
        };
        entropy.zeroize();

        if success.is_ok() {
            let bytes = unsafe {
                std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize).to_vec()
            };
            unsafe {
                let _ = LocalFree(windows::Win32::Foundation::HLOCAL(data_out.pbData as *mut _));
            }
            return Ok(bytes);
        }

        // 2. Fallback: Attempt legacy unprotect without entropy for backward compatibility
        let mut data_out_legacy = CRYPT_INTEGER_BLOB::default();
        let legacy_success = unsafe {
            CryptUnprotectData(
                &data_in,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut data_out_legacy,
            )
        };

        if legacy_success.is_ok() {
            let bytes = unsafe {
                std::slice::from_raw_parts(data_out_legacy.pbData, data_out_legacy.cbData as usize).to_vec()
            };
            unsafe {
                let _ = LocalFree(windows::Win32::Foundation::HLOCAL(data_out_legacy.pbData as *mut _));
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
    use security_framework::passwords::{get_generic_password, set_generic_password, delete_generic_password};
    use rand::Rng;
    use chacha20poly1305::{XChaCha20Poly1305, aead::{Aead, KeyInit}, XNonce};

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

    /// Wrap key using Keychain-stored key with XChaCha20-Poly1305.
    pub fn enclave_wrap_key(plaintext: &[u8]) -> crate::Result<Vec<u8>> {
        let wrap_key = get_or_create_wrap_key()?;
        let cipher = XChaCha20Poly1305::new_from_slice(&wrap_key)
            .map_err(|e| crate::error::VaultError::EncryptionError(format!("Wrap cipher init: {}", e)))?;

        let mut nonce_bytes = [0u8; 24];
        rand::rng().fill(&mut nonce_bytes);
        let nonce = XNonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, plaintext)
            .map_err(|e| crate::error::VaultError::EncryptionError(format!("Wrap encrypt: {}", e)))?;

        // Prepend nonce to ciphertext
        let mut result = Vec::with_capacity(24 + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    /// Unwrap key using Keychain-stored key with XChaCha20-Poly1305.
    pub fn enclave_unwrap_key(data: &[u8]) -> crate::Result<Vec<u8>> {
        if data.len() < 24 {
            return Err(crate::error::VaultError::DecryptionError("Wrapped data too short".into()));
        }
        let wrap_key = get_or_create_wrap_key()?;
        let cipher = XChaCha20Poly1305::new_from_slice(&wrap_key)
            .map_err(|e| crate::error::VaultError::DecryptionError(format!("Unwrap cipher init: {}", e)))?;

        let nonce = XNonce::from_slice(&data[..24]);
        cipher.decrypt(nonce, &data[24..])
            .map_err(|_| crate::error::VaultError::DecryptionError("Unwrap decrypt failed".into()))
    }

    /// Delete the macOS Keychain wrapping key.
    pub fn enclave_delete_key() -> crate::Result<()> {
        let _ = delete_generic_password(SERVICE, ACCOUNT);
        Ok(())
    }
}

// ─── Linux File-Based Key Wrapping ──────────────────────────────────────
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn linux_get_or_create_wrap_key() -> crate::Result<[u8; 32]> {
    use rand::Rng;
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    let base_dir = std::env::var("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|_| {
            std::env::var("HOME").map(|h| std::path::PathBuf::from(h).join(".config"))
        })
        .or_else(|_| {
            // Android sandbox / private data directory fallbacks
            let android_candidates = [
                "/data/user/0/com.yntravault.app",
                "/data/data/com.yntravault.app",
                "/data/user/0/com.yntra.vault",
                "/data/data/com.yntra.vault",
            ];
            for path in &android_candidates {
                let pb = std::path::PathBuf::from(path);
                if pb.exists() {
                    let files_dir = pb.join("files");
                    let _ = std::fs::create_dir_all(&files_dir);
                    return Ok(files_dir);
                }
            }
            if let Ok(data_dir) = std::env::var("ANDROID_DATA") {
                let pb = std::path::PathBuf::from(data_dir).join("data/com.yntravault.app");
                if pb.exists() {
                    let files_dir = pb.join("files");
                    let _ = std::fs::create_dir_all(&files_dir);
                    return Ok(files_dir);
                }
            }
            // Security Invariant 25: Never fallback to world-writable /tmp paths
            Err(())
        })
        .map_err(|_: ()| {
            crate::error::VaultError::TpmError(
                "Neither XDG_CONFIG_HOME, HOME, nor valid private app sandbox is accessible".into(),
            )
        })?;

    let key_dir = base_dir.join("yntra-vault");
    std::fs::create_dir_all(&key_dir)
        .map_err(|e| crate::error::VaultError::TpmError(format!("Failed to create wrap key directory: {e}")))?;

    let dir_perms = std::fs::Permissions::from_mode(0o700);
    let _ = std::fs::set_permissions(&key_dir, dir_perms);

    let key_path = key_dir.join("wrap-key.bin");

    // 1. Try reading existing wrap key directly without TOCTOU exists() check
    match std::fs::read(&key_path) {
        Ok(data) => {
            if data.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&data);
                return Ok(key);
            }
            return Err(crate::error::VaultError::DecryptionError("Corrupted wrap key file".into()));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Key file does not exist yet; proceed to atomic creation below
        }
        Err(e) => {
            return Err(crate::error::VaultError::DecryptionError(format!("Read wrap key: {}", e)));
        }
    }

    // 2. Generate new key and create file atomically with restricted permissions (0600)
    let mut key = [0u8; 32];
    rand::rng().fill(&mut key);

    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&key_path)
    {
        Ok(mut file) => {
            file.write_all(&key)
                .map_err(|e| crate::error::VaultError::EncryptionError(format!("Write wrap key: {}", e)))?;
            file.flush()
                .map_err(|e| crate::error::VaultError::EncryptionError(format!("Flush wrap key: {}", e)))?;
            Ok(key)
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let data = std::fs::read(&key_path)
                .map_err(|e| crate::error::VaultError::DecryptionError(format!("Read wrap key: {}", e)))?;
            if data.len() == 32 {
                let mut existing_key = [0u8; 32];
                existing_key.copy_from_slice(&data);
                Ok(existing_key)
            } else {
                Err(crate::error::VaultError::DecryptionError("Corrupted wrap key file".into()))
            }
        }
        Err(e) => Err(crate::error::VaultError::EncryptionError(format!("Open wrap key with 0600: {}", e))),
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn linux_wrap_key(plaintext: &[u8]) -> crate::Result<Vec<u8>> {
    use chacha20poly1305::{XChaCha20Poly1305, aead::{Aead, KeyInit}, XNonce};
    use rand::Rng;

    let wrap_key = linux_get_or_create_wrap_key()?;
    let cipher = XChaCha20Poly1305::new_from_slice(&wrap_key)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("Linux wrap init: {}", e)))?;

    let mut nonce_bytes = [0u8; 24];
    rand::rng().fill(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);

    let ct = cipher.encrypt(nonce, plaintext)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("Linux wrap encrypt: {}", e)))?;

    let mut result = Vec::with_capacity(24 + ct.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ct);
    Ok(result)
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn linux_unwrap_key(data: &[u8]) -> crate::Result<Vec<u8>> {
    use chacha20poly1305::{XChaCha20Poly1305, aead::{Aead, KeyInit}, XNonce};

    if data.len() < 24 {
        return Err(crate::error::VaultError::DecryptionError("Wrapped data too short".into()));
    }
    let wrap_key = linux_get_or_create_wrap_key()?;
    let cipher = XChaCha20Poly1305::new_from_slice(&wrap_key)
        .map_err(|e| crate::error::VaultError::DecryptionError(format!("Linux unwrap init: {}", e)))?;

    let nonce = XNonce::from_slice(&data[..24]);
    cipher.decrypt(nonce, &data[24..])
        .map_err(|_| crate::error::VaultError::DecryptionError("Linux unwrap decrypt failed".into()))
}

// ─── Unified Cross-Platform Exports & Fallbacks ─────────────────────────

pub const ENVELOPE_MAGIC_TPM: &[u8; 4] = b"YTPM";
pub const ENVELOPE_MAGIC_DPAPI_APP_BOUND: &[u8; 4] = b"YDPB";

/// General hardware-backed key wrapping. Falls back to App-Bound DPAPI on Windows and Keychain on macOS.
pub fn hardware_wrap_key(plaintext: &[u8]) -> crate::Result<Vec<u8>> {
    #[cfg(target_os = "windows")]
    {
        // 1. Primary: Windows CNG TPM 2.0 hardware key
        if let Ok(ct) = windows_hdw::tpm_wrap_key(plaintext) {
            let mut envelope = Vec::with_capacity(4 + ct.len());
            envelope.extend_from_slice(ENVELOPE_MAGIC_TPM);
            envelope.extend_from_slice(&ct);
            return Ok(envelope);
        }
        // 2. Secondary Fallback: App-Bound DPAPI with domain-separated entropy
        let ct = windows_hdw::dpapi_encrypt(plaintext)?;
        let mut envelope = Vec::with_capacity(4 + ct.len());
        envelope.extend_from_slice(ENVELOPE_MAGIC_DPAPI_APP_BOUND);
        envelope.extend_from_slice(&ct);
        Ok(envelope)
    }

    #[cfg(target_os = "macos")]
    {
        macos_hdw::enclave_wrap_key(plaintext)
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        linux_wrap_key(plaintext)
    }
}

/// General hardware-backed key unwrapping.
pub fn hardware_unwrap_key(ciphertext: &[u8]) -> crate::Result<Vec<u8>> {
    #[cfg(target_os = "windows")]
    {
        if ciphertext.len() < 4 {
            return Err(crate::error::VaultError::DecryptionError("Wrapped key data too short".into()));
        }

        if ciphertext.starts_with(ENVELOPE_MAGIC_TPM) {
            return windows_hdw::tpm_unwrap_key(&ciphertext[4..]);
        }
        if ciphertext.starts_with(ENVELOPE_MAGIC_DPAPI_APP_BOUND) {
            return windows_hdw::dpapi_decrypt(&ciphertext[4..]);
        }

        // Legacy un-tagged envelope fallback (prior to envelope tagging):
        // DPAPI blobs always start with dwVersion = 0x00000001 (little-endian [0x01, 0x00, 0x00, 0x00])
        if ciphertext.len() >= 4 && ciphertext[..4] == [1, 0, 0, 0] {
            return windows_hdw::dpapi_decrypt(ciphertext);
        }

        if let Ok(pt) = windows_hdw::tpm_unwrap_key(ciphertext) {
            return Ok(pt);
        }
        windows_hdw::dpapi_decrypt(ciphertext)
    }

    #[cfg(target_os = "macos")]
    {
        macos_hdw::enclave_unwrap_key(ciphertext)
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        linux_unwrap_key(ciphertext)
    }
}

pub fn get_session_token_path() -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir());
        let dir = base.join("Yntra Vault");
        let _ = std::fs::create_dir_all(&dir);
        dir.join("session.token")
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
            if !runtime_dir.is_empty() {
                let mut path = std::path::PathBuf::from(runtime_dir);
                path.push("yntra-vault-session.token");
                return path;
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            let mut path = std::path::PathBuf::from(home);
            path.push(".local/share/yntra");
            let _ = std::fs::create_dir_all(&path);
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700));
            path.push("yntra-vault-session.token");
            return path;
        }
        let mut path = std::env::temp_dir();
        path.push("yntra-vault-session.token");
        path
    }
}

/// Write DPAPI/Keychain encrypted session token to a local handoff file.
pub fn write_session_token(token: &str) -> crate::Result<()> {
    let path = get_session_token_path();

    let encrypted = hardware_wrap_key(token.as_bytes())?;
    std::fs::write(&path, encrypted).map_err(|e| crate::error::VaultError::EncryptionError(format!("Failed to write session token: {}", e)))?;

    // Restrict file permissions on Unix
    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        let _ = std::fs::set_permissions(&path, perms);
    }

    Ok(())
}

/// Read and decrypt DPAPI/Keychain protected session token from local handoff file.
pub fn read_session_token() -> crate::Result<String> {
    let path = get_session_token_path();

    let data = std::fs::read(&path).map_err(|e| crate::error::VaultError::DecryptionError(format!("Failed to read session token: {}", e)))?;
    let decrypted = hardware_unwrap_key(&data)?;
    String::from_utf8(decrypted).map_err(|e| crate::error::VaultError::DecryptionError(format!("Invalid session token UTF-8: {}", e)))
}

/// Clear and remove local handoff session token file.
pub fn clear_session_token() -> crate::Result<()> {
    let path = get_session_token_path();
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
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
        clear_session_token().unwrap();
        assert!(read_session_token().is_err());
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

    #[cfg(target_os = "windows")]
    #[test]
    fn test_app_bound_dpapi_blocks_blind_unprotect() {
        use windows::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};

        let secret = b"super-sensitive-biometric-kek-secret-key";
        let encrypted = windows_hdw::dpapi_encrypt(secret).unwrap();

        // Simulate an infostealer running in userland attempting blind CryptUnprotectData without entropy
        let data_in = CRYPT_INTEGER_BLOB {
            cbData: encrypted.len() as u32,
            pbData: encrypted.as_ptr() as *mut u8,
        };
        let mut data_out = CRYPT_INTEGER_BLOB::default();

        let blind_decrypt_result = unsafe {
            CryptUnprotectData(
                &data_in,
                None,
                None, // Infostealers pass NULL entropy
                None,
                None,
                0,
                &mut data_out,
            )
        };

        // Blind unprotect MUST fail because App-Bound entropy is required!
        assert!(
            blind_decrypt_result.is_err(),
            "Infostealer blind CryptUnprotectData without App-Bound entropy should be REJECTED!"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_legacy_dpapi_backward_compatibility() {
        use windows::Win32::Security::Cryptography::{CryptProtectData, CRYPT_INTEGER_BLOB};
        use windows::Win32::Foundation::LocalFree;

        let legacy_secret = b"legacy-v1-unbound-secret-data";

        // Create a legacy DPAPI blob without entropy (as created by earlier versions)
        let data_in = CRYPT_INTEGER_BLOB {
            cbData: legacy_secret.len() as u32,
            pbData: legacy_secret.as_ptr() as *mut u8,
        };
        let mut data_out = CRYPT_INTEGER_BLOB::default();

        let legacy_blob = unsafe {
            let res = CryptProtectData(&data_in, None, None, None, None, 0, &mut data_out);
            assert!(res.is_ok(), "Legacy protect should succeed");
            let bytes = std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize).to_vec();
            let _ = LocalFree(windows::Win32::Foundation::HLOCAL(data_out.pbData as *mut _));
            bytes
        };

        // Ensure modern dpapi_decrypt seamlessly decrypts legacy blobs via fallback
        let decrypted = windows_hdw::dpapi_decrypt(&legacy_blob).unwrap();
        assert_eq!(decrypted, legacy_secret.to_vec());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_legacy_untagged_envelope_backward_compatibility() {
        use windows::Win32::Security::Cryptography::{CryptProtectData, CRYPT_INTEGER_BLOB};
        use windows::Win32::Foundation::LocalFree;

        let legacy_secret = b"legacy-untagged-envelope-payload";

        let data_in = CRYPT_INTEGER_BLOB {
            cbData: legacy_secret.len() as u32,
            pbData: legacy_secret.as_ptr() as *mut u8,
        };
        let mut data_out = CRYPT_INTEGER_BLOB::default();

        let raw_dpapi_blob = unsafe {
            let res = CryptProtectData(&data_in, None, None, None, None, 0, &mut data_out);
            assert!(res.is_ok());
            let bytes = std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize).to_vec();
            let _ = LocalFree(windows::Win32::Foundation::HLOCAL(data_out.pbData as *mut _));
            bytes
        };

        // Pass raw untagged DPAPI bytes to hardware_unwrap_key
        let unwrapped = hardware_unwrap_key(&raw_dpapi_blob).unwrap();
        assert_eq!(unwrapped, legacy_secret.to_vec());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_app_bound_entropy_normalization() {
        let entropy1 = windows_hdw::get_app_bound_entropy();
        let entropy2 = windows_hdw::get_app_bound_entropy();
        assert_eq!(entropy1, entropy2);
        assert_ne!(entropy1, [0u8; 32]);
    }
}
