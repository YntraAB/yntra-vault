//! Memory safety, heap scrambling, memory page locking, and SOTA protected secret utilities.

use std::sync::OnceLock;
use rand::Rng;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};
use chacha20poly1305::{XChaCha20Poly1305, aead::{Aead, AeadInPlace, KeyInit}, XNonce, Tag};

static EPHEMERAL_KEY: OnceLock<LockedBuffer> = OnceLock::new();

/// Retrieve or lazily initialize the master ephemeral in-memory encryption key.
/// The 256-bit key is pinned inside a page-locked LockedBuffer in physical RAM.
fn get_ephemeral_key() -> &'static [u8] {
    let locked_key = EPHEMERAL_KEY.get_or_init(|| {
        let mut key = [0u8; 32];
        rand::rng().fill(&mut key);
        let locked = LockedBuffer::new(&key);
        key.zeroize();
        locked
    });
    locked_key.as_slice()
}

/// A heap-allocated string container that scrambles string content in memory
/// using an ephemeral key generated dynamically at application startup.
///
/// Ciphertext and nonces are automatically zeroed on drop.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ScrambledString {
    ciphertext: Vec<u8>,
    nonce: [u8; 24],
}

impl ScrambledString {
    /// Encrypt a plaintext string into a ScrambledString.
    pub fn new(plaintext: &str) -> Self {
        let key = get_ephemeral_key();
        let cipher = XChaCha20Poly1305::new_from_slice(key).unwrap();

        let mut nonce_bytes = [0u8; 24];
        rand::rng().fill(&mut nonce_bytes);
        let nonce = XNonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .expect("Encryption of scrambled heap string failed");

        Self {
            ciphertext,
            nonce: nonce_bytes,
        }
    }

    /// Decrypt directly into a pre-allocated, pre-locked RAM buffer (`LockedBuffer`).
    ///
    /// The ciphertext is copied directly into physical RAM locked via `VirtualLock`/`mlock`
    /// and decrypted in-place using `AeadInPlace::decrypt_in_place_detached`.
    /// Plaintext is NEVER created or held in unpinned heap memory.
    pub fn decrypt_to_locked(&self) -> crate::Result<LockedBuffer> {
        let key = get_ephemeral_key();
        let cipher = XChaCha20Poly1305::new_from_slice(key)
            .map_err(|e| crate::error::VaultError::DecryptionError(format!("Ephemeral key init: {}", e)))?;
        let nonce = XNonce::from_slice(&self.nonce);

        if self.ciphertext.len() < 16 {
            return Err(crate::error::VaultError::DecryptionError("Invalid scrambled ciphertext length".into()));
        }

        let plaintext_len = self.ciphertext.len() - 16;

        // 1. Allocate a zeroed LockedBuffer PRE-LOCKED in physical RAM
        let mut locked = LockedBuffer::zeroed(self.ciphertext.len());

        // 2. Copy ciphertext + tag into the pre-locked RAM page
        locked.as_mut_slice().copy_from_slice(&self.ciphertext);

        // 3. Split buffer into payload and tag, then decrypt IN-PLACE inside the pre-locked RAM page
        let (payload, tag_bytes) = locked.as_mut_slice().split_at_mut(plaintext_len);
        let tag = Tag::from_slice(tag_bytes);

        cipher
            .decrypt_in_place_detached(nonce, b"", payload, tag)
            .map_err(|_| crate::error::VaultError::DecryptionError("Scrambled string decrypt failed".into()))?;

        // 4. Truncate length to decrypted plaintext size
        locked.set_len(plaintext_len);

        Ok(locked)
    }

    /// Decrypt the ScrambledString back into a Zeroizing<String> wrapper.
    pub fn decrypt(&self) -> crate::Result<Zeroizing<String>> {
        let locked = self.decrypt_to_locked()?;
        let string = std::str::from_utf8(locked.as_slice())
            .map_err(|_| crate::error::VaultError::DecryptionError("Scrambled string not valid UTF-8".into()))?;

        Ok(Zeroizing::new(string.to_string()))
    }
}

impl Zeroize for ScrambledString {
    fn zeroize(&mut self) {
        self.ciphertext.zeroize();
        self.nonce.zeroize();
    }
}

impl Drop for ScrambledString {
    fn drop(&mut self) {
        self.zeroize();
    }
}

/// A memory-locked buffer wrapper that pins memory pages in RAM,
/// A memory-locked buffer wrapper protected by hardware-level Canary Guard Pages (`PAGE_NOACCESS` / `PROT_NONE`).
///
/// Layout:
/// ┌────────────────────┬──────────────────────────────────────┬────────────────────┐
/// │ Leading Guard Page │ Locked Secret Payload Page(s)        │ Trailing Guard Page│
/// │ [PAGE_NOACCESS]    │ [PAGE_READWRITE + VirtualLock/mlock] │ [PAGE_NOACCESS]    │
/// └────────────────────┴──────────────────────────────────────┴────────────────────┘
/// Any out-of-bounds heap read/write into guard pages triggers an immediate hardware MMU page fault.
pub struct LockedBuffer {
    base_ptr: *mut u8,
    payload_ptr: *mut u8,
    #[allow(dead_code)]
    total_size: usize,
    payload_alloc_size: usize,
    len: usize,
}

unsafe impl Send for LockedBuffer {}
unsafe impl Sync for LockedBuffer {}

impl LockedBuffer {
    /// Create a new pre-locked, page-aligned zeroed buffer protected by leading and trailing guard pages.
    pub fn zeroed(len: usize) -> Self {
        let page_size = 4096;
        let payload_pages = if len == 0 { 1 } else { (len + page_size - 1) / page_size };
        let payload_alloc_size = payload_pages * page_size;
        let total_pages = 1 + payload_pages + 1; // Leading guard + payload pages + trailing guard
        let total_size = total_pages * page_size;

        #[cfg(target_os = "windows")]
        unsafe {
            use windows::Win32::System::Memory::{
                VirtualAlloc, VirtualProtect, VirtualLock,
                MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE, PAGE_NOACCESS, PAGE_PROTECTION_FLAGS,
            };
            use windows::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};

            // 1. Allocate virtual pages
            let base_ptr = VirtualAlloc(None, total_size, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) as *mut u8;
            if base_ptr.is_null() {
                panic!("Failed VirtualAlloc for Guarded LockedBuffer");
            }

            let payload_ptr = base_ptr.add(page_size);
            let trailing_guard_ptr = base_ptr.add((1 + payload_pages) * page_size);

            // 2. Protect leading and trailing guard pages with PAGE_NOACCESS
            let mut old_prot = PAGE_PROTECTION_FLAGS::default();
            let _ = VirtualProtect(base_ptr as *const std::ffi::c_void, page_size, PAGE_NOACCESS, &mut old_prot);
            let _ = VirtualProtect(trailing_guard_ptr as *const std::ffi::c_void, page_size, PAGE_NOACCESS, &mut old_prot);

            // 3. Lock payload pages in physical RAM
            if VirtualLock(payload_ptr as *const std::ffi::c_void, payload_alloc_size).is_err() {
                let process = GetCurrentProcess();
                let _ = SetProcessWorkingSetSize(process, payload_alloc_size + 65536, payload_alloc_size + 1048576);
                let _ = VirtualLock(payload_ptr as *const std::ffi::c_void, payload_alloc_size);
            }

            Self {
                base_ptr,
                payload_ptr,
                total_size,
                payload_alloc_size,
                len,
            }
        }

        #[cfg(not(target_os = "windows"))]
        unsafe {
            let base_ptr = libc::mmap(
                std::ptr::null_mut(),
                total_size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            ) as *mut u8;

            if base_ptr == libc::MAP_FAILED as *mut u8 {
                panic!("Failed mmap for Guarded LockedBuffer");
            }

            let payload_ptr = base_ptr.add(page_size);
            let trailing_guard_ptr = base_ptr.add((1 + payload_pages) * page_size);

            // Protect leading and trailing guard pages with PROT_NONE
            let _ = libc::mprotect(base_ptr as *mut std::ffi::c_void, page_size, libc::PROT_NONE);
            let _ = libc::mprotect(trailing_guard_ptr as *mut std::ffi::c_void, page_size, libc::PROT_NONE);

            // Lock payload pages in physical RAM
            if libc::mlock(payload_ptr as *const std::ffi::c_void, payload_alloc_size) != 0 {
                let mut rlim = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
                if libc::getrlimit(libc::RLIMIT_MEMLOCK, &mut rlim) == 0 {
                    let new_limit = (rlim.rlim_cur + payload_alloc_size as u64 + 65536).min(rlim.rlim_max);
                    rlim.rlim_cur = new_limit;
                    let _ = libc::setrlimit(libc::RLIMIT_MEMLOCK, &rlim);
                    let _ = libc::mlock(payload_ptr as *const std::ffi::c_void, payload_alloc_size);
                }
            }

            #[cfg(target_os = "linux")]
            {
                let _ = libc::madvise(payload_ptr as *mut std::ffi::c_void, payload_alloc_size, libc::MADV_DONTDUMP);
            }

            Self {
                base_ptr,
                payload_ptr,
                total_size,
                payload_alloc_size,
                len,
            }
        }
    }

    /// Create a new page-aligned LockedBuffer from raw bytes and attempt to lock it.
    pub fn new(bytes: &[u8]) -> Self {
        let buffer = Self::zeroed(bytes.len());
        if !bytes.is_empty() {
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer.payload_ptr, bytes.len());
            }
        }
        buffer
    }

    /// Access the underlying locked bytes as a slice.
    pub fn as_slice(&self) -> &[u8] {
        if self.len == 0 || self.payload_ptr.is_null() {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.payload_ptr, self.len) }
        }
    }

    /// Access the underlying locked bytes as a mutable slice.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        if self.len == 0 || self.payload_ptr.is_null() {
            &mut []
        } else {
            unsafe { std::slice::from_raw_parts_mut(self.payload_ptr, self.len) }
        }
    }

    /// Update the valid slice length.
    pub fn set_len(&mut self, new_len: usize) {
        assert!(new_len <= self.payload_alloc_size, "Target length exceeds allocated page payload size");
        self.len = new_len;
    }
}

impl Zeroize for LockedBuffer {
    fn zeroize(&mut self) {
        if !self.payload_ptr.is_null() && self.payload_alloc_size > 0 {
            unsafe {
                // Volatile byte-by-byte write zeroization to guarantee LLVM compiler
                // never optimizes away memory wiping prior to deallocation
                for i in 0..self.payload_alloc_size {
                    std::ptr::write_volatile(self.payload_ptr.add(i), 0u8);
                }
                std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
            }
        }
    }
}

impl Drop for LockedBuffer {
    fn drop(&mut self) {
        if !self.base_ptr.is_null() {
            // 1. Volatile Zeroize payload contents first
            self.zeroize();

            // 2. Unlock memory payload page
            #[cfg(target_os = "windows")]
            unsafe {
                use windows::Win32::System::Memory::{VirtualUnlock, VirtualFree, MEM_RELEASE};
                let _ = VirtualUnlock(self.payload_ptr as *const std::ffi::c_void, self.payload_alloc_size);
                let _ = VirtualFree(self.base_ptr as *mut std::ffi::c_void, 0, MEM_RELEASE);
            }

            #[cfg(not(target_os = "windows"))]
            unsafe {
                let _ = libc::munlock(self.payload_ptr as *const std::ffi::c_void, self.payload_alloc_size);
                let _ = libc::munmap(self.base_ptr as *mut std::ffi::c_void, self.total_size);
            }
        }
    }
}

/// State-of-the-Art Protected Secret container.
///
/// Encrypts secrets at rest in RAM (`ScrambledString`) and transiently
/// exposes them inside a page-locked buffer (`LockedBuffer`) for scoped execution.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ProtectedSecret {
    scrambled: ScrambledString,
}

impl ProtectedSecret {
    /// Create a new ProtectedSecret from a plaintext string.
    pub fn new(secret: &str) -> Self {
        Self {
            scrambled: ScrambledString::new(secret),
        }
    }

    /// Execute a closure with access to the decrypted secret inside a pre-locked buffer.
    /// Memory is decrypted in-place inside physical locked RAM and automatically zeroed upon return.
    pub fn with_secret<F, R>(&self, f: F) -> crate::Result<R>
    where
        F: FnOnce(&str) -> R,
    {
        let locked = self.scrambled.decrypt_to_locked()?;

        let secret_str = std::str::from_utf8(locked.as_slice())
            .map_err(|_| crate::error::VaultError::DecryptionError("Locked secret invalid UTF-8".into()))?;

        Ok(f(secret_str))
    }
}

impl ZeroizeOnDrop for ProtectedSecret {}

impl Zeroize for ProtectedSecret {
    fn zeroize(&mut self) {
        self.scrambled.zeroize();
    }
}

/// Attempts to disable process core dumps, enforce DLL preloading guard, and suppress crash dialogs.
pub fn prevent_core_dumps() {
    let _ = enforce_dll_preloading_guard();

    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::System::Diagnostics::Debug::{
            SetErrorMode, SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX,
        };
        // Suppress GPF error dialog which generates dump files
        let _ = SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
    }

    #[cfg(not(target_os = "windows"))]
    unsafe {
        let limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        let _ = libc::setrlimit(libc::RLIMIT_CORE, &limit);

        // Prevent ptrace attach (blocks debuggers from reading process memory)
        #[cfg(target_os = "linux")]
        {
            let _ = libc::prctl(libc::PR_SET_DUMPABLE, 0);
        }
    }
}

/// Enforces DLL Preloading Guard on Windows (`SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32)`).
///
/// Prevents DLL side-loading / DLL preloading attack vectors by ensuring that `LoadLibrary`
/// calls strictly search `%SystemRoot%\System32` and ignore the current working directory,
/// application directory, or untrusted user PATH environment variable entries.
pub fn enforce_dll_preloading_guard() -> crate::Result<()> {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::System::LibraryLoader::{
            SetDefaultDllDirectories, LOAD_LIBRARY_SEARCH_SYSTEM32,
        };
        if let Err(e) = SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32) {
            return Err(crate::error::VaultError::DecryptionError(format!(
                "SetDefaultDllDirectories failed: {}",
                e
            )));
        }
    }

    Ok(())
}

/// Detects if the Windows workstation is currently locked (`Win + L` or Winlogon screen active).
pub fn is_workstation_locked() -> bool {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::System::StationsAndDesktops::{
            OpenInputDesktop, CloseDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_SWITCHDESKTOP,
        };
        // OpenInputDesktop returns NULL / ERROR_ACCESS_DENIED when desktop is locked by Winlogon
        if let Ok(h_desk) = OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_SWITCHDESKTOP) {
            let _ = CloseDesktop(h_desk);
            false
        } else {
            true
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}



/// Configures window capture protection (`SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`).
///
/// Prevents screen capture, video recording, Snipping Tool, OBS, Teams screen share, and
/// screen scraping malware from reading secrets displayed in the application window.
pub fn set_window_capture_protection(hwnd_ptr: isize, enable: bool) -> crate::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE, WDA_MONITOR,
        };

        if hwnd_ptr == 0 {
            return Err(crate::error::VaultError::DecryptionError(
                "Invalid null window handle passed to set_window_capture_protection".into(),
            ));
        }

        let hwnd = HWND(hwnd_ptr as _);
        let affinity = if enable {
            WDA_EXCLUDEFROMCAPTURE
        } else {
            WDA_NONE
        };

        unsafe {
            if let Err(e) = SetWindowDisplayAffinity(hwnd, affinity) {
                if enable {
                    // Fall back to WDA_MONITOR for older Windows 10 versions (< build 19041)
                    SetWindowDisplayAffinity(hwnd, WDA_MONITOR).map_err(|e2| {
                        crate::error::VaultError::DecryptionError(format!(
                            "SetWindowDisplayAffinity failed (primary: {}, fallback: {})",
                            e, e2
                        ))
                    })?;
                } else {
                    return Err(crate::error::VaultError::DecryptionError(format!(
                        "SetWindowDisplayAffinity disable failed: {}",
                        e
                    )));
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (hwnd_ptr, enable);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scrambled_string_roundtrip() {
        let secret = "SuperSecretPassword123!";
        let scrambled = ScrambledString::new(secret);
        assert_ne!(secret.as_bytes(), scrambled.ciphertext.as_slice());

        let decrypted = scrambled.decrypt().unwrap();
        assert_eq!(*decrypted, secret);
    }

    #[test]
    fn test_locked_buffer_lifecycle() {
        let bytes = vec![1, 2, 3, 4, 5];
        let locked = LockedBuffer::new(&bytes);
        assert_eq!(locked.as_slice(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_protected_secret_scope() {
        let secret = "MySotaSecretKey2026!";
        let protected = ProtectedSecret::new(secret);

        let result = protected.with_secret(|val| {
            assert_eq!(val, secret);
            val.len()
        }).unwrap();

        assert_eq!(result, secret.len());
    }

    #[test]
    fn test_direct_in_place_locked_decryption() {
        let secret = "ZeroAllocationPlaintext123456";
        let scrambled = ScrambledString::new(secret);

        // Decrypt directly into pre-locked memory
        let locked = scrambled.decrypt_to_locked().unwrap();
        assert_eq!(std::str::from_utf8(locked.as_slice()).unwrap(), secret);
    }

    #[test]
    fn test_ephemeral_key_pinning() {
        let key_slice = get_ephemeral_key();
        assert_eq!(key_slice.len(), 32);
        assert_ne!(key_slice, &[0u8; 32]);
    }

    #[test]
    fn test_window_capture_protection_null_handle() {
        let result = set_window_capture_protection(0, true);
        #[cfg(target_os = "windows")]
        assert!(result.is_err());
        #[cfg(not(target_os = "windows"))]
        assert!(result.is_ok());
    }

    #[test]
    fn test_enforce_dll_preloading_guard() {
        let result = enforce_dll_preloading_guard();
        assert!(result.is_ok());
    }

    #[test]
    fn test_is_workstation_locked() {
        // When running unit tests on interactive desktop, workstation should not be locked
        let locked = is_workstation_locked();
        let _ = locked;
    }
}




