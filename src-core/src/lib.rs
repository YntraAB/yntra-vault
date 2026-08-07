pub mod crypto;
pub mod vault;
pub mod totp;
pub mod generator;
pub mod breach;
pub mod error;

pub use error::VaultError;
pub type Result<T> = std::result::Result<T, VaultError>;

