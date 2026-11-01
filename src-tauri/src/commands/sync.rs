use tauri::State;

use super::AppState;

async fn stop_operation(slot: &yntra_vault_core::services::sync::lifecycle::OperationSlot) -> Result<(), String> {
    slot.cancel();
    for _ in 0..250 {
        if slot.is_idle() { return Ok(()); }
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    }
    Err("Previous connection is still closing. Retry shortly.".into())
}

#[tauri::command]
pub async fn cancel_p2p_sync_listener(state: State<'_ , AppState>) -> Result<(), String> {
    stop_operation(&state.sync_listener_operation).await
}


#[tauri::command]
pub async fn webdav_test_connection(
    url: String,
    username: String,
    password: Option<String>,
) -> Result<(), String> {
    yntra_vault_core::services::sync::webdav_test_connection(
        &url,
        &username,
        password.as_deref(),
    ).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn webdav_upload(
    url: String,
    username: String,
    password: Option<String>,
    db_path: String,
    if_match_etag: Option<String>,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let bytes = {
        let vault = state.vault.lock().map_err(|e| e.to_string())?;
        active_upload_snapshot(vault.as_ref(), std::path::Path::new(&db_path))?
    };
    yntra_vault_core::services::sync::webdav_upload_bytes(
        &url,
        &username,
        password.as_deref(),
        bytes,
        if_match_etag.as_deref(),
        false,
    ).await.map_err(|e| e.to_string())
}

fn active_upload_snapshot(
    manager: Option<&yntra_vault_core::vault::VaultManager>,
    target: &std::path::Path,
) -> Result<Vec<u8>, String> {
    let manager = manager.ok_or("Vault is locked")?;
    let same_path = manager.path == target || matches!(
        (std::fs::canonicalize(&manager.path), std::fs::canonicalize(target)),
        (Ok(a), Ok(b)) if a == b
    );
    if !same_path { return Err("WebDAV upload is restricted to the active vault".into()); }
    manager.sync_bytes().map_err(|e| e.to_string())
}

#[cfg(test)]
mod upload_tests {
    use super::*;

    #[test]
    fn upload_requires_active_session_and_uses_its_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("active.vdb");
        assert!(active_upload_snapshot(None, &path).unwrap_err().contains("locked"));
        let manager = yntra_vault_core::vault::VaultManager::create("Upload test", "test-password-only", &path).unwrap();
        let other = dir.path().join("unrelated.vdb");
        std::fs::write(&other, b"private file").unwrap();
        assert!(active_upload_snapshot(Some(&manager), &other).unwrap_err().contains("active vault"));
        let bytes = active_upload_snapshot(Some(&manager), &path).unwrap();
        assert!(!yntra_vault_core::vault::storage::is_protected(&bytes));
        yntra_vault_core::vault::format::VaultFile::from_bytes(&bytes).unwrap();
    }
}

#[tauri::command]
pub async fn webdav_download(
    app: tauri::AppHandle,
    url: String,
    username: String,
    password: Option<String>,
    dest_db_path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use tauri::Emitter;

    let target = std::path::Path::new(&dest_db_path);
    let (session, revision) = {
        let vault = state.vault.lock().map_err(|e| e.to_string())?;
        let manager = vault.as_ref().ok_or("Vault is locked")?;
        let same_path = manager.path == target || matches!((std::fs::canonicalize(&manager.path), std::fs::canonicalize(target)), (Ok(a), Ok(b)) if a == b);
        if !same_path {
            return Err("WebDAV restore is restricted to the active vault".into());
        }
        (manager.session_identity(), yntra_vault_core::services::sync::restore_revision(target).map_err(|e| e.to_string())?)
    };
    let bytes = yntra_vault_core::services::sync::webdav_download_bytes(&url, &username, password.as_deref()).await.map_err(|e| e.to_string())?;
    let mut vault_guard = state.vault.lock().map_err(|e| e.to_string())?;
    let mgr = vault_guard.as_mut().filter(|manager| manager.session_identity() == session).ok_or("Vault session changed during download")?;
    yntra_vault_core::services::sync::restore_downloaded(&bytes, target, revision).map_err(|e| e.to_string())?;
    {
        if let Err(err) = mgr.reload() {
                // If reload fails (e.g. restored database has different credentials or salt),
                // lock in-memory state to prevent stale data from overwriting the restored file.
                mgr.lock();
                *vault_guard = None;
                let _ = yntra_vault_core::crypto::clear_clipboard();
                let _ = app.emit("vault-locked", ());
                return Err(format!(
                    "Database was restored, but could not be decrypted with current session credentials ({}). Vault has been locked.",
                    err
                ));
            }
    }

    Ok(())
}

#[tauri::command]
pub fn get_local_ip() -> Result<Option<String>, String> {
    Ok(yntra_vault_core::services::sync::get_local_lan_ip().map(|ip| ip.to_string()))
}

#[tauri::command]
pub fn get_local_ips() -> Result<Vec<String>, String> {
    Ok(yntra_vault_core::services::sync::get_local_lan_ips().into_iter().map(|ip| ip.to_string()).collect())
}

#[tauri::command]
pub async fn scan_p2p_discovery(
    timeout_ms: Option<u64>,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let subkeys = {
        let vault = state.vault.lock().map_err(|e| e.to_string())?;
        let manager = vault.as_ref().ok_or("Vault is locked")?;
        manager.get_subkeys().map_err(|e| e.to_string())?.clone()
    };

    let discovery_id = yntra_vault_core::services::sync::compute_p2p_discovery_id(&subkeys);
    let dur = std::time::Duration::from_millis(timeout_ms.unwrap_or(3000));
    let local_lan_ips = yntra_vault_core::services::sync::get_local_lan_ips();

    let res = tokio::task::spawn_blocking(move || {
        yntra_vault_core::services::sync::listen_discovery_beacon(&discovery_id, dur)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    // Filter out self-echo across all active network adapter IPs
    if let Some(ref peer_addr) = res
        && (peer_addr.ip().is_loopback() || local_lan_ips.iter().any(|lip| *lip == peer_addr.ip())) {
            return Ok(None);
        }

    Ok(res.map(|addr| addr.to_string()))
}

#[tauri::command]
pub fn generate_pairing_code() -> Result<String, String> {
    Ok(yntra_vault_core::services::sync::pairing::generate_pairing_code())
}

#[tauri::command]
pub fn get_local_device_info() -> Result<yntra_vault_core::services::sync::pairing::DeviceInfo, String> {
    Ok(yntra_vault_core::services::sync::pairing::resolve_local_device_info(None))
}

#[tauri::command]
pub async fn get_trusted_devices(
    state: State<'_, AppState>,
) -> Result<Vec<yntra_vault_core::vault::types::TrustedDevice>, String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    Ok(manager.get_trusted_devices())
}

#[tauri::command]
pub async fn revoke_trusted_device(
    device_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let dev_uuid = uuid::Uuid::parse_str(&device_id).map_err(|e| e.to_string())?;
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    manager.revoke_trusted_device(dev_uuid).map_err(|e| e.to_string())?;
    state.sync_listener_operation.cancel();
    state.sync_client_operation.cancel();
    *state.sync_review.lock().map_err(|e| e.to_string())? = None;
    Ok(())
}

#[tauri::command]
pub async fn start_pairing_host(
    app: tauri::AppHandle,
    listen_addr: String,
    password: String,
    pairing_code: String,
    device_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<yntra_vault_core::services::sync::PairingStats, String> {
    let password = zeroize::Zeroizing::new(password);
    let pairing_code = zeroize::Zeroizing::new(pairing_code);
    let operation = std::sync::Arc::new(state.pairing_operation.begin()?);
    let (host_db_path, session) = {
        let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
        if operation.cancel.load(std::sync::atomic::Ordering::Acquire) { return Err("Pairing cancelled".into()); }
        let manager = vault.as_mut().ok_or("Vault is locked")?;
        manager.save().map_err(|e| e.to_string())?;
        (manager.path.clone(), manager.session_identity())
    };

    let worker_operation = operation.clone();
    let cancel_flag = operation.cancel.clone();

    let (stats, merged_data) = tokio::task::spawn_blocking(move || {
        let _operation = worker_operation;
        yntra_vault_core::services::sync::pairing::run_p2p_pairing_host_reviewed(
            &listen_addr,
            &password,
            &pairing_code,
            &host_db_path,
            std::time::Duration::from_secs(180),
            device_name,
            Some(cancel_flag.clone()),
            |manager, incoming| {
                let local = yntra_vault_core::services::sync::review_transport::Snapshot::capture(manager)?;
                let remote = yntra_vault_core::services::sync::review_transport::Snapshot { revision: yntra_vault_core::services::sync::review::revision(incoming)?, data: incoming.clone(), observed_at: local.observed_at };
                let reporter = super::sync_operation::Reporter::new(&app, session, "pairing");
                super::sync_review::wait_for_review(&app, manager.get_subkeys()?, &cancel_flag, session, &local, &remote, false, &reporter).map(|(_, data)| data)
            },
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    {
        let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
        if operation.cancel.load(std::sync::atomic::Ordering::Acquire) { return Err("Pairing cancelled".into()); }
        if let Some(manager) = vault.as_mut() {
            if manager.is_unlocked() && manager.data.metadata.id == merged_data.metadata.id {
                // Both managers persist edits with revision checks. The disk is authoritative.
                manager.reload().map_err(|e| e.to_string())?;
            }
        }
    }

    Ok(stats)
}

#[tauri::command]
pub async fn cancel_pairing_host(state: State<'_, AppState>) -> Result<(), String> {
    stop_operation(&state.pairing_operation).await
}

#[tauri::command]
pub async fn start_pairing_client(
    app: tauri::AppHandle,
    server_addr: String,
    password: String,
    pairing_code: String,
    db_path: String,
    device_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<yntra_vault_core::services::sync::PairingStats, String> {
    use yntra_vault_core::services::sync::pairing::ClientPairingMode;

    let vault_dir = super::auth::vault_storage_dir(&app)?;
    let operation = std::sync::Arc::new(state.pairing_operation.begin()?);

    // CRITICAL SECURITY INVARIANT:
    // A client can only transmit and merge an existing local vault if that vault is
    // actively unlocked in the current session (`state.vault` is Some).
    // If no vault is currently unlocked (e.g. user is on the VaultSelect screen or locked),
    // pairing MUST strictly operate in Adopt mode. In Adopt mode, the client reads ZERO
    // local files from disk, transmits ZERO entries to the host, and saves the received
    // remote vault into a dedicated, collision-free file.
    let active_vault_path = {
        let vault_guard = state.vault.lock().map_err(|e| e.to_string())?;
        if operation.cancel.load(std::sync::atomic::Ordering::Acquire) { return Err("Pairing cancelled".into()); }
        vault_guard.as_ref().map(|m| m.path.clone())
    };

    let mode = match active_vault_path {
        Some(active_path) => {
            if db_path.trim().is_empty() || std::path::Path::new(&db_path) == active_path {
                ClientPairingMode::ExistingVault { path: active_path }
            } else {
                return Err("Pairing is restricted to the active vault".into());
            }
        }
        None => {
            // Client is not authenticated with any vault. Under no circumstances should
            // any local vault file be read or transmitted!
            ClientPairingMode::AdoptIntoDir { target_dir: vault_dir }
        }
    };

    let worker_operation = operation.clone();
    let pass_clone = zeroize::Zeroizing::new(password.clone());
    let (stats, _merged_data) = tokio::task::spawn_blocking(move || {
        let _operation = worker_operation;
        yntra_vault_core::services::sync::pairing::run_p2p_pairing_client_cancellable(
            &server_addr,
            &pass_clone,
            &pairing_code,
            mode,
            device_name,
            Some(_operation.cancel.clone()),
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    if operation.cancel.load(std::sync::atomic::Ordering::Acquire) { return Err("Pairing cancelled".into()); }
    // Open newly adopted vault or update active manager in AppState
    if let Some(ref saved_path_str) = stats.vault_path {
        let saved_path = std::path::PathBuf::from(saved_path_str);
        let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
        if vault.as_ref().is_some_and(|manager| manager.path != saved_path) {
            return Err("Active vault changed during pairing".into());
        }
        // Pairing may change the root keys; reopen instead of retaining old keys.
        let manager = yntra_vault_core::vault::VaultManager::open(&saved_path, &password).map_err(|e| e.to_string())?;
        if operation.cancel.load(std::sync::atomic::Ordering::Acquire) { return Err("Pairing cancelled".into()); }
        *vault = Some(manager);
        if operation.cancel.load(std::sync::atomic::Ordering::Acquire) {
            if let Some(mut manager) = vault.take() { manager.lock(); }
            return Err("Pairing cancelled".into());
        }
    }

    Ok(stats)
}

#[tauri::command]
pub async fn scan_pairing_discovery(
    password: String,
    pairing_code: String,
    timeout_ms: Option<u64>,
) -> Result<Option<String>, String> {
    let discovery_id = yntra_vault_core::services::sync::pairing::compute_pairing_beacon_id(&password, &pairing_code)
        .map_err(|e| e.to_string())?;
    let dur = std::time::Duration::from_millis(timeout_ms.unwrap_or(3000));

    let res = tokio::task::spawn_blocking(move || {
        yntra_vault_core::services::sync::pairing::listen_pairing_beacon(&discovery_id, dur)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    Ok(res.map(|addr| addr.to_string()))
}

#[tauri::command]
pub fn generate_qr_pairing_session(
    device_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<yntra_vault_core::services::sync::QrSessionInfo, String> {
    let operation = state.pairing_operation.begin()?;
    // Serialize generation with native lock cleanup so a late QR cannot restore
    // a pairing secret after that cleanup cleared the session.
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    if !vault.as_ref().is_some_and(|manager| manager.is_unlocked()) {
        return Err("Vault is locked".into());
    }
    let local_ips: Vec<String> = yntra_vault_core::services::sync::get_local_lan_ips()
        .into_iter()
        .filter_map(|ip| {
            if let std::net::IpAddr::V4(ipv4) = ip {
                if !ipv4.is_loopback() {
                    Some(ipv4.to_string())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    yntra_vault_core::services::network::ensure_allowed().map_err(|e| e.to_string())?;
    // Reserve before showing the QR: an advertised code always has a bound socket.
    let listener = std::net::TcpListener::bind("0.0.0.0:5324")
        .or_else(|_| std::net::TcpListener::bind("0.0.0.0:0"))
        .map_err(|e| format!("Unable to prepare QR listener: {}", e))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let (mut session, info) = yntra_vault_core::services::sync::generate_qr_pairing_session(
        local_ips,
        port,
        device_name,
    );

    session.listener = Some(std::sync::Arc::new(listener));
    let mut guard = state.qr_pairing_session.lock().map_err(|e| e.to_string())?;
    if operation.cancel.load(std::sync::atomic::Ordering::Acquire) { return Err("Pairing cancelled".into()); }
    yntra_vault_core::services::network::ensure_allowed().map_err(|e| e.to_string())?;
    *guard = Some(session);

    Ok(info)
}

#[tauri::command]
pub async fn start_qr_pairing_host(
    password: String,
    include_password: bool,
    device_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<yntra_vault_core::services::sync::PairingStats, String> {
    let operation = std::sync::Arc::new(state.pairing_operation.begin()?);
    // Keep the lock order vault -> QR session, matching native cleanup. This
    // prevents a lock/QR callback from deadlocking while this operation starts.
    let (session, host_db_path) = {
        let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
        if operation.cancel.load(std::sync::atomic::Ordering::Acquire) { return Err("Pairing cancelled".into()); }
        let manager = vault.as_mut().ok_or("Valvet är låst")?;
        manager.save().map_err(|e| e.to_string())?;
        let mut guard = state.qr_pairing_session.lock().map_err(|e| e.to_string())?;
        let session = guard.take().ok_or("Ingen aktiv QR-parningssession hittades. Generera en QR-kod först.")?;
        (session, manager.path.clone())
    };

    let worker_operation = operation.clone();
    let cancel_flag = operation.cancel.clone();

    let (stats, merged_data) = tokio::task::spawn_blocking(move || {
        let _operation = worker_operation;
        yntra_vault_core::services::sync::run_p2p_qr_pairing_host(
            &session,
            &host_db_path,
            &password,
            include_password,
            std::time::Duration::from_secs(90),
            device_name,
            Some(cancel_flag),
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    {
        let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
        if operation.cancel.load(std::sync::atomic::Ordering::Acquire) { return Err("Pairing cancelled".into()); }
        if let Some(manager) = vault.as_mut() {
            if manager.is_unlocked() && manager.data.metadata.id == merged_data.metadata.id {
                // Both managers persist edits with revision checks. The disk is authoritative.
                manager.reload().map_err(|e| e.to_string())?;
            }
        }
    }

    Ok(stats)
}

#[tauri::command]
pub async fn cancel_qr_pairing_host(state: State<'_, AppState>) -> Result<(), String> {
    state.pairing_operation.cancel();
    if let Ok(mut guard) = state.qr_pairing_session.lock() {
        *guard = None;
    }
    if let Ok(mut guard) = state.pending_adopted_vault.lock() {
        *guard = None;
    }
    stop_operation(&state.pairing_operation).await
}

#[tauri::command]
pub async fn start_qr_pairing_client(
    app: tauri::AppHandle,
    qr_payload: String,
    device_name: Option<String>,
    password: Option<String>,
    state: State<'_, AppState>,
) -> Result<yntra_vault_core::services::sync::QrClientPairingResult, String> {
    use yntra_vault_core::services::sync::pairing::ClientPairingMode;

    let vault_dir = super::auth::vault_storage_dir(&app)?;

    // QR pairing adopts into a dedicated collision-free file in vault_dir to avoid destroying existing open vaults
    let mode = ClientPairingMode::AdoptIntoDir { target_dir: vault_dir };

    let operation = std::sync::Arc::new(state.pairing_operation.begin()?);
    let worker_operation = operation.clone();
    let cancel = operation.cancel.clone();
    let worker_cancel = cancel.clone();
    let (initial_path, initial_identity) = {
        let vault = state.vault.lock().map_err(|e| e.to_string())?;
        (vault.as_ref().map(|m| m.path.clone()), vault.as_ref().map(|m| m.data.metadata.id))
    };
    let mut res = tokio::task::spawn_blocking(move || {
        let _operation = worker_operation;
        yntra_vault_core::services::sync::pairing::run_p2p_qr_pairing_client_cancellable(
            &qr_payload,
            mode,
            device_name,
            password,
            Some(worker_cancel),
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    use zeroize::Zeroizing;
    let received_password = res.master_password.take().map(Zeroizing::new);
    if cancel.load(std::sync::atomic::Ordering::Acquire) { return Err("Pairing cancelled".into()); }
    // A late result must neither unlock after cancellation nor replace a different active vault.
    if let (Some(saved_path), Some(pwd)) = (&res.stats.vault_path, received_password.as_ref()) {
        let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
        if vault.as_ref().map(|m| m.path.clone()) != initial_path
            || vault.as_ref().map(|m| m.data.metadata.id) != initial_identity {
            return Err("Active vault changed during pairing".into());
        }
        if cancel.load(std::sync::atomic::Ordering::Acquire) { return Err("Pairing cancelled".into()); }
        let manager = yntra_vault_core::vault::VaultManager::open(std::path::Path::new(saved_path), pwd)
            .map_err(|e| e.to_string())?;
        if cancel.load(std::sync::atomic::Ordering::Acquire) { return Err("Pairing cancelled".into()); }
        *vault = Some(manager);
    }

    // If the host did not provide a master password, store the pending vault in memory for manual completion
    if let Some(pending) = res.pending_vault.take() {
        let mut guard = state.pending_adopted_vault.lock().map_err(|e| e.to_string())?;
        if cancel.load(std::sync::atomic::Ordering::Acquire) { return Err("Pairing cancelled".into()); }
        *guard = Some(pending);
    }

    // Zeroize and strip master password so it NEVER leaves Rust or serializes to webview JS
    res.master_password = None;

    Ok(res)
}

#[tauri::command]
pub async fn complete_adopted_vault(
    password: String,
    state: State<'_, AppState>,
) -> Result<yntra_vault_core::services::sync::PairingStats, String> {
    let operation = std::sync::Arc::new(state.pairing_operation.begin()?);
    let worker_operation = operation.clone();
    let (initial_path, initial_identity) = {
        let vault = state.vault.lock().map_err(|e| e.to_string())?;
        (vault.as_ref().map(|m| m.path.clone()), vault.as_ref().map(|m| m.data.metadata.id))
    };
    let pending = {
        let mut guard = state.pending_adopted_vault.lock().map_err(|e| e.to_string())?;
        guard.take().ok_or("Ingen väntande valvadoption hittades.")?
    };

    let dest_path = pending.dest_path.clone();
    let entries_count = pending.data.entries.len();

    let pwd_clone = zeroize::Zeroizing::new(password.clone());
    let retry_pending = pending.clone();
    let result = tokio::task::spawn_blocking(move || {
        let _operation = worker_operation;
        yntra_vault_core::services::sync::pairing::complete_adopted_vault_save(&pending, &pwd_clone)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string());
    if let Err(error) = result {
        let mut guard = state.pending_adopted_vault.lock().map_err(|e| e.to_string())?;
        if !operation.cancel.load(std::sync::atomic::Ordering::Acquire) {
            *guard = Some(retry_pending);
        }
        return Err(error);
    }
    if operation.cancel.load(std::sync::atomic::Ordering::Acquire) { return Err("Pairing cancelled".into()); }

    {
        let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
        if vault.as_ref().map(|m| m.path.clone()) != initial_path
            || vault.as_ref().map(|m| m.data.metadata.id) != initial_identity {
            return Err("Active vault changed during pairing".into());
        }
        if operation.cancel.load(std::sync::atomic::Ordering::Acquire) { return Err("Pairing cancelled".into()); }
        let manager = yntra_vault_core::vault::manager::VaultManager::open(&dest_path, &password)
            .map_err(|e| e.to_string())?;
        if operation.cancel.load(std::sync::atomic::Ordering::Acquire) { return Err("Pairing cancelled".into()); }
        *vault = Some(manager);
        if operation.cancel.load(std::sync::atomic::Ordering::Acquire) {
            if let Some(mut manager) = vault.take() { manager.lock(); }
            return Err("Pairing cancelled".into());
        }
    }

    Ok(yntra_vault_core::services::sync::PairingStats {
        peer_saved: None,
        entries_sent: 0,
        entries_received: entries_count,
        entries_merged: entries_count,
        total_entries: entries_count,
        vault_path: Some(dest_path.to_string_lossy().to_string()),
        peer_addr: None,
    })
}


