use tauri::State;

use super::AppState;

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

        if is_same_vault {
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
    const MAX_RETRIES: usize = 3;

    let (subkeys, db_path, local_salt, mut current_etag) = {
        let mut vault_guard = state.vault.lock().map_err(|e| e.to_string())?;
        let mgr = vault_guard.as_mut().ok_or("Vault is locked")?;
        mgr.save().map_err(|e| e.to_string())?;
        let subkeys = (*mgr.get_subkeys().map_err(|e| e.to_string())?).clone();
        let db_path = mgr.path.clone();
        let local_salt = mgr.salt();
        let current_etag = mgr.data.settings.webdav.last_etag.clone();
        (subkeys, db_path, local_salt, current_etag)
    };

    let mut accumulated_stats = yntra_vault_core::services::sync::MergeStats::default();

    // If client has never synced before (no known ETag), check if a remote vault already exists.
    // If a remote vault exists, download, verify root salt, and merge before uploading
    // to prevent unconditionally overwriting an existing remote vault from another device.
    if current_etag.is_none() {
        let remote_etag = yntra_vault_core::services::sync::webdav_get_etag(
            &url,
            &username,
            password.as_deref(),
        ).await.unwrap_or(None);

        if let Some(etag) = remote_etag {
            let remote_bytes = yntra_vault_core::services::sync::webdav_download_bytes(
                &url,
                &username,
                password.as_deref(),
            ).await.map_err(|err| format!("Failed downloading remote vault for initial merge: {}", err))?;

            let remote_data = yntra_vault_core::services::sync::decrypt_remote_vault_bytes_checked(
                &remote_bytes,
                &subkeys,
                Some(&local_salt),
            ).map_err(|err| err.to_string())?;

            let stats = {
                let mut vault_guard = state.vault.lock().map_err(|e| e.to_string())?;
                let mgr = vault_guard.as_mut().ok_or("Vault is locked")?;
                let stats = yntra_vault_core::services::sync::merge_vault_data(&mut mgr.data, remote_data);
                mgr.save().map_err(|e| e.to_string())?;
                mgr.rebuild_search_index();
                stats
            };

            accumulated_stats.entries_added += stats.entries_added;
            accumulated_stats.entries_updated += stats.entries_updated;
            accumulated_stats.entries_kept_local += stats.entries_kept_local;
            accumulated_stats.tags_merged += stats.tags_merged;
            accumulated_stats.trash_merged += stats.trash_merged;

            current_etag = Some(etag);
        }
    }

    for attempt in 0..MAX_RETRIES {
        let upload_res = yntra_vault_core::services::sync::webdav_upload(
            &url,
            &username,
            password.as_deref(),
            &db_path,
            current_etag.as_deref(),
        ).await;

        match upload_res {
            Ok(new_etag_opt) => {
                let mut vault_guard = state.vault.lock().map_err(|e| e.to_string())?;
                if let Some(mgr) = vault_guard.as_mut() {
                    if let Some(new_etag) = new_etag_opt {
                        mgr.data.settings.webdav.last_etag = Some(new_etag);
                    }
                    mgr.data.settings.webdav.last_sync_at = Some(chrono::Utc::now());
                    let _ = mgr.save();
                }
                return Ok(accumulated_stats);
            }
            Err(e) if e.to_string().contains("412 Precondition Failed") || e.to_string().contains("modified on server") => {
                // Conflict detected!
                // 1. Fetch remote ETag first to get the latest server version
                let remote_etag = yntra_vault_core::services::sync::webdav_get_etag(
                    &url,
                    &username,
                    password.as_deref(),
                ).await.unwrap_or(None);

                // 2. Download remote bytes into memory
                let remote_bytes = yntra_vault_core::services::sync::webdav_download_bytes(
                    &url,
                    &username,
                    password.as_deref(),
                ).await.map_err(|err| format!("Failed downloading remote vault for merge (attempt {}): {}", attempt + 1, err))?;

                // 3. Decrypt remote payload with root salt verification
                let remote_data = yntra_vault_core::services::sync::decrypt_remote_vault_bytes_checked(
                    &remote_bytes,
                    &subkeys,
                    Some(&local_salt),
                ).map_err(|err| err.to_string())?;

                // 4. Perform 3-way merge in memory & save local database file
                let stats = {
                    let mut vault_guard = state.vault.lock().map_err(|e| e.to_string())?;
                    let mgr = vault_guard.as_mut().ok_or("Vault is locked")?;
                    let stats = yntra_vault_core::services::sync::merge_vault_data(&mut mgr.data, remote_data);
                    mgr.save().map_err(|e| e.to_string())?;
                    mgr.rebuild_search_index();
                    stats
                };

                accumulated_stats.entries_added += stats.entries_added;
                accumulated_stats.entries_updated += stats.entries_updated;
                accumulated_stats.entries_kept_local += stats.entries_kept_local;
                accumulated_stats.tags_merged += stats.tags_merged;
                accumulated_stats.trash_merged += stats.trash_merged;

                // Set current_etag to the acquired remote_etag so the next loop iteration attempts conditional PUT against it
                current_etag = remote_etag;
            }
            Err(e) => return Err(e.to_string()),
        }
    }

    Err("WebDAV sync failed after maximum retry attempts due to high remote contention".into())
}

#[tauri::command]
pub async fn run_p2p_sync_listener(
    listen_addr: String,
    db_path: String,
    state: State<'_, AppState>,
) -> Result<yntra_vault_core::services::sync::MergeStats, String> {
    let (subkeys, path) = {
        let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
        let manager = vault.as_mut().ok_or("Vault is locked")?;
        manager.save().map_err(|e| e.to_string())?;
        (manager.get_subkeys().map_err(|e| e.to_string())?.clone(), manager.path.clone())
    };

    let target_path = if db_path.is_empty() { path } else { std::path::PathBuf::from(db_path) };

    let (stats, merged_data) = tokio::task::spawn_blocking(move || {
        yntra_vault_core::services::sync::run_p2p_sync_listener(
            &listen_addr,
            &subkeys,
            &target_path,
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    {
        let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
        if let Some(manager) = vault.as_mut() {
            manager.data = merged_data;
            manager.rebuild_search_index();
        }
    }

    Ok(stats)
}

#[tauri::command]
pub async fn run_p2p_sync_client(
    server_addr: String,
    db_path: String,
    state: State<'_, AppState>,
) -> Result<yntra_vault_core::services::sync::MergeStats, String> {
    let (subkeys, path) = {
        let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
        let manager = vault.as_mut().ok_or("Vault is locked")?;
        manager.save().map_err(|e| e.to_string())?;
        (manager.get_subkeys().map_err(|e| e.to_string())?.clone(), manager.path.clone())
    };

    let target_path = if db_path.is_empty() { path } else { std::path::PathBuf::from(db_path) };

    let (stats, merged_data) = tokio::task::spawn_blocking(move || {
        yntra_vault_core::services::sync::run_p2p_sync_client(
            &server_addr,
            &subkeys,
            &target_path,
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    {
        let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
        if let Some(manager) = vault.as_mut() {
            manager.data = merged_data;
            manager.rebuild_search_index();
        }
    }

    Ok(stats)
}
