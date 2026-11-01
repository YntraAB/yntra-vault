use super::{AppState, SyncOutcome, sync_operation::Reporter};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager, State};
use yntra_vault_core::{
    VaultError,
    crypto::SubKeys,
    services::sync::{
        self,
        review::{self, SyncDecision, SyncPolicy, SyncPreview},
        review_transport::{self, Snapshot},
    },
    vault::VaultData,
};

pub struct PendingReview {
    preview: SyncPreview,
    local: Snapshot,
    remote: Snapshot,
    session: uuid::Uuid,
    sender: mpsc::Sender<Vec<SyncDecision>>,
}

fn error(message: &str) -> VaultError {
    VaultError::SyncError(message.into())
}

pub(super) fn wait_for_review(
    app: &tauri::AppHandle,
    keys: &SubKeys,
    cancel: &Arc<AtomicBool>,
    session: uuid::Uuid,
    local: &Snapshot,
    remote: &Snapshot,
    background: bool,
    reporter: &Reporter,
) -> yntra_vault_core::Result<(uuid::Uuid, VaultData)> {
    let mut preview = review::preview(&local.data, &remote.data, keys)?;
    {
        let state = app.state::<AppState>();
        let vault = state
            .vault
            .lock()
            .map_err(|_| error("Vault state unavailable"))?;
        let manager = vault
            .as_ref()
            .filter(|manager| manager.session_identity() == session)
            .ok_or(VaultError::VaultLocked)?;
        for change in &mut preview.changes {
            for (id, name) in [
                (change.local_device, &mut change.local_device_name),
                (change.remote_device, &mut change.remote_device_name),
            ] {
                if let Some(device_name) = id.and_then(|id| manager.trusted_device_name(id)) {
                    *name = Some(device_name);
                }
            }
        }
    }
    if local
        .observed_at
        .signed_duration_since(remote.observed_at)
        .num_seconds()
        .unsigned_abs()
        > 120
    {
        for change in &mut preview.changes {
            change.time_order_reliable = false;
        }
    }
    preview.background = background;
    let (suggested, needs_review) = review::apply_policy(&mut preview, &local.data.settings.sync_policy, &remote.data.settings.sync_policy);
    if !needs_review {
        return Ok((
            preview.id,
            review::resolve(
                &local.data,
                &remote.data,
                &preview,
                &suggested,
                keys,
                session,
            )?,
        ));
    }
    reporter.phase("reviewing");
    let (sender, receiver) = mpsc::channel();
    let state = app.state::<AppState>();
    {
        let mut pending = state
            .sync_review
            .lock()
            .map_err(|_| error("Synchronization state unavailable"))?;
        if pending.is_some() {
            return Err(error("Another synchronization is waiting for review"));
        }
        *pending = Some(PendingReview {
            preview: preview.clone(),
            local: local.clone(),
            remote: remote.clone(),
            session,
            sender,
        });
    }
    let _ = app.emit("sync-review-ready", &preview);
    let deadline = Instant::now() + Duration::from_secs(900);
    let answer = loop {
        if cancel.load(Ordering::Acquire) || Instant::now() >= deadline {
            break Err(error("Synchronization review cancelled or expired"));
        }
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(decisions) => {
                break review::resolve(
                    &local.data,
                    &remote.data,
                    &preview,
                    &decisions,
                    keys,
                    session,
                );
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break Err(error("Synchronization cancelled"));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    };
    if let Ok(mut pending) = state.sync_review.lock() {
        if pending.as_ref().is_some_and(|p| p.preview.id == preview.id) {
            *pending = None;
        }
    }
    let _ = app.emit("sync-review-closed", preview.id);
    answer.map(|data| (preview.id, data))
}

fn capture(state: &AppState, session: uuid::Uuid) -> yntra_vault_core::Result<Snapshot> {
    let vault = state
        .vault
        .lock()
        .map_err(|_| error("Vault state unavailable"))?;
    let manager = vault.as_ref().ok_or(VaultError::VaultLocked)?;
    if manager.session_identity() != session {
        return Err(error("Active vault changed during synchronization"));
    }
    Snapshot::capture(manager)
}

fn commit(
    state: &AppState,
    session: uuid::Uuid,
    expected: &str,
    data: VaultData,
    operation: uuid::Uuid,
    peer: Option<uuid::Uuid>,
) -> yntra_vault_core::Result<()> {
    let mut vault = state
        .vault
        .lock()
        .map_err(|_| error("Vault state unavailable"))?;
    let manager = vault.as_mut().ok_or(VaultError::VaultLocked)?;
    if manager.session_identity() != session
        || review::revision(&manager.data)? != expected
        || manager.data.metadata.id != data.metadata.id
    {
        return Err(error("Vault changed during review; compare again"));
    }
    review_transport::apply_with_peer(manager, expected, data, operation, peer)
}

#[tauri::command]
pub fn get_sync_review(state: State<'_, AppState>) -> Result<Option<SyncPreview>, String> {
    Ok(state
        .sync_review
        .lock()
        .map_err(|_| "Synchronization state unavailable")?
        .as_ref()
        .map(|p| p.preview.clone()))
}

#[tauri::command]
pub fn inspect_sync_field(
    id: String,
    key: String,
    remote: bool,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let vault = state.vault.lock().map_err(|_| "Vault state unavailable")?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let pending = state
        .sync_review
        .lock()
        .map_err(|_| "Synchronization state unavailable")?;
    let pending = pending
        .as_ref()
        .filter(|p| p.preview.id.to_string() == id && p.session == manager.session_identity())
        .ok_or("Review expired")?;
    let change = pending
        .preview
        .changes
        .iter()
        .find(|c| c.key == key)
        .ok_or("Unknown change")?;
    review::inspect(
        if remote {
            &pending.remote.data
        } else {
            &pending.local.data
        },
        change.entry_id,
        &change.field,
        manager.get_subkeys().map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn approve_sync_review(
    id: String,
    decisions: Vec<SyncDecision>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let vault = state.vault.lock().map_err(|_| "Vault state unavailable")?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let pending = state
        .sync_review
        .lock()
        .map_err(|_| "Synchronization state unavailable")?;
    let pending = pending
        .as_ref()
        .filter(|p| p.preview.id.to_string() == id && p.session == manager.session_identity())
        .ok_or("Review expired")?;
    if review::revision(&manager.data).map_err(|e| e.to_string())? != pending.local.revision {
        return Err("Vault changed during review; cancel and compare again".into());
    }
    review::resolve(
        &pending.local.data,
        &pending.remote.data,
        &pending.preview,
        &decisions,
        manager.get_subkeys().map_err(|e| e.to_string())?,
        manager.session_identity(),
    )
    .map_err(|e| e.to_string())?;
    pending
        .sender
        .send(decisions)
        .map_err(|_| "Review expired".into())
}

#[tauri::command]
pub fn cancel_sync_review(state: State<'_, AppState>) -> Result<(), String> {
    *state
        .sync_review
        .lock()
        .map_err(|_| "Synchronization state unavailable")? = None;
    state.sync_client_operation.cancel();
    Ok(())
}

#[tauri::command]
pub fn get_sync_policy(state: State<'_, AppState>) -> Result<SyncPolicy, String> {
    let vault = state.vault.lock().map_err(|_| "Vault state unavailable")?;
    Ok(vault
        .as_ref()
        .ok_or("Vault is locked")?
        .data
        .settings
        .sync_policy
        .clone())
}

#[tauri::command]
pub fn set_sync_policy(policy: SyncPolicy, state: State<'_, AppState>) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|_| "Vault state unavailable")?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    let previous = manager.data.settings.sync_policy.clone();
    manager.data.settings.sync_policy = policy;
    if let Err(e) = manager.save() {
        manager.data.settings.sync_policy = previous;
        return Err(e.to_string());
    }
    Ok(())
}

#[tauri::command]
pub async fn run_p2p_sync_listener(
    app: tauri::AppHandle,
    listen_addr: String,
    db_path: String,
    state: State<'_, AppState>,
) -> Result<SyncOutcome, String> {
    let operation = state.sync_listener_operation.begin()?;
    let (keys, salt, trusted, session) = {
        let mut vault = state.vault.lock().map_err(|_| "Vault state unavailable")?;
        let manager = vault.as_mut().ok_or("Vault is locked")?;
        if !db_path.is_empty() && manager.path != std::path::Path::new(&db_path) {
            return Err("Synchronization is restricted to the active vault".into());
        }
        (
            manager.get_subkeys().map_err(|e| e.to_string())?.clone(),
            manager.salt(),
            manager.data.settings.trusted_devices.clone(),
            manager.session_identity(),
        )
    };
    let reporter = Reporter::new(&app, session, "p2p_listener");
    tokio::task::spawn_blocking(move || {
        let _lease = operation;
        let stored = std::cell::Cell::new(None);
        let result = review_transport::listen(
            &listen_addr,
            &keys,
            &salt,
            &trusted,
            _lease.cancel.clone(),
            |_| {
                reporter.phase("listening");
                let _ = app.emit("p2p-listener-ready", ());
            },
            || {
                reporter.phase("awaiting_review");
                let state = app.state::<AppState>();
                let vault = state
                    .vault
                    .lock()
                    .map_err(|_| error("Vault state unavailable"))?;
                let manager = vault
                    .as_ref()
                    .filter(|m| m.session_identity() == session)
                    .ok_or(VaultError::VaultLocked)?;
                Snapshot::capture_with_trust(manager, &trusted)
            },
            |expected, data, operation, peer| {
                if _lease.cancel.load(Ordering::Acquire) {
                    return Err(error("Synchronization cancelled"));
                }
                reporter.phase("saving");
                commit(&app.state::<AppState>(), session, expected, data, operation, Some(peer))?;
                stored.set(Some(operation));
                reporter.phase("waiting_peer");
                let _ = app.emit("vault-content-changed", ());
                Ok(())
            },
        );
        reporter.finish(result.map_err(|e| e.to_string()), stored.get())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn run_p2p_sync_client(
    app: tauri::AppHandle,
    server_addr: String,
    db_path: String,
    device_id: Option<String>,
    background: Option<bool>,
    state: State<'_, AppState>,
) -> Result<SyncOutcome, String> {
    let operation = state.sync_client_operation.begin()?;
    let (keys, salt, trusted, session, local) = {
        let mut vault = state.vault.lock().map_err(|_| "Vault state unavailable")?;
        let manager = vault.as_mut().ok_or("Vault is locked")?;
        if !db_path.is_empty() && manager.path != std::path::Path::new(&db_path) {
            return Err("Synchronization is restricted to the active vault".into());
        }
        (
            manager.get_subkeys().map_err(|e| e.to_string())?.clone(),
            manager.salt(),
            manager.data.settings.trusted_devices.clone(),
            manager.session_identity(),
            Snapshot::capture(manager).map_err(|e| e.to_string())?,
        )
    };
    let expected_peer = device_id
        .map(|id| uuid::Uuid::parse_str(&id))
        .transpose()
        .map_err(|_| "Invalid selected device identity")?;
    let reporter = Reporter::new(&app, session, "p2p_client").with_peer(expected_peer);
    tokio::task::spawn_blocking(move || {
        let _lease = operation;
        let stored = std::cell::Cell::new(None);
        let result = review_transport::connect(
            &server_addr,
            &keys,
            &salt,
            &trusted,
            _lease.cancel.clone(),
            expected_peer,
            local,
            |local, remote| {
                reporter.phase("comparing");
                let result = wait_for_review(
                    &app,
                    &keys,
                    &_lease.cancel,
                    session,
                    local,
                    remote,
                    background.unwrap_or(false),
                    &reporter,
                );
                if result.is_ok() {
                    reporter.phase("preparing");
                }
                result
            },
            |expected| {
                if capture(&app.state::<AppState>(), session)?.revision == expected {
                    Ok(())
                } else {
                    Err(error("Vault changed during review"))
                }
            },
            |expected, data, operation, peer| {
                if _lease.cancel.load(Ordering::Acquire) {
                    return Err(error("Synchronization cancelled"));
                }
                reporter.phase("saving");
                commit(&app.state::<AppState>(), session, expected, data, operation, Some(peer))?;
                stored.set(Some(operation));
                reporter.phase("waiting_peer");
                let _ = app.emit("vault-content-changed", ());
                Ok(())
            },
        );
        reporter.finish(result.map_err(|e| e.to_string()), stored.get())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn webdav_sync(
    app: tauri::AppHandle,
    url: String,
    username: String,
    password: Option<String>,
    background: Option<bool>,
    state: State<'_, AppState>,
) -> Result<SyncOutcome, String> {
    let operation = Arc::new(state.sync_client_operation.begin()?);
    let password = password.map(zeroize::Zeroizing::new);
    let (keys, salt, session, local) = {
        let mut vault = state.vault.lock().map_err(|_| "Vault state unavailable")?;
        let manager = vault.as_mut().ok_or("Vault is locked")?;
        (
            manager.get_subkeys().map_err(|e| e.to_string())?.clone(),
            manager.salt(),
            manager.session_identity(),
            Snapshot::capture(manager).map_err(|e| e.to_string())?,
        )
    };
    let reporter = Reporter::new(&app, session, "webdav");
    let mut stored = None;
    let result = async {
        let snapshot = sync::webdav_download_snapshot(
            &url,
            &username,
            password.as_deref().map(|v| v.as_str()),
        )
        .await
        .map_err(|e| e.to_string())?;
        let remote = if let Some((bytes, _)) = &snapshot {
            sync::decrypt_remote_vault_bytes_checked(bytes, &keys, Some(&salt))
                .map_err(|e| e.to_string())?
        } else {
            let mut empty = local.data.clone();
            empty.entries.clear();
            empty.tags.clear();
            empty.trash.clear();
            empty.settings.sync = Default::default();
            empty
        };
        let remote = Snapshot {
            revision: review::revision(&remote).map_err(|e| e.to_string())?,
            data: remote,
            observed_at: local.observed_at,
        };
        reporter.phase("comparing");
        let review_reporter = reporter.clone();
        let (review_app, review_keys, review_operation, review_local) =
            (app.clone(), keys.clone(), operation.clone(), local.clone());
        let (operation_id, merged) = tokio::task::spawn_blocking(move || {
            wait_for_review(
                &review_app,
                &review_keys,
                &review_operation.cancel,
                session,
                &review_local,
                &remote,
                background.unwrap_or(false),
                &review_reporter,
            )
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
        if operation.cancel.load(Ordering::Acquire) {
            return Err("Synchronization cancelled".into());
        }
        if capture(&state, session)
            .map_err(|e| e.to_string())?
            .revision
            != local.revision
        {
            return Err("Vault changed during review; compare again".into());
        }
        let stats =
            review_transport::stats(&local.data, &merged, &keys).map_err(|e| e.to_string())?;
        let directory = tempfile::tempdir().map_err(|e| e.to_string())?;
        let path = directory.path().join("sync.vdb");
        std::fs::write(
            &path,
            review_transport::encode_vault(&merged, &keys, salt).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        reporter.phase("preparing");
        sync::webdav_upload_conditional(
            &url,
            &username,
            password.as_deref().map(|v| v.as_str()),
            &path,
            snapshot.as_ref().map(|(_, etag)| etag.as_str()),
            snapshot.is_none(),
        )
        .await
        .map_err(|e| format!("Remote changed or upload failed; compare again: {e}"))?;
        if operation.cancel.load(Ordering::Acquire) {
            return Err("Remote stored the result; local confirmation is pending".into());
        }
        reporter.phase("saving");
        commit(&state, session, &local.revision, merged, operation_id, None)
            .map_err(|e| format!("Remote stored the result; local save failed: {e}"))?;
        stored = Some(operation_id);
        let _ = app.emit("vault-content-changed", ());
        Ok(stats)
    }
    .await;
    reporter.finish(result, stored)
}
