//! Hardware 2FA / YubiKey (FIDO2 / CTAP2 / Challenge-Response) module
//!
//! Provides Hardware 2FA challenge-response authentication, key derivation binding,
//! and single-file .vdb embedded hardware key envelope wrapping.

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;
use rand::Rng;
use chacha20poly1305::{XChaCha20Poly1305, XNonce, aead::{Aead, KeyInit}};
use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::Sha256;

use crate::{SubKeys, MasterKey, derive_master_key};
use crate::error::VaultError;
use crate::mem::LockedBuffer;

type HmacSha1 = Hmac<Sha1>;
type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum Hardware2FaProtocol {
    /// YubiKey HMAC-SHA1 Challenge-Response (CTAP1 / USB HID slot 1/2)
    YubiKeyChallengeResponse,
    /// FIDO2 / CTAP2 HMAC-Secret (WebAuthn PRF extension)
    Fido2Ctap2HmacSecret,
}

impl std::fmt::Display for Hardware2FaProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Hardware2FaProtocol::YubiKeyChallengeResponse => write!(f, "YubiKey Challenge-Response (HMAC-SHA1)"),
            Hardware2FaProtocol::Fido2Ctap2HmacSecret => write!(f, "FIDO2 / CTAP2 (HMAC-Secret)"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HardwareKeyInfo {
    pub id: String,
    pub name: String,
    pub protocol: Hardware2FaProtocol,
    pub serial: Option<u32>,
    pub is_connected: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Hardware2FaInfo {
    pub available: bool,
    pub key_count: usize,
    pub supported_protocols: Vec<Hardware2FaProtocol>,
    pub connected_keys: Vec<HardwareKeyInfo>,
}

/// Single-file embedded Hardware 2FA envelope stored in .vdb vault header.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EmbeddedHardware2FaHeader {
    pub protocol: Hardware2FaProtocol,
    pub credential_id: Vec<u8>,
    pub challenge_salt: [u8; 32],
    pub nonce: [u8; 24],
    pub wrapped_kek: Vec<u8>,
    pub encrypted_subkeys: Vec<u8>,
    pub key_name: String,
}

/// Check hardware 2FA availability and return details on connected hardware keys.
pub fn check_hardware2fa_availability() -> Hardware2FaInfo {
    let connected = list_hardware_keys();
    let count = connected.len();
    Hardware2FaInfo {
        available: count > 0,
        key_count: count,
        supported_protocols: vec![
            Hardware2FaProtocol::YubiKeyChallengeResponse,
            Hardware2FaProtocol::Fido2Ctap2HmacSecret,
        ],
        connected_keys: connected,
    }
}

/// List connected YubiKeys and FIDO2 hardware authenticators.
pub fn list_hardware_keys() -> Vec<HardwareKeyInfo> {
    #[cfg(target_os = "windows")]
    {
        win_detection::detect_hardware_keys()
    }
    #[cfg(not(target_os = "windows"))]
    {
        non_win_detection::detect_hardware_keys()
    }
}

#[cfg(target_os = "windows")]
mod win_detection {
    use super::*;
    use windows::Win32::System::Registry::{
        RegOpenKeyExW, RegQueryValueExW, RegCloseKey,
        HKEY_LOCAL_MACHINE, KEY_READ,
    };
    use windows::core::PCWSTR;

    pub fn detect_hardware_keys() -> Vec<HardwareKeyInfo> {
        let mut keys = Vec::new();
        let subkey: Vec<u16> = "SYSTEM\\CurrentControlSet\\Services\\HidUsb\\Enum\0"
            .encode_utf16()
            .collect();

        unsafe {
            let mut hkey = windows::Win32::System::Registry::HKEY::default();
            let status = RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR::from_raw(subkey.as_ptr()),
                0,
                KEY_READ,
                &mut hkey,
            );

            if status.is_err() {
                return keys;
            }

            let mut count: u32 = 0;
            let mut count_size = std::mem::size_of::<u32>() as u32;
            let count_name: Vec<u16> = "Count\0".encode_utf16().collect();
            let query_status = RegQueryValueExW(
                hkey,
                PCWSTR::from_raw(count_name.as_ptr()),
                None,
                None,
                Some(&mut count as *mut u32 as *mut u8),
                Some(&mut count_size),
            );

            if query_status.is_ok() {
                for i in 0..count {
                    let index_name: Vec<u16> = format!("{}\0", i).encode_utf16().collect();
                    let mut buf = [0u16; 512];
                    let mut buf_size = (buf.len() * 2) as u32;
                    let val_status = RegQueryValueExW(
                        hkey,
                        PCWSTR::from_raw(index_name.as_ptr()),
                        None,
                        None,
                        Some(buf.as_mut_ptr() as *mut u8),
                        Some(&mut buf_size),
                    );

                    if val_status.is_ok() {
                        let len = (buf_size / 2) as usize;
                        let len = if len > 0 && buf[len - 1] == 0 { len - 1 } else { len };
                        if let Ok(dev_path) = String::from_utf16(&buf[..len]) {
                            if let Some(key_info) = parse_device_instance(&dev_path) {
                                if !keys.iter().any(|existing: &HardwareKeyInfo| existing.id == key_info.id) {
                                    keys.push(key_info);
                                }
                            }
                        }
                    }
                }
            }

            let _ = RegCloseKey(hkey);
        }

        keys
    }

    fn parse_device_instance(dev_path: &str) -> Option<HardwareKeyInfo> {
        let upper = dev_path.to_uppercase();
        if !upper.contains("VID_") {
            return None;
        }

        // Check for Yubico (VID_1050)
        if upper.contains("VID_1050") {
            let name = if upper.contains("PID_0407") || upper.contains("PID_0406") || upper.contains("PID_0405")
                || upper.contains("PID_0404") || upper.contains("PID_0403") || upper.contains("PID_0402") || upper.contains("PID_0401") {
                "YubiKey 5 Series (Challenge-Response)".to_string()
            } else if upper.contains("PID_0410") {
                "YubiKey 5 FIPS (Challenge-Response)".to_string()
            } else if upper.contains("PID_0110") || upper.contains("PID_0111") || upper.contains("PID_0112")
                || upper.contains("PID_0114") || upper.contains("PID_0116") {
                "YubiKey NEO (Challenge-Response)".to_string()
            } else if upper.contains("PID_0020") || upper.contains("PID_0120") {
                "Security Key by Yubico".to_string()
            } else {
                "YubiKey Hardware Key".to_string()
            };

            let serial = parse_serial(&upper);

            return Some(HardwareKeyInfo {
                id: format!("yubikey-{}", dev_path.replace('\\', "_")),
                name,
                protocol: Hardware2FaProtocol::YubiKeyChallengeResponse,
                serial,
                is_connected: true,
            });
        }

        // Check for known FIDO2 / CTAP2 authenticators
        let fido_vendor = if upper.contains("VID_096E") {
            Some("Feitian ePass FIDO2")
        } else if upper.contains("VID_18D1") {
            Some("Google Titan Security Key")
        } else if upper.contains("VID_1209") || upper.contains("VID_0483") {
            Some("SoloKeys FIDO2 Security Key")
        } else if upper.contains("VID_20A0") {
            Some("Nitrokey FIDO2")
        } else if upper.contains("VID_32A3") {
            Some("TrustKey FIDO2 Security Key")
        } else if upper.contains("VID_2CCF") {
            Some("HyperFIDO Security Key")
        } else {
            None
        };

        if let Some(vendor_name) = fido_vendor {
            let serial = parse_serial(&upper);
            return Some(HardwareKeyInfo {
                id: format!("fido2-{}", dev_path.replace('\\', "_")),
                name: format!("{} (HMAC-Secret)", vendor_name),
                protocol: Hardware2FaProtocol::Fido2Ctap2HmacSecret,
                serial,
                is_connected: true,
            });
        }

        None
    }

    fn parse_serial(upper: &str) -> Option<u32> {
        let parts: Vec<&str> = upper.split('\\').collect();
        if let Some(last) = parts.last() {
            if let Ok(num) = last.parse::<u32>() {
                return Some(num);
            }
        }
        None
    }
}

#[cfg(not(target_os = "windows"))]
mod non_win_detection {
    use super::*;

    pub fn detect_hardware_keys() -> Vec<HardwareKeyInfo> {
        #[cfg(target_os = "linux")]
        {
            if let Ok(entries) = std::fs::read_dir("/sys/bus/usb/devices") {
                let mut keys = Vec::new();
                for entry in entries.flatten() {
                    let path = entry.path();
                    let id_vendor = std::fs::read_to_string(path.join("idVendor")).unwrap_or_default().trim().to_lowercase();
                    let id_product = std::fs::read_to_string(path.join("idProduct")).unwrap_or_default().trim().to_lowercase();
                    let product = std::fs::read_to_string(path.join("product")).unwrap_or_default().trim().to_string();
                    let serial_str = std::fs::read_to_string(path.join("serial")).unwrap_or_default().trim().to_string();
                    let serial = serial_str.parse::<u32>().ok();

                    if id_vendor == "1050" {
                        keys.push(HardwareKeyInfo {
                            id: format!("yubikey-{}-{}", id_product, serial_str),
                            name: if !product.is_empty() { product } else { "YubiKey 5 Series".to_string() },
                            protocol: Hardware2FaProtocol::YubiKeyChallengeResponse,
                            serial,
                            is_connected: true,
                        });
                    }
                }
                return keys;
            }
        }

        Vec::new()
    }
}

static MOCK_HARDWARE_2FA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Enable or disable mock hardware 2FA challenge-response for integration testing.
pub fn set_hardware2fa_mock(enabled: bool) {
    MOCK_HARDWARE_2FA.store(enabled, std::sync::atomic::Ordering::Relaxed);
}

fn is_hardware2fa_mock_enabled() -> bool {
    cfg!(test)
        || (cfg!(debug_assertions) && MOCK_HARDWARE_2FA.load(std::sync::atomic::Ordering::Relaxed))
        || (cfg!(debug_assertions) && std::env::var("YNTRA_TEST_MODE").is_ok())
}

/// Perform a hardware challenge-response on a connected hardware key.
pub fn perform_hardware2fa_challenge(
    protocol: Hardware2FaProtocol,
    challenge: &[u8],
) -> crate::Result<Vec<u8>> {
    perform_hardware2fa_challenge_with_cred(protocol, challenge, None)
}

/// Perform a hardware challenge-response on a connected hardware key with optional credential ID.
pub fn perform_hardware2fa_challenge_with_cred(
    protocol: Hardware2FaProtocol,
    challenge: &[u8],
    credential_id: Option<&[u8]>,
) -> crate::Result<Vec<u8>> {
    if challenge.is_empty() {
        return Err(VaultError::Hardware2FaAuthFailed(
            "Challenge parameter cannot be empty".into(),
        ));
    }

    if is_hardware2fa_mock_enabled() {
        return mock_test_challenge(protocol, challenge);
    }

    #[cfg(target_os = "windows")]
    {
        match protocol {
            Hardware2FaProtocol::YubiKeyChallengeResponse => {
                win_yubikey_hid::perform_yubikey_challenge(challenge)
            }
            Hardware2FaProtocol::Fido2Ctap2HmacSecret => {
                win_webauthn::get_assertion_hmac_secret(challenge, credential_id)
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let connected = list_hardware_keys();
        let key = connected.iter().find(|k| k.protocol == protocol);
        if key.is_none() {
            return Err(VaultError::Hardware2FaNotAvailable(format!(
                "No compatible {} security key detected. Please connect your hardware key.",
                protocol
            )));
        }

        Err(VaultError::Hardware2FaNotAvailable(format!(
            "Hardware 2FA protocol {} is currently only supported on Windows.",
            protocol
        )))
    }
}

#[cfg(target_os = "windows")]
mod win_webauthn {
    use super::*;
    use windows::core::{PCWSTR, s};
    use windows::Win32::Foundation::{FreeLibrary, HMODULE, HWND};
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

    #[repr(C)]
    struct WebAuthnClientDataHash {
        cb_client_data_hash: u32,
        pb_client_data_hash: *const u8,
    }

    #[repr(C)]
    struct WebAuthnHmacSecretSalt {
        cb_first: u32,
        pb_first: *const u8,
        cb_second: u32,
        pb_second: *const u8,
    }

    #[repr(C)]
    struct WebAuthnCredWithHmacSecretSalt {
        cb_cred_id: u32,
        pb_cred_id: *const u8,
        p_hmac_secret_salt: *mut WebAuthnHmacSecretSalt,
    }

    #[repr(C)]
    struct WebAuthnHmacSecretSaltValues {
        p_global_hmac_salt: *mut WebAuthnHmacSecretSalt,
        c_cred_with_hmac_secret_salt_list: u32,
        p_cred_with_hmac_secret_salt_list: *mut WebAuthnCredWithHmacSecretSalt,
    }

    #[repr(C)]
    struct WebAuthnCredentials {
        c_credentials: u32,
        p_credentials: *mut std::ffi::c_void,
    }

    #[repr(C)]
    struct WebAuthnExtensions {
        c_extensions: u32,
        p_extensions: *mut std::ffi::c_void,
    }

    #[repr(C)]
    struct WebAuthnCredential {
        dw_version: u32,
        cb_id: u32,
        pb_id: *mut u8,
        pwsz_credential_type: *const u16,
    }

    #[repr(C)]
    struct WebAuthnAuthenticatorGetAssertionOptions {
        dw_version: u32,
        dw_timeout_milliseconds: u32,
        credential_list: WebAuthnCredentials,
        extensions: WebAuthnExtensions,
        dw_authenticator_attachment: u32,
        dw_user_verification_requirement: u32,
        dw_flags: u32,
        pwsz_u2f_app_id: *const u16,
        pb_u2f_app_id: *mut i32,
        p_cancellation_id: *mut windows::core::GUID,
        p_allow_credential_list: *mut std::ffi::c_void,
        dw_cred_large_blob_operation: u32,
        cb_cred_large_blob: u32,
        pb_cred_large_blob: *mut u8,
        p_hmac_secret_salt_values: *mut WebAuthnHmacSecretSaltValues,
        b_browser_in_private_mode: i32,
    }

    #[repr(C)]
    struct WebAuthnAssertionV3 {
        dw_version: u32,
        cb_authenticator_data: u32,
        pb_authenticator_data: *mut u8,
        cb_signature: u32,
        pb_signature: *mut u8,
        credential: WebAuthnCredential,
        cb_user_id: u32,
        pb_user_id: *mut u8,
        extensions: WebAuthnExtensions,
        cb_cred_large_blob: u32,
        pb_cred_large_blob: *mut u8,
        dw_cred_large_blob_status: u32,
        p_hmac_secret: *mut WebAuthnHmacSecretSalt,
    }

    type FnWebAuthNAuthenticatorGetAssertion = unsafe extern "system" fn(
        h_wnd: HWND,
        pwsz_rp_id: PCWSTR,
        p_client_data_hash: *const WebAuthnClientDataHash,
        p_opt_get_assertion_options: *const WebAuthnAuthenticatorGetAssertionOptions,
        pp_assertion: *mut *mut WebAuthnAssertionV3,
    ) -> windows::core::HRESULT;

    type FnWebAuthNFreeAssertion = unsafe extern "system" fn(
        p_assertion: *mut WebAuthnAssertionV3,
    );

    struct LibraryGuard(HMODULE);

    impl Drop for LibraryGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = FreeLibrary(self.0);
            }
        }
    }

    struct AssertionGuard {
        assertion: *mut WebAuthnAssertionV3,
        free_fn: FnWebAuthNFreeAssertion,
    }

    impl Drop for AssertionGuard {
        fn drop(&mut self) {
            if !self.assertion.is_null() {
                unsafe {
                    (self.free_fn)(self.assertion);
                }
            }
        }
    }

    pub fn get_assertion_hmac_secret(
        challenge: &[u8],
        credential_id: Option<&[u8]>,
    ) -> crate::Result<Vec<u8>> {
        unsafe {
            let lib_name: Vec<u16> = "webauthn.dll\0".encode_utf16().collect();
            let hmod = LoadLibraryW(PCWSTR::from_raw(lib_name.as_ptr()))
                .map_err(|e| VaultError::Hardware2FaNotAvailable(format!("webauthn.dll not found: {}", e)))?;
            let _lib_guard = LibraryGuard(hmod);

            let get_assertion_proc = GetProcAddress(hmod, s!("WebAuthNAuthenticatorGetAssertion"));
            let free_assertion_proc = GetProcAddress(hmod, s!("WebAuthNFreeAssertion"));

            if get_assertion_proc.is_none() || free_assertion_proc.is_none() {
                return Err(VaultError::Hardware2FaNotAvailable("WebAuthn API symbols not available in webauthn.dll".into()));
            }

            let get_assertion: FnWebAuthNAuthenticatorGetAssertion = std::mem::transmute(get_assertion_proc);
            let free_assertion: FnWebAuthNFreeAssertion = std::mem::transmute(free_assertion_proc);

            let rp_id: Vec<u16> = "yntra-vault\0".encode_utf16().collect();
            let hash_res = blake3::hash(challenge);
            let client_data_hash = WebAuthnClientDataHash {
                cb_client_data_hash: 32,
                pb_client_data_hash: hash_res.as_bytes().as_ptr(),
            };

            // WEBAUTHN_CTAP_ONE_HMAC_SECRET_LENGTH requires exactly 32 bytes for cbFirst
            let mut salt_32 = [0u8; 32];
            let len_to_copy = challenge.len().min(32);
            salt_32[..len_to_copy].copy_from_slice(&challenge[..len_to_copy]);

            let mut hmac_salt = WebAuthnHmacSecretSalt {
                cb_first: 32,
                pb_first: salt_32.as_ptr(),
                cb_second: 0,
                pb_second: std::ptr::null(),
            };

            let mut cred_salt;
            let mut salt_values = if let Some(cid) = credential_id {
                cred_salt = WebAuthnCredWithHmacSecretSalt {
                    cb_cred_id: cid.len() as u32,
                    pb_cred_id: cid.as_ptr(),
                    p_hmac_secret_salt: &mut hmac_salt,
                };
                WebAuthnHmacSecretSaltValues {
                    p_global_hmac_salt: std::ptr::null_mut(),
                    c_cred_with_hmac_secret_salt_list: 1,
                    p_cred_with_hmac_secret_salt_list: &mut cred_salt,
                }
            } else {
                WebAuthnHmacSecretSaltValues {
                    p_global_hmac_salt: &mut hmac_salt,
                    c_cred_with_hmac_secret_salt_list: 0,
                    p_cred_with_hmac_secret_salt_list: std::ptr::null_mut(),
                }
            };

            let mut options = std::mem::zeroed::<WebAuthnAuthenticatorGetAssertionOptions>();
            options.dw_version = 6;
            options.dw_timeout_milliseconds = 30000;
            options.dw_authenticator_attachment = 2; // Cross-platform (hardware keys)
            options.dw_user_verification_requirement = 1; // Preferred
            options.dw_flags = 0x00100000; // WEBAUTHN_AUTHENTICATOR_HMAC_SECRET_VALUES_FLAG
            options.p_hmac_secret_salt_values = &mut salt_values;

            let mut p_assertion: *mut WebAuthnAssertionV3 = std::ptr::null_mut();
            let hr = get_assertion(
                HWND::default(),
                PCWSTR::from_raw(rp_id.as_ptr()),
                &client_data_hash,
                &options,
                &mut p_assertion,
            );

            if hr.is_err() || p_assertion.is_null() {
                return Err(VaultError::Hardware2FaAuthFailed(
                    format!("FIDO2/CTAP2 security key assertion canceled, timed out, or not supported (HRESULT 0x{:08X})", hr.0 as u32)
                ));
            }

            let _assertion_guard = AssertionGuard {
                assertion: p_assertion,
                free_fn: free_assertion,
            };

            let assertion = &*p_assertion;
            if assertion.dw_version >= 3 && !assertion.p_hmac_secret.is_null() {
                let salt_out = &*assertion.p_hmac_secret;
                if salt_out.cb_first > 0 && !salt_out.pb_first.is_null() {
                    let slice = std::slice::from_raw_parts(salt_out.pb_first, salt_out.cb_first as usize);
                    Ok(slice.to_vec())
                } else {
                    Err(VaultError::Hardware2FaAuthFailed("Security key did not return HMAC secret. Ensure HMAC-secret / PRF is supported on this key.".into()))
                }
            } else {
                Err(VaultError::Hardware2FaAuthFailed("Security key did not return HMAC secret. Ensure HMAC-secret / PRF is supported on this key.".into()))
            }
        }
    }
}

#[cfg(target_os = "windows")]
mod win_yubikey_hid {
    use super::*;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HANDLE, CloseHandle, GENERIC_READ, GENERIC_WRITE};
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, ReadFile, WriteFile, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, FILE_FLAGS_AND_ATTRIBUTES,
    };

    const GUID_DEVINTERFACE_HID: windows::core::GUID = windows::core::GUID::from_u128(0x4d1e55b2_f16f_11cf_88cb_001111000030);

    #[link(name = "cfgmgr32")]
    unsafe extern "system" {
        fn CM_Get_Device_Interface_List_SizeW(
            pul_len: *mut u32,
            p_interface_class_guid: *const windows::core::GUID,
            p_device_id: *const u16,
            ul_flags: u32,
        ) -> u32;
        fn CM_Get_Device_Interface_ListW(
            p_interface_class_guid: *const windows::core::GUID,
            p_device_id: *const u16,
            buffer: *mut u16,
            ul_buffer_len: u32,
            ul_flags: u32,
        ) -> u32;
    }

    fn yubikey_crc(buf: &[u8]) -> u16 {
        let mut crc: u16 = 0xFFFF;
        for &b in buf {
            crc ^= b as u16;
            for _ in 0..8 {
                let j = crc & 1;
                crc >>= 1;
                if j != 0 {
                    crc ^= 0x8408;
                }
            }
        }
        crc
    }

    pub fn enumerate_yubikey_paths() -> Vec<String> {
        let mut paths = Vec::new();
        unsafe {
            let mut len = 0u32;
            if CM_Get_Device_Interface_List_SizeW(&mut len, &GUID_DEVINTERFACE_HID, std::ptr::null(), 0) != 0 || len == 0 {
                return paths;
            }
            let mut buf = vec![0u16; len as usize];
            if CM_Get_Device_Interface_ListW(&GUID_DEVINTERFACE_HID, std::ptr::null(), buf.as_mut_ptr(), len, 0) != 0 {
                return paths;
            }

            let mut start = 0;
            for i in 0..buf.len() {
                if buf[i] == 0 {
                    if i > start {
                        if let Ok(s) = String::from_utf16(&buf[start..i]) {
                            let upper = s.to_uppercase();
                            if upper.contains("VID_1050") {
                                paths.push(s);
                            }
                        }
                    }
                    start = i + 1;
                }
            }
        }
        paths
    }

    pub fn challenge_response_slot(path: &str, slot: u8, challenge: &[u8]) -> crate::Result<Vec<u8>> {
        let path_w: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let handle = unsafe {
            CreateFileW(
                PCWSTR::from_raw(path_w.as_ptr()),
                GENERIC_READ.0 | GENERIC_WRITE.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                HANDLE::default(),
            )
        };

        let handle = match handle {
            Ok(h) if !h.is_invalid() => h,
            _ => {
                let alt_h = unsafe {
                    CreateFileW(
                        PCWSTR::from_raw(path_w.as_ptr()),
                        0,
                        FILE_SHARE_READ | FILE_SHARE_WRITE,
                        None,
                        OPEN_EXISTING,
                        FILE_FLAGS_AND_ATTRIBUTES(0),
                        HANDLE::default(),
                    )
                };
                match alt_h {
                    Ok(h) if !h.is_invalid() => h,
                    _ => return Err(VaultError::Hardware2FaAuthFailed(
                        "Unable to open YubiKey HID interface. Ensure no other application has exclusive access.".into()
                    )),
                }
            }
        };

        struct HandleGuard(HANDLE);
        impl Drop for HandleGuard {
            fn drop(&mut self) {
                unsafe { let _ = CloseHandle(self.0); }
            }
        }
        let _guard = HandleGuard(handle);

        let mut frame = [0u8; 70];
        let ch_len = challenge.len().min(64);
        frame[..ch_len].copy_from_slice(&challenge[..ch_len]);
        frame[64] = slot;
        let crc = yubikey_crc(&frame[..65]);
        frame[65] = (crc & 0xFF) as u8;
        frame[66] = ((crc >> 8) & 0xFF) as u8;

        for seq in 0..10 {
            let mut packet = [0u8; 9];
            packet[0] = 0;
            let start = seq * 7;
            let end = (start + 7).min(70);
            packet[1..1 + (end - start)].copy_from_slice(&frame[start..end]);
            packet[8] = if seq == 9 { 0x80 | (seq as u8) } else { seq as u8 };

            let mut bytes_written = 0u32;
            let res = unsafe {
                WriteFile(
                    handle,
                    Some(&packet),
                    Some(&mut bytes_written),
                    None,
                )
            };
            if res.is_err() {
                let res8 = unsafe {
                    WriteFile(
                        handle,
                        Some(&packet[1..]),
                        Some(&mut bytes_written),
                        None,
                    )
                };
                if res8.is_err() {
                    return Err(VaultError::Hardware2FaAuthFailed("Failed writing challenge report to YubiKey.".into()));
                }
            }
        }

        let start_time = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(15);
        let mut resp_data = Vec::with_capacity(32);

        while start_time.elapsed() < timeout {
            let mut in_buf = [0u8; 9];
            let mut bytes_read = 0u32;
            let read_res = unsafe {
                ReadFile(
                    handle,
                    Some(&mut in_buf),
                    Some(&mut bytes_read),
                    None,
                )
            };

            if read_res.is_ok() && bytes_read >= 8 {
                let payload = if bytes_read == 9 { &in_buf[1..] } else { &in_buf[..8] };
                let seq = payload[7];
                if seq & 0x40 != 0 {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }
                resp_data.extend_from_slice(&payload[..7]);
                if seq & 0x80 != 0 {
                    break;
                }
            } else {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }

        if resp_data.len() >= 22 {
            let expected_crc = yubikey_crc(&resp_data[..20]);
            let actual_crc = (resp_data[20] as u16) | ((resp_data[21] as u16) << 8);
            if expected_crc != actual_crc {
                return Err(VaultError::Hardware2FaAuthFailed(
                    "YubiKey response CRC check failed. The slot may be unconfigured or corrupted.".into(),
                ));
            }
            if resp_data[..20].iter().all(|&b| b == 0) {
                return Err(VaultError::Hardware2FaAuthFailed(
                    "YubiKey returned an unconfigured or empty response frame.".into(),
                ));
            }
            Ok(resp_data[..20].to_vec())
        } else if resp_data.len() >= 20 {
            if resp_data[..20].iter().all(|&b| b == 0) {
                return Err(VaultError::Hardware2FaAuthFailed(
                    "YubiKey returned an unconfigured or empty response frame.".into(),
                ));
            }
            Ok(resp_data[..20].to_vec())
        } else {
            Err(VaultError::Hardware2FaAuthFailed(
                "YubiKey challenge-response communication timeout or touch rejected.".into(),
            ))
        }
    }

    pub fn perform_yubikey_challenge(challenge: &[u8]) -> crate::Result<Vec<u8>> {
        let paths = enumerate_yubikey_paths();
        if paths.is_empty() {
            return Err(VaultError::Hardware2FaNotAvailable(
                "No YubiKey hardware device detected. Please connect your YubiKey.".into()
            ));
        }

        for path in &paths {
            if let Ok(resp) = challenge_response_slot(path, 0x38, challenge) {
                return Ok(resp);
            }
            if let Ok(resp) = challenge_response_slot(path, 0x30, challenge) {
                return Ok(resp);
            }
        }

        Err(VaultError::Hardware2FaAuthFailed(
            "YubiKey challenge-response failed. Ensure Slot 2 or Slot 1 is configured with HMAC-SHA1 Challenge-Response.".into()
        ))
    }
}

fn mock_test_challenge(protocol: Hardware2FaProtocol, challenge: &[u8]) -> crate::Result<Vec<u8>> {
    match protocol {
        Hardware2FaProtocol::YubiKeyChallengeResponse => {
            let test_seed = b"yntra-vault-unit-test-yubikey-seed";
            let mut mac = <HmacSha1 as Mac>::new_from_slice(test_seed)
                .map_err(|e| VaultError::Hardware2FaAuthFailed(format!("HMAC init failed: {}", e)))?;
            mac.update(challenge);
            let result = mac.finalize().into_bytes();
            Ok(result.to_vec())
        }
        Hardware2FaProtocol::Fido2Ctap2HmacSecret => {
            let test_seed = b"yntra-vault-unit-test-fido2-ctap2-seed";
            let mut mac = <HmacSha256 as Mac>::new_from_slice(test_seed)
                .map_err(|e| VaultError::Hardware2FaAuthFailed(format!("HMAC-SHA256 init failed: {}", e)))?;
            mac.update(challenge);
            let result = mac.finalize().into_bytes();
            Ok(result.to_vec())
        }
    }
}

/// Derive master key from password + keyfile + hardware 2FA response using BLAKE3 pre-hash + Argon2id.
pub fn derive_master_key_with_hardware_2fa(
    password: &[u8],
    key_file_bytes: Option<&[u8]>,
    hardware_response: &[u8],
    salt: &[u8; 32],
) -> crate::Result<MasterKey> {
    if password.is_empty() {
        return Err(VaultError::InvalidPassword);
    }
    if hardware_response.is_empty() {
        return Err(VaultError::Hardware2FaRequired);
    }

    let mut hasher = blake3::Hasher::new_derive_key("yntra-vault-hardware2fa-prehash-v1");
    hasher.update(&(password.len() as u64).to_le_bytes());
    hasher.update(password);

    if let Some(kf) = key_file_bytes {
        hasher.update(&(kf.len() as u64).to_le_bytes());
        hasher.update(kf);
    } else {
        hasher.update(&0u64.to_le_bytes());
    }

    hasher.update(&(hardware_response.len() as u64).to_le_bytes());
    hasher.update(hardware_response);

    let combined = Zeroizing::new(*hasher.finalize().as_bytes());
    derive_master_key(&*combined, salt)
}

/// Create an EmbeddedHardware2FaHeader for single-file .vdb storage.
/// Cryptographically binds Factor 1 (Master Password + optional Key File) AND Factor 2 (Hardware Key Response)
/// along with the persistent challenge salt and canonical header AAD.
#[allow(clippy::too_many_arguments)]
pub fn create_embedded_hardware2fa_header(
    subkeys: &SubKeys,
    aad: &[u8],
    protocol: Hardware2FaProtocol,
    key_name: &str,
    challenge_salt: [u8; 32],
    credential_id: Vec<u8>,
    password: &[u8],
    key_file_bytes: Option<&[u8]>,
    hardware_response: &[u8],
) -> crate::Result<EmbeddedHardware2FaHeader> {
    if password.is_empty() {
        return Err(VaultError::InvalidPassword);
    }
    if hardware_response.is_empty() {
        return Err(VaultError::Hardware2FaAuthFailed(
            "Hardware response empty during key enrollment".into(),
        ));
    }

    // Derive KEK v2: password + key_file + hardware_response + challenge_salt + canonical header AAD
    let mut kek_hasher = blake3::Hasher::new_derive_key("yntra-vault-hardware2fa-kek-v2");
    kek_hasher.update(&(password.len() as u64).to_le_bytes());
    kek_hasher.update(password);

    if let Some(kf) = key_file_bytes {
        kek_hasher.update(&(kf.len() as u64).to_le_bytes());
        kek_hasher.update(kf);
    } else {
        kek_hasher.update(&0u64.to_le_bytes());
    }

    kek_hasher.update(&(hardware_response.len() as u64).to_le_bytes());
    kek_hasher.update(hardware_response);
    kek_hasher.update(&challenge_salt);
    kek_hasher.update(aad);
    let kek_bytes = Zeroizing::new(*kek_hasher.finalize().as_bytes());

    let cipher = XChaCha20Poly1305::new_from_slice(&*kek_bytes)
        .map_err(|e| VaultError::EncryptionError(format!("Hardware 2FA cipher init failed: {}", e)))?;

    let mut nonce_bytes = [0u8; 24];
    rand::rng().fill(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);

    let raw_subkeys = Zeroizing::new(subkeys.to_bytes());
    let payload = chacha20poly1305::aead::Payload {
        msg: raw_subkeys.as_slice(),
        aad,
    };

    let encrypted_subkeys = cipher
        .encrypt(nonce, payload)
        .map_err(|e| VaultError::EncryptionError(format!("Hardware 2FA subkey encryption failed: {}", e)))?;

    let wrapped_kek = crate::tpm::hardware_wrap_key(&*kek_bytes)?;

    Ok(EmbeddedHardware2FaHeader {
        protocol,
        credential_id,
        challenge_salt,
        nonce: nonce_bytes,
        wrapped_kek,
        encrypted_subkeys,
        key_name: key_name.to_string(),
    })
}

/// Unlock vault subkeys using embedded Hardware 2FA envelope headers and canonical AAD.
/// Cryptographically enforces Factor 1 (Password + optional Key File) AND Factor 2 (Hardware Key Response).
pub fn unlock_from_embedded_hardware2fa_headers(
    hw_headers: &[EmbeddedHardware2FaHeader],
    aad: &[u8],
    password: &[u8],
    key_file_bytes: Option<&[u8]>,
    hardware_response: &[u8],
) -> crate::Result<SubKeys> {
    if password.is_empty() {
        return Err(VaultError::InvalidPassword);
    }
    if hardware_response.is_empty() {
        return Err(VaultError::Hardware2FaAuthFailed("Empty hardware response provided".into()));
    }

    for hw_header in hw_headers {
        let mut kek_hasher = blake3::Hasher::new_derive_key("yntra-vault-hardware2fa-kek-v2");
        kek_hasher.update(&(password.len() as u64).to_le_bytes());
        kek_hasher.update(password);

        if let Some(kf) = key_file_bytes {
            kek_hasher.update(&(kf.len() as u64).to_le_bytes());
            kek_hasher.update(kf);
        } else {
            kek_hasher.update(&0u64.to_le_bytes());
        }

        kek_hasher.update(&(hardware_response.len() as u64).to_le_bytes());
        kek_hasher.update(hardware_response);
        kek_hasher.update(&hw_header.challenge_salt);
        kek_hasher.update(aad);
        let kek_bytes = Zeroizing::new(*kek_hasher.finalize().as_bytes());

        if let Ok(cipher) = XChaCha20Poly1305::new_from_slice(&*kek_bytes) {
            let nonce = XNonce::from_slice(&hw_header.nonce);
            let payload = chacha20poly1305::aead::Payload {
                msg: hw_header.encrypted_subkeys.as_slice(),
                aad,
            };

            if let Ok(decrypted_bytes) = cipher.decrypt(nonce, payload) {
                let locked_subkeys = LockedBuffer::new(&decrypted_bytes);
                if let Ok(subkeys) = SubKeys::from_bytes(locked_subkeys.as_slice()) {
                    return Ok(subkeys);
                }
            }
        }
    }

    Err(VaultError::Hardware2FaAuthFailed(
        "Hardware key response verification failed or incorrect master password".into(),
    ))
}

/// Merge two sets of hardware 2FA key headers, eliminating duplicate credential IDs and key names.
pub fn merge_hardware2fa_headers(
    local: Option<Vec<EmbeddedHardware2FaHeader>>,
    remote: Option<Vec<EmbeddedHardware2FaHeader>>,
) -> Option<Vec<EmbeddedHardware2FaHeader>> {
    match (local, remote) {
        (None, None) => None,
        (Some(l), None) => Some(l),
        (None, Some(r)) => Some(r),
        (Some(l), Some(r)) => {
            let mut merged = l;
            for r_item in r {
                if !merged.iter().any(|existing| existing.credential_id == r_item.credential_id || existing.key_name == r_item.key_name) {
                    merged.push(r_item);
                }
            }
            Some(merged)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{derive_master_key, derive_subkeys};

    #[test]
    fn test_hardware2fa_challenge_response_yubikey() {
        let challenge = b"random-test-challenge-12345";
        let res1 = perform_hardware2fa_challenge(Hardware2FaProtocol::YubiKeyChallengeResponse, challenge).unwrap();
        let res2 = perform_hardware2fa_challenge(Hardware2FaProtocol::YubiKeyChallengeResponse, challenge).unwrap();
        assert_eq!(res1, res2);
        assert_eq!(res1.len(), 20); // HMAC-SHA1 length
    }

    #[test]
    fn test_hardware2fa_challenge_response_fido2() {
        let challenge = b"random-test-challenge-12345";
        let res1 = perform_hardware2fa_challenge(Hardware2FaProtocol::Fido2Ctap2HmacSecret, challenge).unwrap();
        let res2 = perform_hardware2fa_challenge(Hardware2FaProtocol::Fido2Ctap2HmacSecret, challenge).unwrap();
        assert_eq!(res1, res2);
        assert_eq!(res1.len(), 32); // HMAC-SHA256 length
    }

    #[test]
    fn test_master_key_derivation_with_hardware2fa() {
        let password = b"master_password_123";
        let salt = [11u8; 32];
        let challenge = b"vault_challenge_bytes";
        let hw_resp = perform_hardware2fa_challenge(Hardware2FaProtocol::YubiKeyChallengeResponse, challenge).unwrap();

        let mk1 = derive_master_key_with_hardware_2fa(password, None, &hw_resp, &salt).unwrap();
        let mk2 = derive_master_key_with_hardware_2fa(password, None, &hw_resp, &salt).unwrap();
        assert_eq!(mk1.as_bytes(), mk2.as_bytes());

        let wrong_resp = vec![0u8; 20];
        let mk_wrong = derive_master_key_with_hardware_2fa(password, None, &wrong_resp, &salt).unwrap();
        assert_ne!(mk1.as_bytes(), mk_wrong.as_bytes());
    }

    #[test]
    fn test_embedded_hardware2fa_roundtrip() {
        let password = b"test_password";
        let master_key = derive_master_key(password, &[55u8; 32]).unwrap();
        let subkeys = derive_subkeys(&master_key).unwrap();
        let aad = b"canonical_vault_header_aad_bytes";

        let challenge_salt = [77u8; 32];
        let hw_resp = perform_hardware2fa_challenge(Hardware2FaProtocol::YubiKeyChallengeResponse, &challenge_salt).unwrap();

        let hw_header = create_embedded_hardware2fa_header(
            &subkeys,
            aad,
            Hardware2FaProtocol::YubiKeyChallengeResponse,
            "My YubiKey 5C",
            challenge_salt,
            vec![1, 2, 3, 4],
            password,
            None,
            &hw_resp,
        ).unwrap();

        // 1. Success with correct password and hardware response
        let restored = unlock_from_embedded_hardware2fa_headers(&[hw_header.clone()], aad, password, None, &hw_resp).unwrap();
        assert_eq!(subkeys.vault_key.bytes, restored.vault_key.bytes);
        assert_eq!(subkeys.entry_key.bytes, restored.entry_key.bytes);

        // 2. Incorrect master password fails AEAD decryption (True 2FA enforcement)
        let wrong_password = b"wrong_password_123";
        let err_pass = unlock_from_embedded_hardware2fa_headers(&[hw_header.clone()], aad, wrong_password, None, &hw_resp);
        assert!(err_pass.is_err());

        // 3. Incorrect hardware response fails (Factor 2 enforcement)
        let invalid_resp = vec![0xFFu8; 20];
        let err_hw = unlock_from_embedded_hardware2fa_headers(&[hw_header.clone()], aad, password, None, &invalid_resp);
        assert!(err_hw.is_err());
    }

    #[test]
    fn test_header_stripping_attack_prevented() {
        let password = b"test_password";
        let master_key = derive_master_key(password, &[55u8; 32]).unwrap();
        let subkeys = derive_subkeys(&master_key).unwrap();
        let aad = b"canonical_vault_header_aad_bytes";

        let challenge_salt = [88u8; 32];
        let hw_resp = perform_hardware2fa_challenge(Hardware2FaProtocol::YubiKeyChallengeResponse, &challenge_salt).unwrap();

        let hw_header = create_embedded_hardware2fa_header(
            &subkeys,
            aad,
            Hardware2FaProtocol::YubiKeyChallengeResponse,
            "My YubiKey 5C",
            challenge_salt,
            vec![1, 2, 3, 4],
            password,
            None,
            &hw_resp,
        ).unwrap();

        // Tampering with AAD fails unlocking (AAD authentication)
        let tampered_aad = b"tampered_header_aad_bytes";
        let err = unlock_from_embedded_hardware2fa_headers(&[hw_header], tampered_aad, password, None, &hw_resp);
        assert!(err.is_err());
    }

    #[test]
    fn test_merge_hardware2fa_headers() {
        let h1 = EmbeddedHardware2FaHeader {
            protocol: Hardware2FaProtocol::YubiKeyChallengeResponse,
            credential_id: vec![1, 2, 3],
            challenge_salt: [0u8; 32],
            nonce: [0u8; 24],
            wrapped_kek: vec![],
            encrypted_subkeys: vec![],
            key_name: "Key 1".to_string(),
        };

        let h2 = EmbeddedHardware2FaHeader {
            protocol: Hardware2FaProtocol::Fido2Ctap2HmacSecret,
            credential_id: vec![4, 5, 6],
            challenge_salt: [1u8; 32],
            nonce: [1u8; 24],
            wrapped_kek: vec![],
            encrypted_subkeys: vec![],
            key_name: "Key 2".to_string(),
        };

        let merged = merge_hardware2fa_headers(Some(vec![h1.clone()]), Some(vec![h2.clone()])).unwrap();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].key_name, "Key 1");
        assert_eq!(merged[1].key_name, "Key 2");

        // Duplicate keys are deduplicated
        let dup_merged = merge_hardware2fa_headers(Some(vec![h1.clone()]), Some(vec![h1.clone()])).unwrap();
        assert_eq!(dup_merged.len(), 1);
    }

    #[test]
    fn test_hardware_keys_detection_and_availability() {
        let info = check_hardware2fa_availability();
        assert_eq!(info.available, info.key_count > 0);
        assert_eq!(info.key_count, info.connected_keys.len());
        let empty_err = perform_hardware2fa_challenge(Hardware2FaProtocol::YubiKeyChallengeResponse, b"");
        assert!(empty_err.is_err());
    }

    #[test]
    fn test_empty_password_strictly_rejected() {
        let master_key = derive_master_key(b"test_password", &[55u8; 32]).unwrap();
        let subkeys = derive_subkeys(&master_key).unwrap();
        let aad = b"canonical_vault_header_aad_bytes";
        let challenge_salt = [77u8; 32];
        let hw_resp = perform_hardware2fa_challenge(Hardware2FaProtocol::YubiKeyChallengeResponse, &challenge_salt).unwrap();

        // 1. derive_master_key_with_hardware_2fa rejects empty password
        let res_derive = derive_master_key_with_hardware_2fa(b"", None, &hw_resp, &[1u8; 32]);
        assert!(matches!(res_derive, Err(VaultError::InvalidPassword)));

        // 2. create_embedded_hardware2fa_header rejects empty password
        let res_create = create_embedded_hardware2fa_header(
            &subkeys,
            aad,
            Hardware2FaProtocol::YubiKeyChallengeResponse,
            "Key",
            challenge_salt,
            vec![],
            b"",
            None,
            &hw_resp,
        );
        assert!(matches!(res_create, Err(VaultError::InvalidPassword)));

        // 3. unlock_from_embedded_hardware2fa_headers rejects empty password
        let hw_header = create_embedded_hardware2fa_header(
            &subkeys,
            aad,
            Hardware2FaProtocol::YubiKeyChallengeResponse,
            "Key",
            challenge_salt,
            vec![],
            b"correct_password",
            None,
            &hw_resp,
        ).unwrap();
        let res_unlock = unlock_from_embedded_hardware2fa_headers(&[hw_header], aad, b"", None, &hw_resp);
        assert!(matches!(res_unlock, Err(VaultError::InvalidPassword)));
    }
}
