//! Tauri IPC Commands — Modularized bridge between React frontend and yntra-vault-core.
//!
//! Re-exports all domain command functions for unified registration in `tauri::generate_handler!`.

pub mod auth;
pub mod entries;
pub mod attachments;
pub mod sync;
pub mod tools;
pub mod smartlogin;
pub mod updater;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use yntra_vault_core::vault::manager::VaultManager;

pub use auth::*;
pub use entries::*;
pub use attachments::*;
pub use sync::*;
pub use tools::*;
pub use smartlogin::*;
pub use updater::*;

/// Shared vault state across all IPC commands.
pub struct AppState {
    pub vault: Mutex<Option<VaultManager>>,
    pub minimize_to_tray: AtomicBool,
    pub lock_on_focus_loss: AtomicBool,
    pub lock_on_system_lock: AtomicBool,
    pub smart_login_cancel: Arc<AtomicBool>,
    pub smart_login_running: Arc<AtomicBool>,
    pub pairing_cancel: Arc<AtomicBool>,
    pub qr_pairing_session: Mutex<Option<yntra_vault_core::services::sync::QrPairingSession>>,
    pub pending_adopted_vault: Mutex<Option<yntra_vault_core::services::sync::PendingAdoptedVault>>,
}
