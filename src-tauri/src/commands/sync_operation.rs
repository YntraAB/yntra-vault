use super::AppState;
use serde::Serialize;
use tauri::{Emitter, Manager, State};
use uuid::Uuid;
use yntra_vault_core::{
    VaultError,
    services::sync::{MergeStats, review_transport},
};

#[derive(Clone, Serialize)]
pub struct SyncOperation {
    pub id: Uuid,
    pub transport: String,
    pub phase: String,
    pub error: Option<String>,
    pub peer_device_id: Option<Uuid>,
    #[serde(skip)]
    pub session: Uuid,
}

#[derive(Serialize)]
pub struct SyncOutcome {
    pub operation: SyncOperation,
    pub stats: MergeStats,
}

#[derive(Clone)]
pub struct Reporter {
    app: tauri::AppHandle,
    operation: SyncOperation,
}

impl Reporter {
    pub fn new(app: &tauri::AppHandle, session: Uuid, transport: &str) -> Self {
        let reporter = Self {
            app: app.clone(),
            operation: SyncOperation {
                id: Uuid::new_v4(),
                transport: transport.into(),
                phase: "connecting".into(),
                error: None,
                peer_device_id: None,
                session,
            },
        };
        reporter.phase("connecting");
        reporter
    }
    pub fn phase(&self, phase: &str) {
        self.publish(phase, None);
    }
    pub fn with_peer(mut self, peer: Option<Uuid>) -> Self {
        self.operation.peer_device_id = peer;
        self.phase("connecting");
        self
    }
    fn publish(&self, phase: &str, error: Option<String>) -> SyncOperation {
        let mut operation = self.operation.clone();
        operation.phase = phase.into();
        operation.error = error;
        let state = self.app.state::<AppState>();
        if let Ok(vault) = state.vault.lock() {
            if vault
                .as_ref()
                .is_some_and(|m| m.session_identity() == operation.session)
            {
                if let Ok(mut operations) = state.sync_progress.lock() {
                    operations.retain(|o| o.id != operation.id && o.session == operation.session);
                    if phase == "complete" && operation.peer_device_id.is_some() {
                        operations.retain(|previous| {
                            previous.peer_device_id != operation.peer_device_id
                                || !matches!(
                                    previous.phase.as_str(),
                                    "failed" | "incomplete" | "cancelled"
                                )
                        });
                    }
                    if operations.len() >= 8 {
                        operations.remove(0);
                    }
                    operations.push(operation.clone());
                    let _ = self.app.emit("sync-operation", &operation);
                }
            }
        }
        operation
    }
    pub fn finish(
        &self,
        result: Result<MergeStats, String>,
        stored: Option<Uuid>,
    ) -> Result<SyncOutcome, String> {
        match result {
            Ok(stats) => {
                if let Some(id) = stored {
                    let confirmation = self.acknowledge(id, stats.peer_device_id);
                    if let Err(error) = confirmation {
                        let error = format!(
                            "Both copies were stored, but completion could not be recorded: {error}"
                        );
                        self.publish("incomplete", Some(error.clone()));
                        return Err(error);
                    }
                }
                let mut completed = self.clone();
                completed.operation.peer_device_id =
                    stats.peer_device_id.or(self.operation.peer_device_id);
                Ok(SyncOutcome {
                    operation: completed.publish("complete", None),
                    stats,
                })
            }
            Err(error) => {
                let phase = if stored.is_some()
                    || error.contains("Peer saved")
                    || error.contains("Remote stored")
                    || error.contains("confirmation is missing")
                    || error.contains("could not confirm saving")
                {
                    "incomplete"
                } else if error.to_lowercase().contains("cancel")
                    || self.operation.transport == "p2p_listener"
                        && error.contains("No peer connected")
                {
                    "cancelled"
                } else {
                    "failed"
                };
                self.publish(phase, Some(error.clone()));
                Err(error)
            }
        }
    }
    fn acknowledge(&self, id: Uuid, peer: Option<Uuid>) -> yntra_vault_core::Result<()> {
        let state = self.app.state::<AppState>();
        let mut vault = state.vault.lock().map_err(|_| VaultError::VaultLocked)?;
        let manager = vault
            .as_mut()
            .filter(|m| m.session_identity() == self.operation.session)
            .ok_or(VaultError::VaultLocked)?;
        review_transport::acknowledge_with_peer(manager, id, peer)
    }
}

#[tauri::command]
pub fn get_sync_operations(state: State<'_, AppState>) -> Result<Vec<SyncOperation>, String> {
    let vault = state.vault.lock().map_err(|_| "Vault state unavailable")?;
    let Some(manager) = vault.as_ref() else {
        return Ok(vec![]);
    };
    let mut operations: Vec<_> = state
        .sync_progress
        .lock()
        .map_err(|_| "Synchronization state unavailable")?
        .iter()
        .filter(|o| o.session == manager.session_identity())
        .cloned()
        .collect();
    let pending = manager
        .data
        .settings
        .sync_pending_receipts
        .iter()
        .map(|(id, receipt)| (*id, receipt.peer_device_id))
        .chain(manager.data.settings.sync_pending.map(|id| (id, None)));
    for (id, peer_device_id) in pending {
        if !operations.iter().any(|o| {
            (o.id == id || peer_device_id.is_some() && o.peer_device_id == peer_device_id)
                && matches!(o.phase.as_str(), "incomplete" | "saving" | "waiting_peer")
        }) {
            operations.push(SyncOperation {
                id,
                transport: "sync".into(),
                phase: "incomplete".into(),
                error: None,
                peer_device_id,
                session: manager.session_identity(),
            });
        }
    }
    Ok(operations)
}
