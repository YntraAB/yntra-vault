pub mod kdf;
pub mod cipher;
pub mod mem;
pub mod sharing;
pub mod tpm;
pub mod passkey;
pub mod biometric;
pub mod hardware2fa;
pub mod clipboard;

pub use kdf::{MasterKey, SubKeys, EntryKey, derive_master_key, derive_master_key_with_keyfile, derive_subkeys, derive_per_entry_key};
pub use cipher::{
    encrypt_vault, decrypt_vault,
    encrypt_vault_with_aad, decrypt_vault_with_aad,
    encrypt_entry, decrypt_entry,
    encrypt_entry_with_aad, decrypt_entry_with_aad,
    compute_hmac, verify_hmac,
};
pub use mem::{LockedBuffer, ScrambledString, prevent_core_dumps};
pub use sharing::{split_secret, reconstruct_secret, parse_share, split_password, reconstruct_password_to_hex};
pub use tpm::{hardware_wrap_key, hardware_unwrap_key, write_session_token, read_session_token};
pub use passkey::{generate_passkey_pair, sign_assertion, verify_assertion};
pub use clipboard::{copy_to_clipboard_defended, clear_clipboard, schedule_auto_clear};




