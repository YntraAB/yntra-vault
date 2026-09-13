pub use yntra_crypto as crypto;
pub mod vault;
pub mod services;
pub mod totp;
pub mod generator;
pub mod breach;
pub mod smartlogin;
pub mod error;

pub use error::VaultError;
pub type Result<T> = std::result::Result<T, VaultError>;

