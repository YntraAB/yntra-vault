use tauri::{Manager, State};

use super::AppState;

fn ensure_sync_target(manager: &yntra_vault_core::vault::VaultManager, path: &std::path::Path, salt: &[u8; 32]) -> Result<(), String> {
    if !manager.is_unlocked() || manager.path != path || manager.salt() != *salt {
        return Err("Active vault changed during synchronization".into());
    }
    Ok(())
}

fn apply_p2p_result(state: &AppState, path: &std::path::Path, salt: &[u8; 32], data: yntra_vault_core::vault::VaultData) -> Result<(), String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;
    ensure_sync_target(manager, path, salt)?;
    if manager.data.metadata.id != data.metadata.id {
        return Err("Synchronized vault identity does not match the active vault".into());
    }
    // Preserve edits made while the network operation was running.
    yntra_vault_core::services::sync::merge_vault_data(&mut manager.data, data);
    manager.rebuild_search_index();
    manager.save().map_err(|e| e.to_string())
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
) -> Result<Option<String>, String> {
    yntra_vault_core::services::sync::webdav_upload(
        &url,
        &username,
        password.as_deref(),
        std::path::Path::new(&db_path),
        if_match_etag.as_deref(),
    ).await.map_err(|e| e.to_string())
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

    // 1. Download and write file to dest_db_path with safety backup
    yntra_vault_core::services::sync::webdav_download(
        &url,
        &username,
        password.as_deref(),
        std::path::Path::new(&dest_db_path),
    ).await.map_err(|e| e.to_string())?;

    // 2. If the active in-memory vault corresponds to dest_db_path, reload it immediately
    let mut vault_guard = state.vault.lock().map_err(|e| e.to_string())?;
    if let Some(mgr) = vault_guard.as_mut() {
        let is_same_vault = mgr.path == std::path::Path::new(&dest_db_path)
            || match (std::fs::canonicalize(&mgr.path), std::fs::canonicalize(&dest_db_path)) {
                (Ok(c1), Ok(c2)) => c1 == c2,
                _ => false,
            };

        if is_same_vault
            && let Err(err) = mgr.reload() {
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
pub async fn webdav_sync(
    url: String,
    username: String,
    password: Option<String>,
    state: State<'_, AppState>,
) -> Result<yntra_vault_core::services::sync::MergeStats, String> {
    use yntra_vault_core::services::sync;
    let password = password.map(zeroize::Zeroizing::new);
    let credentials = password.as_ref().map(|p| p.as_str());
    let (subkeys, path, salt, id) = {
        let vault = state.vault.lock().map_err(|e| e.to_string())?;
        let manager = vault.as_ref().ok_or("Vault is locked")?;
        (manager.get_subkeys().map_err(|e| e.to_string())?.clone(), manager.path.clone(), manager.salt(), manager.data.metadata.id)
    };
    let mut total = sync::MergeStats::default();
    for _ in 0..3 {
        let snapshot = sync::webdav_download_snapshot(&url, &username, credentials)
            .await.map_err(|e| e.to_string())?;
        let remote = snapshot.as_ref().map(|(bytes, _)| {
            sync::decrypt_remote_vault_bytes_checked(bytes, &subkeys, Some(&salt))
        }).transpose().map_err(|e| e.to_string())?;
        let upload_snapshot = {
            let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
            let manager = vault.as_mut().ok_or("Vault is locked")?;
            ensure_sync_target(manager, &path, &salt)?;
            if let Some(remote) = remote {
                if remote.metadata.id != id { return Err("Remote vault identity does not match".into()); }
                let stats = sync::merge_vault_data(&mut manager.data, remote);
                total.entries_added += stats.entries_added;
                total.entries_updated += stats.entries_updated;
                total.entries_kept_local += stats.entries_kept_local;
                total.tags_merged += stats.tags_merged;
                total.trash_merged += stats.trash_merged;
            }
            manager.rebuild_search_index();
            manager.save().map_err(|e| e.to_string())?;
            sync::SyncSnapshot::create(&path).map_err(|e| e.to_string())?
        };
        let etag = snapshot.as_ref().map(|(_, etag)| etag.as_str());
        match sync::webdav_upload_conditional(&url, &username, credentials, &upload_snapshot.path(), etag, snapshot.is_none()).await {
            Ok(new_etag) => {
                let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
                let manager = vault.as_mut().ok_or("Vault is locked")?;
                ensure_sync_target(manager, &path, &salt)?;
                manager.data.settings.webdav.last_etag = new_etag;
                manager.data.settings.webdav.last_sync_at = Some(chrono::Utc::now());
                manager.save().map_err(|e| e.to_string())?;
                return Ok(total);
            }
            Err(error) if error.to_string().contains("412 Precondition Failed") => continue,
            Err(error) => return Err(error.to_string()),
        }
    }
    Err("WebDAV changed during all three sync attempts; retry synchronization".into())
}

#[tauri::command]
pub async fn run_p2p_sync_listener(
    app: tauri::AppHandle,
    listen_addr: String,
    db_path: String,
    state: State<'_, AppState>,
) -> Result<yntra_vault_core::services::sync::MergeStats, String> {
    let (subkeys, path, salt) = {
        let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
        let manager = vault.as_mut().ok_or("Vault is locked")?;
        manager.save().map_err(|e| e.to_string())?;
        (manager.get_subkeys().map_err(|e| e.to_string())?.clone(), manager.path.clone(), manager.salt())
    };

    if !db_path.is_empty() && std::path::Path::new(&db_path) != path {
        return Err("Synchronization is restricted to the active vault".into());
    }
    let target_path = path.clone();

    let (stats, merged_data) = tokio::task::spawn_blocking(move || {
        yntra_vault_core::services::sync::run_p2p_sync_listener_with_snapshot(
            &listen_addr,
            &subkeys,
            || {
                use yntra_vault_core::{services::sync::SyncSnapshot, VaultError};
                let state = app.state::<AppState>();
                let mut vault = state.vault.lock().map_err(|e| VaultError::SyncError(e.to_string()))?;
                let manager = vault.as_mut().ok_or(VaultError::VaultLocked)?;
                ensure_sync_target(manager, &target_path, &salt).map_err(VaultError::SyncError)?;
                manager.save()?;
                SyncSnapshot::create(&target_path)
            },
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    apply_p2p_result(&state, &path, &salt, merged_data)?;

    Ok(stats)
}

#[tauri::command]
pub async fn run_p2p_sync_client(
    server_addr: String,
    db_path: String,
    device_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<yntra_vault_core::services::sync::MergeStats, String> {
    let (subkeys, path, salt, snapshot) = {
        let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
        let manager = vault.as_mut().ok_or("Vault is locked")?;
        manager.save().map_err(|e| e.to_string())?;
        let snapshot = yntra_vault_core::services::sync::SyncSnapshot::create(&manager.path).map_err(|e| e.to_string())?;
        (manager.get_subkeys().map_err(|e| e.to_string())?.clone(), manager.path.clone(), manager.salt(), snapshot)
    };

    if !db_path.is_empty() && std::path::Path::new(&db_path) != path {
        return Err("Synchronization is restricted to the active vault".into());
    }
    let dev_uuid = device_id.and_then(|d| uuid::Uuid::parse_str(&d).ok());

    let (stats, merged_data) = tokio::task::spawn_blocking(move || {
        yntra_vault_core::services::sync::run_p2p_sync_client_with_device(
            &server_addr,
            &subkeys,
            &snapshot.path(),
            dev_uuid,
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    apply_p2p_result(&state, &path, &salt, merged_data)?;

    Ok(stats)
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
    Ok(())
}

#[tauri::command]
pub async fn start_pairing_host(
    listen_addr: String,
    password: String,
    pairing_code: String,
    device_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<yntra_vault_core::services::sync::PairingStats, String> {
    let host_db_path = {
        let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
        let manager = vault.as_mut().ok_or("Vault is locked")?;
        manager.save().map_err(|e| e.to_string())?;
        manager.path.clone()
    };

    state.pairing_cancel.store(false, std::sync::atomic::Ordering::Relaxed);
    let cancel_flag = state.pairing_cancel.clone();

    let (stats, merged_data) = tokio::task::spawn_blocking(move || {
        yntra_vault_core::services::sync::pairing::run_p2p_pairing_host_with_device_and_cancel(
            &listen_addr,
            &password,
            &pairing_code,
            &host_db_path,
            std::time::Duration::from_secs(180),
            device_name,
            Some(cancel_flag),
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    {
        let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
        if let Some(manager) = vault.as_mut() {
            if manager.is_unlocked() && manager.data.metadata.id == merged_data.metadata.id {
                let trusted_devices = merged_data.settings.trusted_devices.clone();
                yntra_vault_core::services::sync::merge_vault_data(&mut manager.data, merged_data);
                manager.data.settings.trusted_devices = trusted_devices;
                manager.rebuild_search_index();
                manager.save().map_err(|e| e.to_string())?;
            }
        }
    }

    Ok(stats)
}

#[tauri::command]
pub fn cancel_pairing_host(state: State<'_, AppState>) -> Result<(), String> {
    state.pairing_cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    Ok(())
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

    // CRITICAL SECURITY INVARIANT:
    // A client can only transmit and merge an existing local vault if that vault is
    // actively unlocked in the current session (`state.vault` is Some).
    // If no vault is currently unlocked (e.g. user is on the VaultSelect screen or locked),
    // pairing MUST strictly operate in Adopt mode. In Adopt mode, the client reads ZERO
    // local files from disk, transmits ZERO entries to the host, and saves the received
    // remote vault into a dedicated, collision-free file.
    let active_vault_path = {
        let vault_guard = state.vault.lock().map_err(|e| e.to_string())?;
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

    let pass_clone = password.clone();
    let (stats, _merged_data) = tokio::task::spawn_blocking(move || {
        yntra_vault_core::services::sync::pairing::run_p2p_pairing_client_with_device(
            &server_addr,
            &pass_clone,
            &pairing_code,
            mode,
            device_name,
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    // Open newly adopted vault or update active manager in AppState
    if let Some(ref saved_path_str) = stats.vault_path {
        let saved_path = std::path::PathBuf::from(saved_path_str);
        let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
        if let Some(manager) = vault.as_mut() {
            if manager.path == saved_path {
                // Pairing may change the root keys; reopen instead of retaining old keys.
                *manager = yntra_vault_core::vault::VaultManager::open(&saved_path, &password)
                    .map_err(|e| e.to_string())?;
            }
        } else if let Ok(manager) = yntra_vault_core::vault::manager::VaultManager::open(&saved_path, &password) {
            *vault = Some(manager);
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

    let (session, info) = yntra_vault_core::services::sync::generate_qr_pairing_session(
        local_ips,
        yntra_vault_core::services::sync::DEFAULT_PAIRING_PORT,
        device_name,
    );

    let mut guard = state.qr_pairing_session.lock().map_err(|e| e.to_string())?;
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
    let session = {
        let guard = state.qr_pairing_session.lock().map_err(|e| e.to_string())?;
        guard.as_ref().cloned().ok_or("Ingen aktiv QR-parningssession hittades. Generera en QR-kod först.")?
    };

    let host_db_path = {
        let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
        let manager = vault.as_mut().ok_or("Valvet är låst")?;
        manager.save().map_err(|e| e.to_string())?;
        manager.path.clone()
    };

    state.pairing_cancel.store(false, std::sync::atomic::Ordering::Relaxed);
    let cancel_flag = state.pairing_cancel.clone();

    let (stats, merged_data) = tokio::task::spawn_blocking(move || {
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
        if let Some(manager) = vault.as_mut() {
            if manager.is_unlocked() && manager.data.metadata.id == merged_data.metadata.id {
                let trusted_devices = merged_data.settings.trusted_devices.clone();
                yntra_vault_core::services::sync::merge_vault_data(&mut manager.data, merged_data);
                manager.data.settings.trusted_devices = trusted_devices;
                manager.rebuild_search_index();
                manager.save().map_err(|e| e.to_string())?;
            }
        }
    }

    if let Ok(mut guard) = state.qr_pairing_session.lock() {
        *guard = None;
    }

    Ok(stats)
}

#[tauri::command]
pub fn cancel_qr_pairing_host(state: State<'_, AppState>) -> Result<(), String> {
    state.pairing_cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    if let Ok(mut guard) = state.qr_pairing_session.lock() {
        *guard = None;
    }
    if let Ok(mut guard) = state.pending_adopted_vault.lock() {
        *guard = None;
    }
    Ok(())
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

    let mut res = tokio::task::spawn_blocking(move || {
        yntra_vault_core::services::sync::run_p2p_qr_pairing_client(
            &qr_payload,
            mode,
            device_name,
            password,
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    // If a master password was provisioned in-transit, unlock the vault in AppState immediately
    if let Some(ref saved_path_str) = res.stats.vault_path
        && let Some(ref pwd) = res.master_password {
            let saved_path = std::path::PathBuf::from(saved_path_str);
            let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
            if let Some(manager) = vault.as_mut() {
                if manager.path == saved_path {
                    let _ = manager.reload();
                }
            } else if let Ok(manager) = yntra_vault_core::vault::manager::VaultManager::open(&saved_path, pwd) {
                *vault = Some(manager);
            }
        }

    // If the host did not provide a master password, store the pending vault in memory for manual completion
    if let Some(pending) = res.pending_vault.take() {
        let mut guard = state.pending_adopted_vault.lock().map_err(|e| e.to_string())?;
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
    let pending = {
        let mut guard = state.pending_adopted_vault.lock().map_err(|e| e.to_string())?;
        guard.take().ok_or("Ingen väntande valvadoption hittades.")?
    };

    let dest_path = pending.dest_path.clone();
    let entries_count = pending.data.entries.len();

    let pwd_clone = password.clone();
    tokio::task::spawn_blocking(move || {
        yntra_vault_core::services::sync::pairing::complete_adopted_vault_save(&pending, &pwd_clone)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    {
        let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
        let manager = yntra_vault_core::vault::manager::VaultManager::open(&dest_path, &password)
            .map_err(|e| e.to_string())?;
        *vault = Some(manager);
    }

    Ok(yntra_vault_core::services::sync::PairingStats {
        entries_sent: 0,
        entries_received: entries_count,
        entries_merged: entries_count,
        total_entries: entries_count,
        vault_path: Some(dest_path.to_string_lossy().to_string()),
        peer_addr: None,
    })
}

