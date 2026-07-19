pub mod kdf;
pub mod cipher;
pub mod mem;
pub mod sharing;
pub mod tpm;
pub mod passkey;

pub use kdf::{MasterKey, SubKeys, derive_master_key, derive_subkeys};
pub use cipher::{
    encrypt_vault, decrypt_vault,
    encrypt_entry, decrypt_entry,
    compute_hmac, verify_hmac,
};
pub use mem::{LockedBuffer, ScrambledString, prevent_core_dumps};
pub use sharing::{split_secret, reconstruct_secret, parse_share, split_password, reconstruct_password_to_hex};
pub use tpm::{hardware_wrap_key, hardware_unwrap_key, write_session_token, read_session_token};
pub use passkey::{generate_passkey_pair, sign_assertion, verify_assertion};




