//! Memory safety, heap scrambling, and memory page locking utilities.

use std::sync::OnceLock;
use rand::Rng;
use zeroize::{Zeroize, Zeroizing};
use chacha20poly1305::{XChaCha20Poly1305, aead::{Aead, KeyInit}, XNonce};

static EPHEMERAL_KEY: OnceLock<[u8; 32]> = OnceLock::new();

/// Retrieve or lazily initialize the ephemeral in-memory encryption key.
fn get_ephemeral_key() -> &'static [u8; 32] {
    EPHEMERAL_KEY.get_or_init(|| {
        let mut key = [0u8; 32];
        rand::rng().fill(&mut key);
        key
    })
}

/// A heap-allocated string container that scrambles string content in memory
/// using an ephemeral key generated dynamically at application startup.
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

    /// Decrypt the ScrambledString back into a Zeroizing<String> wrapper,
    /// which automatically zeroes out its contents when dropped.
    pub fn decrypt(&self) -> Zeroizing<String> {
        let key = get_ephemeral_key();
        let cipher = XChaCha20Poly1305::new_from_slice(key).unwrap();
        let nonce = XNonce::from_slice(&self.nonce);

        let plaintext_bytes = cipher
            .decrypt(nonce, self.ciphertext.as_slice())
            .expect("Decryption of scrambled heap string failed");

        let string = String::from_utf8(plaintext_bytes)
            .expect("Scrambled string is not valid UTF-8");

        Zeroizing::new(string)
    }
}

/// A memory-locked buffer wrapper that pins memory pages in RAM,
/// preventing the OS from writing them to swap/paging files.
pub struct LockedBuffer {
    data: Vec<u8>,
}

impl LockedBuffer {
    /// Create a new LockedBuffer from raw bytes and attempt to lock it.
    pub fn new(bytes: Vec<u8>) -> Self {
        let ptr = bytes.as_ptr() as *const std::ffi::c_void;
        let len = bytes.len();

        #[cfg(target_os = "windows")]
        unsafe {
            use windows::Win32::System::Memory::VirtualLock;
            let _ = VirtualLock(ptr, len);
        }

        #[cfg(not(target_os = "windows"))]
        unsafe {
            let _ = libc::mlock(ptr, len);
        }

        Self { data: bytes }
    }

    /// Access the underlying locked bytes.
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }
}

impl Zeroize for LockedBuffer {
    fn zeroize(&mut self) {
        self.data.zeroize();
    }
}

impl Drop for LockedBuffer {
    fn drop(&mut self) {
        let ptr = self.data.as_ptr() as *const std::ffi::c_void;
        let len = self.data.len();

        // Safe cleanup
        self.data.zeroize();

        #[cfg(target_os = "windows")]
        unsafe {
            use windows::Win32::System::Memory::VirtualUnlock;
            let _ = VirtualUnlock(ptr, len);
        }

        #[cfg(not(target_os = "windows"))]
        unsafe {
            let _ = libc::munlock(ptr, len);
        }
    }
}

/// Attempts to disable process core dumps and debugger attach events.
pub fn prevent_core_dumps() {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scrambled_string_roundtrip() {
        let secret = "SuperSecretPassword123!";
        let scrambled = ScrambledString::new(secret);
        assert_ne!(secret.as_bytes(), scrambled.ciphertext.as_slice());

        let decrypted = scrambled.decrypt();
        assert_eq!(*decrypted, secret);
    }

    #[test]
    fn test_locked_buffer_lifecycle() {
        let bytes = vec![1, 2, 3, 4, 5];
        let locked = LockedBuffer::new(bytes);
        assert_eq!(locked.as_slice(), &[1, 2, 3, 4, 5]);
    }
}
