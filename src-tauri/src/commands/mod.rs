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
pub mod platform;
pub mod documents;
pub mod metadata;
pub use metadata::*;
pub use documents::*;
pub use platform::*;

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
    pub pairing_operation: Arc<yntra_vault_core::services::sync::lifecycle::OperationSlot>,
    pub sync_listener_operation: Arc<yntra_vault_core::services::sync::lifecycle::OperationSlot>,
    pub sync_client_operation: Arc<yntra_vault_core::services::sync::lifecycle::OperationSlot>,
    pub qr_pairing_session: Mutex<Option<yntra_vault_core::services::sync::QrPairingSession>>,
    pub pending_adopted_vault: Mutex<Option<yntra_vault_core::services::sync::PendingAdoptedVault>>,
}

impl AppState {
    pub fn cancel_network_work(&self) {
        self.pairing_operation.cancel();
        self.sync_listener_operation.cancel();
        self.sync_client_operation.cancel();
        self.smart_login_cancel.store(true, std::sync::atomic::Ordering::Release);
        yntra_vault_core::services::autotype::cancel_autotype();
        if let Ok(mut session) = self.qr_pairing_session.lock() { *session = None; }
        if let Ok(mut pending) = self.pending_adopted_vault.lock() { *pending = None; }
    }

    pub fn clear_session(&self) -> Result<(), String> {
        let mut vault = self.vault.lock().map_err(|_| "Vault state unavailable")?;
        if let Some(mut manager) = vault.take() { manager.lock(); }
        self.cancel_network_work();
        yntra_vault_core::services::favicon::clear_favicon_cache();
        Ok(())
    }

    /// Revalidate the exact session and USB policy after slow I/O outside the mutex.
    pub fn clear_session_if_disconnected(
        &self,
        snapshot: &yntra_vault_core::vault::presence::PresenceSnapshot,
        usb_missing: bool,
    ) -> Result<bool, String> {
        let mut vault = self.vault.lock().map_err(|_| "Vault state unavailable")?;
        if !vault.as_ref().is_some_and(|manager| snapshot.matches(manager)) { return Ok(false); }
        // This check is serialized with saves. An atomic replacement in progress
        // must not cause a false lock, nor may a stale .tmp suppress a real loss.
        if !usb_missing && snapshot.file_present() { return Ok(false); }
        if let Some(mut manager) = vault.take() { manager.lock(); }
        // Keep the vault lock until cancellation is visible to work on this session.
        self.cancel_network_work();
        yntra_vault_core::services::favicon::clear_favicon_cache();
        Ok(true)
    }
}
