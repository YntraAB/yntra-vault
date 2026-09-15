//! Vault Synchronization Protocols (WebDAV cloud sync and local network P2P sync).

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use rand::Rng;
use crate::crypto::{compute_hmac, verify_hmac};
use crate::vault::format::VaultFile;

/// Maximum database size accepted during P2P sync (256 MB)
const MAX_DB_SIZE: usize = 256 * 1024 * 1024;

// ─── WebDAV Cloud Sync & SOTA Merge Protocol ───────────────────────────────────

/// Result of a 3-way item-level vault merge.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MergeStats {
    pub entries_added: usize,
    pub entries_updated: usize,
    pub entries_kept_local: usize,
    pub tags_merged: usize,
    pub trash_merged: usize,
}

/// Perform an item-level 3-way merge between local and remote VaultData.
/// Reconciles entries by UUID, choosing the entry with the latest `updated_at` timestamp.
/// Preserves entry history and merges tags and trash.
/// Perform an item-level 3-way merge between local and remote VaultData.
/// Reconciles entries by UUID, choosing the entry with the latest `updated_at` timestamp.
/// Respects trash `deleted_at` timestamps to prevent resurrected "zombie" entries.
pub fn merge_vault_data(local: &mut crate::vault::types::VaultData, remote: crate::vault::types::VaultData) -> MergeStats {
    use std::collections::HashMap;
    use uuid::Uuid;
    use crate::vault::types::{Entry, Tag, TrashedEntry};

    let mut stats = MergeStats::default();

    // Build map of trash items
    let mut local_trash_map: HashMap<Uuid, TrashedEntry> = local.trash.drain(..).map(|t| (t.entry.id, t)).collect();
    let mut remote_trash_map: HashMap<Uuid, TrashedEntry> = remote.trash.into_iter().map(|t| (t.entry.id, t)).collect();

    // 1. Build map of local entries by UUID
    let mut local_map: HashMap<Uuid, Entry> = local.entries.drain(..).map(|e| (e.id, e)).collect();

    for remote_entry in remote.entries {
        match local_map.get_mut(&remote_entry.id) {
            Some(local_entry) => {
                if remote_entry.updated_at > local_entry.updated_at {
                    // Remote entry is newer: replace local entry with remote entry, but retain combined password history
                    let mut winner = remote_entry;
                    merge_password_histories(&mut winner.password_history, local_entry.password_history.clone());
                    *local_entry = winner;
                    stats.entries_updated += 1;
                } else {
                    // Local entry is newer or equal: retain local entry, but merge remote password history
                    merge_password_histories(&mut local_entry.password_history, remote_entry.password_history);
                    stats.entries_kept_local += 1;
                }
            }
            None => {
                // Entry exists only in remote: check if local trashed it after remote's update
                let should_add = match local_trash_map.get(&remote_entry.id) {
                    Some(local_trash) => local_trash.deleted_at < remote_entry.updated_at,
                    None => true,
                };
                if should_add {
                    // If remote entry was updated after local trashed it, remove from local trash & add to active entries
                    local_trash_map.remove(&remote_entry.id);
                    local_map.insert(remote_entry.id, remote_entry);
                    stats.entries_added += 1;
                }
            }
        }
    }

    // Filter local entries against remote trash
    let mut final_entries = Vec::new();
    for (id, local_entry) in local_map {
        let is_trashed_by_remote = match remote_trash_map.get(&id) {
            Some(remote_trash) => remote_trash.deleted_at >= local_entry.updated_at,
            None => false,
        };
        if is_trashed_by_remote {
            // Remote trashed entry after local's last update: move local entry into trash
            let remote_trash = remote_trash_map.remove(&id).unwrap();
            local_trash_map.insert(id, remote_trash);
            stats.trash_merged += 1;
        } else {
            final_entries.push(local_entry);
        }
    }
    local.entries = final_entries;

    // 2. Merge Tags by UUID
    let mut tag_map: HashMap<Uuid, Tag> = local.tags.drain(..).map(|t| (t.id, t)).collect();
    for remote_tag in remote.tags {
        if !tag_map.contains_key(&remote_tag.id) {
            tag_map.insert(remote_tag.id, remote_tag);
            stats.tags_merged += 1;
        }
    }
    local.tags = tag_map.into_values().collect();

    // 3. Reconcile remaining trash items
    for (id, remote_trash) in remote_trash_map {
        if local.entries.iter().any(|e| e.id == id) {
            continue;
        }
        match local_trash_map.get_mut(&id) {
            Some(local_trash) => {
                if remote_trash.deleted_at > local_trash.deleted_at {
                    *local_trash = remote_trash;
                    stats.trash_merged += 1;
                }
            }
            None => {
                local_trash_map.insert(id, remote_trash);
                stats.trash_merged += 1;
            }
        }
    }
    local.trash = local_trash_map.into_values().collect();

    stats
}

/// Helper: Merge and deduplicate password histories.
fn merge_password_histories(
    dest: &mut Vec<crate::vault::types::PasswordHistoryItem>,
    source: Vec<crate::vault::types::PasswordHistoryItem>,
) {
    use crate::vault::types::MAX_PASSWORD_HISTORY;
    for item in source {
        if !dest.iter().any(|existing| existing.changed_at == item.changed_at) {
            dest.push(item);
        }
    }
    dest.sort_by(|a, b| b.changed_at.cmp(&a.changed_at));
    if dest.len() > MAX_PASSWORD_HISTORY {
        dest.truncate(MAX_PASSWORD_HISTORY);
    }
}

/// Helper: Normalize ETag according to RFC 7232. Strips weak tags (`W/` or `w/`), quotes, and whitespace.
pub fn normalize_etag(etag: &str) -> String {
    let trimmed = etag.trim();
    let stripped = if trimmed.starts_with("W/") || trimmed.starts_with("w/") {
        &trimmed[2..]
    } else {
        trimmed
    };
    stripped.trim_matches('"').trim().to_string()
}

/// Helper: Validate WebDAV URL for basic structural correctness and TLS transport security.
/// Rejects plain `http://` URLs unless connecting to localhost / 127.0.0.1 / [::1].
pub fn validate_webdav_url(raw_url: &str) -> crate::Result<()> {
    let trimmed = raw_url.trim();
    if trimmed.is_empty() {
        return Err(crate::error::VaultError::InvalidFormat("WebDAV URL cannot be empty".into()));
    }

    let parsed = url::Url::parse(trimmed).map_err(|e| {
        crate::error::VaultError::InvalidFormat(format!("Invalid WebDAV URL format: {}", e))
    })?;

    match parsed.scheme() {
        "https" => Ok(()),
        "http" => {
            let host = parsed.host_str().unwrap_or("");
            if host == "localhost" || host == "127.0.0.1" || host == "::1" || host == "[::1]" {
                Ok(())
            } else {
                Err(crate::error::VaultError::InvalidFormat(
                    "WebDAV URL must use HTTPS for secure transport outside localhost".into(),
                ))
            }
        }
        _ => Err(crate::error::VaultError::InvalidFormat(
            "WebDAV URL must start with https:// (or http:// for localhost testing)".into(),
        )),
    }
}

/// Query the current ETag of a remote WebDAV resource using HEAD or PROPFIND (RFC 4918).
pub async fn webdav_get_etag(
    url: &str,
    username: &str,
    password: Option<&str>,
) -> crate::Result<Option<String>> {
    validate_webdav_url(url)?;

    let client = reqwest::Client::builder()
        .user_agent("Yntra Vault-PasswordManager/1.0")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("HTTP client init: {}", e)))?;

    let mut req = client.head(url);
    if !username.is_empty() {
        req = req.basic_auth(username, password);
    }

    if let Ok(response) = req.send().await {
        if response.status().is_success() {
            if let Some(etag) = response.headers().get("ETag").and_then(|h| h.to_str().ok()) {
                let trimmed = normalize_etag(etag);
                if !trimmed.is_empty() {
                    return Ok(Some(trimmed));
                }
            }
        }
    }

    // Fallback: WebDAV PROPFIND (Depth: 0) for servers that don't return ETag on HEAD
    let propfind_method = reqwest::Method::from_bytes(b"PROPFIND").unwrap_or(reqwest::Method::GET);
    let mut pf_req = client.request(propfind_method, url)
        .header("Depth", "0")
        .header("Content-Type", "application/xml");
    if !username.is_empty() {
        pf_req = pf_req.basic_auth(username, password);
    }

    if let Ok(pf_resp) = pf_req.send().await {
        if pf_resp.status().is_success() || pf_resp.status().as_u16() == 207 {
            if let Some(etag) = pf_resp.headers().get("ETag").and_then(|h| h.to_str().ok()) {
                let trimmed = normalize_etag(etag);
                if !trimmed.is_empty() {
                    return Ok(Some(trimmed));
                }
            }
            if let Ok(body) = pf_resp.text().await {
                if let Some(start) = body.find("<getetag>") {
                    let rest = &body[start + 9..];
                    if let Some(end) = rest.find("</getetag>") {
                        let etag_val = normalize_etag(&rest[..end]);
                        if !etag_val.is_empty() {
                            return Ok(Some(etag_val));
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

/// Test connectivity and credentials against a remote WebDAV server.
pub async fn webdav_test_connection(
    url: &str,
    username: &str,
    password: Option<&str>,
) -> crate::Result<()> {
    validate_webdav_url(url)?;

    let client = reqwest::Client::builder()
        .user_agent("Yntra Vault-PasswordManager/1.0")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("HTTP client init: {}", e)))?;

    let mut req = client.head(url);
    if !username.is_empty() {
        req = req.basic_auth(username, password);
    }

    let response = req.send().await
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("WebDAV connection failed: {}", e)))?;

    if response.status().is_success() || response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED {
        Ok(())
    } else {
        Err(crate::error::VaultError::EncryptionError(format!(
            "WebDAV server returned status code: {}",
            response.status()
        )))
    }
}

/// Upload the local encrypted database file to a WebDAV server with conditional `If-Match` header.
pub async fn webdav_upload(
    url: &str,
    username: &str,
    password: Option<&str>,
    db_filepath: &Path,
    if_match_etag: Option<&str>,
) -> crate::Result<Option<String>> {
    validate_webdav_url(url)?;

    let client = reqwest::Client::builder()
        .user_agent("Yntra Vault-PasswordManager/1.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("HTTP client init: {}", e)))?;

    let file_data = fs::read(db_filepath)
        .map_err(|e| crate::error::VaultError::SerializationError(format!("Read DB: {}", e)))?;

    let mut req = client.put(url).body(file_data);
    if !username.is_empty() {
        req = req.basic_auth(username, password);
    }

    if let Some(etag) = if_match_etag {
        let norm = normalize_etag(etag);
        if !norm.is_empty() {
            let etag_header = format!("\"{}\"", norm);
            req = req.header("If-Match", etag_header);
        }
    }

    let response = req.send().await
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("WebDAV PUT request failed: {}", e)))?;

    if response.status() == reqwest::StatusCode::PRECONDITION_FAILED {
        return Err(crate::error::VaultError::EncryptionError(
            "HTTP 412 Precondition Failed: Remote vault was modified on server".into()
        ));
    }

    if !response.status().is_success() {
        return Err(crate::error::VaultError::EncryptionError(format!(
            "WebDAV server returned failure status: {}",
            response.status()
        )));
    }

    let etag = response.headers()
        .get("ETag")
        .and_then(|h| h.to_str().ok())
        .map(normalize_etag)
        .filter(|s| !s.is_empty());

    if etag.is_none() {
        // Fallback: Query ETag from server if PUT response omitted ETag header
        Ok(webdav_get_etag(url, username, password).await.unwrap_or(None))
    } else {
        Ok(etag)
    }
}

/// Download the encrypted database file from a WebDAV server with safety backup.
pub async fn webdav_download(
    url: &str,
    username: &str,
    password: Option<&str>,
    dest_db_filepath: &Path,
) -> crate::Result<()> {
    validate_webdav_url(url)?;

    let client = reqwest::Client::builder()
        .user_agent("Yntra Vault-PasswordManager/1.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| crate::error::VaultError::DecryptionError(format!("HTTP client init: {}", e)))?;

    let mut req = client.get(url);
    if !username.is_empty() {
        req = req.basic_auth(username, password);
    }

    let response = req.send().await
        .map_err(|e| crate::error::VaultError::DecryptionError(format!("WebDAV GET request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(crate::error::VaultError::DecryptionError(format!(
            "WebDAV server returned failure status: {}",
            response.status()
        )));
    }

    let bytes = response.bytes().await
        .map_err(|e| crate::error::VaultError::DecryptionError(format!("WebDAV body retrieval: {}", e)))?;

    // Pre-flight validation: verify downloaded payload is a valid .vdb vault file
    VaultFile::from_bytes(&bytes).map_err(|e| {
        crate::error::VaultError::InvalidFormat(format!("Downloaded file is not a valid vault database: {}", e))
    })?;

    // Create a local backup (.vdb.bak) before overwriting current file
    if dest_db_filepath.exists() {
        let backup_path = dest_db_filepath.with_extension("vdb.bak");
        let _ = fs::copy(dest_db_filepath, &backup_path);
    }

    // Atomic write using temp file
    let tmp_path = dest_db_filepath.with_extension("vdb.sync.tmp");
    fs::write(&tmp_path, &bytes)
        .map_err(|e| crate::error::VaultError::SerializationError(format!("Write downloaded temp DB: {}", e)))?;

    fs::rename(&tmp_path, dest_db_filepath)
        .map_err(|e| crate::error::VaultError::SerializationError(format!("Replace local DB with downloaded DB: {}", e)))?;

    Ok(())
}

/// Download raw vault bytes from a WebDAV server into memory.
pub async fn webdav_download_bytes(
    url: &str,
    username: &str,
    password: Option<&str>,
) -> crate::Result<Vec<u8>> {
    validate_webdav_url(url)?;

    let client = reqwest::Client::builder()
        .user_agent("Yntra Vault-PasswordManager/1.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| crate::error::VaultError::DecryptionError(format!("HTTP client init: {}", e)))?;

    let mut req = client.get(url);
    if !username.is_empty() {
        req = req.basic_auth(username, password);
    }

    let response = req.send().await
        .map_err(|e| crate::error::VaultError::DecryptionError(format!("WebDAV GET request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(crate::error::VaultError::DecryptionError(format!(
            "WebDAV server returned failure status: {}",
            response.status()
        )));
    }

    let bytes = response.bytes().await
        .map_err(|e| crate::error::VaultError::DecryptionError(format!("WebDAV body retrieval: {}", e)))?;

    Ok(bytes.to_vec())
}

/// Decrypt raw remote vault bytes using active derived SubKeys, validating against an expected salt if provided.
pub fn decrypt_remote_vault_bytes_checked(
    bytes: &[u8],
    subkeys: &crate::crypto::SubKeys,
    expected_salt: Option<&[u8; 32]>,
) -> crate::Result<crate::vault::types::VaultData> {
    use zeroize::Zeroize;
    use crate::vault::format::VaultFile;
    use crate::crypto::cipher::{decrypt_vault, decrypt_vault_with_aad, EncryptedBlob};
    use crate::crypto::verify_hmac;

    let vault_file = VaultFile::from_bytes(bytes)?;

    if let Some(expected) = expected_salt {
        if &vault_file.header.salt != expected {
            return Err(crate::error::VaultError::SyncError(
                "Remote vault originates from a different root salt. Synchronizing two independently created vaults is not supported; use the same vault file across devices.".into(),
            ));
        }
    }

    if vault_file.header.version <= 2 {
        if let Some(expected_hmac) = &vault_file.hmac {
            verify_hmac(
                &vault_file.encrypted_payload,
                expected_hmac,
                &subkeys.hmac_key,
            )?;
        } else {
            return Err(crate::error::VaultError::InvalidFormat(
                "Missing expected HMAC in legacy v1/v2 file format".into(),
            ));
        }
    }

    if vault_file.encrypted_payload.len() < 24 {
        return Err(crate::error::VaultError::InvalidFormat(
            "Encrypted payload too short".into(),
        ));
    }

    let encrypted_blob = EncryptedBlob {
        nonce: vault_file.encrypted_payload[..24].to_vec(),
        ciphertext: vault_file.encrypted_payload[24..].to_vec(),
    };

    let mut decrypted = if vault_file.header.version >= 3 {
        let aad = vault_file.header.aad_bytes()?;
        decrypt_vault_with_aad(&encrypted_blob, &subkeys.vault_key, &aad)?
    } else {
        decrypt_vault(&encrypted_blob, &subkeys.vault_key)?
    };

    let data_res: crate::Result<crate::vault::types::VaultData> = match vault_file.header.version {
        1 => {
            match bincode::deserialize(&decrypted) {
                Ok(d) => Ok(d),
                Err(_) => {
                    let legacy: crate::vault::manager::LegacyVaultData = bincode::deserialize(&decrypted)
                        .map_err(|e| crate::error::VaultError::SerializationError(format!("Legacy deserialize: {}", e)))?;
                    Ok(legacy.into_current())
                }
            }
        }
        _ => {
            rmp_serde::from_slice(&decrypted)
                .map_err(|e| crate::error::VaultError::SerializationError(format!("Vault deserialize: {}", e)))
        }
    };

    decrypted.zeroize();
    data_res
}

/// Decrypt raw remote vault bytes using active derived SubKeys.
pub fn decrypt_remote_vault_bytes(
    bytes: &[u8],
    subkeys: &crate::crypto::SubKeys,
) -> crate::Result<crate::vault::types::VaultData> {
    decrypt_remote_vault_bytes_checked(bytes, subkeys, None)
}

// ─── Local Network P2P Sync ─────────────────────────────────────────────

/// Runs a secure TCP listener for vault synchronization.
/// Verifies peer credentials via a mutual challenge-response handshake signed with HMAC key,
/// receives the peer's encrypted vault, performs a 3-way item-level merge with the local vault,
/// saves the merged vault atomically to disk, and returns the merge statistics and merged vault data.
/// Helper: Processes received remote vault bytes, executes a 3-way merge against the local vault
/// (preserving local salt, biometric, and hardware 2FA envelopes), atomically writes the re-encrypted
/// database to disk via temp file rename, and returns the merge statistics, merged data, and the serialized encrypted bytes.
fn apply_and_save_remote_vault(
    remote_bytes: &[u8],
    subkeys: &crate::crypto::SubKeys,
    db_filepath: &Path,
) -> crate::Result<(MergeStats, crate::vault::types::VaultData, Vec<u8>)> {
    // Validate received data is a valid .vdb file structure before decrypting
    let remote_vault_file = VaultFile::from_bytes(remote_bytes).map_err(|e| {
        crate::error::VaultError::InvalidFormat(format!("Received invalid vault file: {}", e))
    })?;

    let (stats, final_data, saved_bytes) = if db_filepath.exists() && fs::metadata(db_filepath).map(|m| m.len() > 0).unwrap_or(false) {
        let local_bytes = fs::read(db_filepath)
            .map_err(|e| crate::error::VaultError::SerializationError(format!("Failed to read local database: {}", e)))?;
        let local_vault_file = VaultFile::from_bytes(&local_bytes)?;

        // Enforce root salt parity before attempting decryption
        if local_vault_file.header.salt != remote_vault_file.header.salt {
            return Err(crate::error::VaultError::SyncError(
                "Remote vault originates from a different root salt. Synchronizing two independently created vaults is not supported; use the same vault file across devices.".into(),
            ));
        }

        // Decrypt remote and local vault payloads using active subkeys, explicitly validating against expected salt
        let remote_data = decrypt_remote_vault_bytes_checked(remote_bytes, subkeys, Some(&local_vault_file.header.salt))?;
        let mut local_data = decrypt_remote_vault_bytes_checked(&local_bytes, subkeys, Some(&local_vault_file.header.salt))?;

        // Execute 3-way item-level merge
        let stats = merge_vault_data(&mut local_data, remote_data);

        // Update metadata timestamp and entry count
        local_data.metadata.updated_at = chrono::Utc::now();
        local_data.metadata.entry_count = local_data.entries.len();

        // Clean up old trash (> 30 days)
        let cutoff = chrono::Utc::now() - chrono::Duration::days(30);
        local_data.trash.retain(|t| t.deleted_at > cutoff);

        // Re-serialize vault data as MessagePack
        let serialized = rmp_serde::to_vec(&local_data)
            .map_err(|e| crate::error::VaultError::SerializationError(format!("Vault serialize: {}", e)))?;

        let mut flags = 0u16;
        if local_vault_file.biometric.is_some() {
            flags |= crate::vault::format::FLAG_HAS_BIOMETRIC;
        }
        if local_vault_file.hardware2fa.is_some() {
            flags |= crate::vault::format::FLAG_HAS_HARDWARE_2FA;
        }

        let header = crate::vault::format::FileHeader {
            version: crate::vault::format::FORMAT_VERSION,
            flags,
            salt: local_vault_file.header.salt,
            kdf_params: local_vault_file.header.kdf_params,
        };
        let aad = header.aad_bytes()?;

        let encrypted = crate::crypto::cipher::encrypt_vault_with_aad(&serialized, &subkeys.vault_key, &aad)?;

        let mut payload = Vec::with_capacity(encrypted.nonce.len() + encrypted.ciphertext.len());
        payload.extend_from_slice(&encrypted.nonce);
        payload.extend_from_slice(&encrypted.ciphertext);

        let merged_vault_file = VaultFile {
            header,
            hmac: None,
            biometric: local_vault_file.biometric,
            hardware2fa: local_vault_file.hardware2fa,
            encrypted_payload: payload,
        };

        let merged_bytes = merged_vault_file.to_bytes()?;
        let tmp_path = db_filepath.with_extension("vdb.sync.tmp");
        fs::write(&tmp_path, &merged_bytes)
            .map_err(|e| crate::error::VaultError::SerializationError(format!("Failed to write temp sync file: {}", e)))?;
        fs::rename(&tmp_path, db_filepath)
            .map_err(|e| crate::error::VaultError::SerializationError(format!("Failed to rename sync file: {}", e)))?;

        (stats, local_data, merged_bytes)
    } else {
        let remote_data = decrypt_remote_vault_bytes_checked(remote_bytes, subkeys, Some(&remote_vault_file.header.salt))?;
        let tmp_path = db_filepath.with_extension("vdb.sync.tmp");
        fs::write(&tmp_path, remote_bytes)
            .map_err(|e| crate::error::VaultError::SerializationError(format!("Failed to write temp sync file: {}", e)))?;
        fs::rename(&tmp_path, db_filepath)
            .map_err(|e| crate::error::VaultError::SerializationError(format!("Failed to rename sync file: {}", e)))?;

        let mut stats = MergeStats::default();
        stats.entries_added = remote_data.entries.len();
        (stats, remote_data, remote_bytes.to_vec())
    };

    Ok((stats, final_data, saved_bytes))
}

/// Runs a secure TCP listener for vault synchronization.
/// Verifies peer credentials via a mutual challenge-response handshake signed with HMAC key,
/// receives the peer's encrypted vault, performs a 3-way item-level merge with the local vault,
/// saves the merged vault atomically to disk, transmits the merged database back to the peer,
const P2P_SALT_MISMATCH_MARKER: [u8; 32] = *b"YNTRA_SYNC_ERR_SALT_MISMATCH____";
const P2P_AUTH_FAILED_SIG: [u8; 64] = *b"YNTRA_SYNC_ERR_AUTH_FAILED_MASTER_PASSWORD_MISMATCH_____________";

/// Runs a secure TCP listener for vault synchronization.
/// Verifies peer credentials via a mutual challenge-response handshake signed with HMAC key,
/// receives the peer's encrypted vault, performs a 3-way item-level merge with the local vault,
/// saves the merged vault atomically to disk, transmits the merged database back to the peer,
/// and returns the merge statistics and merged vault data.
pub fn run_p2p_sync_listener(
    listen_addr: &str,
    subkeys: &crate::crypto::SubKeys,
    db_filepath: &Path,
) -> crate::Result<(MergeStats, crate::vault::types::VaultData)> {
    let listener = TcpListener::bind(listen_addr)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("Failed to bind TCP listener: {}", e)))?;

    // Wait for a single peer connection
    let (mut stream, _) = listener.accept()
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("Failed to accept peer connection: {}", e)))?;

    let timeout = Some(std::time::Duration::from_secs(30));
    let _ = stream.set_read_timeout(timeout);
    let _ = stream.set_write_timeout(timeout);

    let local_salt = if db_filepath.exists() && fs::metadata(db_filepath).map(|m| m.len() > 0).unwrap_or(false) {
        fs::read(db_filepath)
            .ok()
            .and_then(|bytes| VaultFile::from_bytes(&bytes).ok().map(|vf| vf.header.salt))
    } else {
        None
    };

    // 1. Handshake Phase
    // Step 1a: Exchange root salt markers to detect divergent vaults early
    let server_has_salt = local_salt.is_some();
    let server_salt_buf = local_salt.unwrap_or([0u8; 32]);
    stream.write_all(&server_salt_buf)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

    let mut client_salt_buf = [0u8; 32];
    stream.read_exact(&mut client_salt_buf)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

    if server_has_salt && client_salt_buf != [0u8; 32] && server_salt_buf != client_salt_buf {
        let _ = stream.write_all(&P2P_SALT_MISMATCH_MARKER);
        return Err(crate::error::VaultError::SyncError(
            "P2P sync rejected: Remote peer vault originates from a different root salt. Both devices must share the same initial vault database to sync.".into(),
        ));
    }

    // Step 1b: Mutual challenge-response HMAC authentication (Client-First Verification)
    let mut server_challenge = [0u8; 32];
    rand::rng().fill(&mut server_challenge);

    // Send server challenge
    stream.write_all(&server_challenge)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

    // Read client challenge
    let mut client_challenge = [0u8; 32];
    stream.read_exact(&mut client_challenge)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

    // Read client signature
    let mut client_sig = [0u8; 64];
    if let Err(e) = stream.read_exact(&mut client_sig) {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            return Err(crate::error::VaultError::DecryptionError("Peer verification failed: Master password mismatch".into()));
        }
        return Err(crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)));
    }

    // Verify client signature BEFORE generating or sending any server signature
    if client_sig == P2P_AUTH_FAILED_SIG || verify_hmac(&server_challenge, &client_sig, &subkeys.hmac_key).is_err() {
        let _ = stream.write_all(b"UNAUTHOR");
        return Err(crate::error::VaultError::DecryptionError("Peer verification failed: Master password mismatch".into()));
    }

    // Compute and send server signature only after client has successfully authenticated
    let sig_to_send = compute_hmac(&client_challenge, &subkeys.hmac_key);
    stream.write_all(&sig_to_send)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

    // Acknowledge mutual authentication success
    stream.write_all(b"AUTH__OK")
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

    // 2. Database Transfer Phase (Receive DB from client)
    let mut size_buf = [0u8; 8];
    stream.read_exact(&mut size_buf)
        .map_err(|e| crate::error::VaultError::DecryptionError(format!("Failed to read database size: {}", e)))?;
    let db_size = u64::from_be_bytes(size_buf) as usize;

    if db_size > MAX_DB_SIZE {
        let _ = stream.write_all(b"SIZE_REJ");
        return Err(crate::error::VaultError::InvalidFormat(
            format!("Received DB size {} exceeds maximum {} bytes", db_size, MAX_DB_SIZE)
        ));
    }

    let mut db_data = vec![0u8; db_size];
    stream.read_exact(&mut db_data)
        .map_err(|e| crate::error::VaultError::DecryptionError(format!("Failed to read database data: {}", e)))?;

    // Merge client data into local database and save
    let (stats, final_data, saved_bytes) = match apply_and_save_remote_vault(&db_data, subkeys, db_filepath) {
        Ok(res) => res,
        Err(e) => {
            if let crate::error::VaultError::SyncError(_) = &e {
                let _ = stream.write_all(b"SALT_MIS");
            }
            return Err(e);
        }
    };

    // 3. Database Return Phase (Send merged DB back to client for mutual two-way synchronization)
    let merged_size = saved_bytes.len() as u64;
    stream.write_all(&merged_size.to_be_bytes())
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("Failed to send merged database size: {}", e)))?;
    stream.write_all(&saved_bytes)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("Failed to send merged database data: {}", e)))?;
    stream.flush()
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("Failed to flush sync stream: {}", e)))?;

    Ok((stats, final_data))
}

/// Connects as a client to a run_p2p_sync_listener peer.
/// Authenticates using mutual challenge-response, sends the local database,
/// receives the merged database back from the listener, updates the local database,
/// and returns the client-side merge statistics and updated vault data.
pub fn run_p2p_sync_client(
    server_addr: &str,
    subkeys: &crate::crypto::SubKeys,
    db_filepath: &Path,
) -> crate::Result<(MergeStats, crate::vault::types::VaultData)> {
    let mut stream = None;
    for attempt in 0..20 {
        match TcpStream::connect(server_addr) {
            Ok(s) => {
                stream = Some(s);
                break;
            }
            Err(_) if attempt < 19 => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                return Err(crate::error::VaultError::EncryptionError(format!("Failed to connect to sync server: {}", e)));
            }
        }
    }
    let mut stream = stream.unwrap();

    let timeout = Some(std::time::Duration::from_secs(30));
    let _ = stream.set_read_timeout(timeout);
    let _ = stream.set_write_timeout(timeout);

    let local_salt = if db_filepath.exists() && fs::metadata(db_filepath).map(|m| m.len() > 0).unwrap_or(false) {
        fs::read(db_filepath)
            .ok()
            .and_then(|bytes| VaultFile::from_bytes(&bytes).ok().map(|vf| vf.header.salt))
    } else {
        None
    };

    // 1. Handshake Phase
    // Step 1a: Exchange root salt markers to detect divergent vaults early
    let mut server_salt_buf = [0u8; 32];
    stream.read_exact(&mut server_salt_buf)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

    let client_has_salt = local_salt.is_some();
    let client_salt_buf = local_salt.unwrap_or([0u8; 32]);
    stream.write_all(&client_salt_buf)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

    if client_has_salt && server_salt_buf != [0u8; 32] && client_salt_buf != server_salt_buf {
        return Err(crate::error::VaultError::SyncError(
            "P2P sync rejected: Remote peer vault originates from a different root salt. Both devices must share the same initial vault database to sync.".into(),
        ));
    }

    // Step 1b: Mutual challenge-response HMAC authentication (Client-First Verification)
    let mut server_challenge = [0u8; 32];
    stream.read_exact(&mut server_challenge)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

    if server_challenge == P2P_SALT_MISMATCH_MARKER {
        return Err(crate::error::VaultError::SyncError(
            "P2P sync rejected: Remote peer vault originates from a different root salt. Both devices must share the same initial vault database to sync.".into(),
        ));
    }

    let mut client_challenge = [0u8; 32];
    rand::rng().fill(&mut client_challenge);

    // Send client challenge
    stream.write_all(&client_challenge)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

    // Send client signature over server_challenge
    let client_sig = compute_hmac(&server_challenge, &subkeys.hmac_key);
    stream.write_all(&client_sig)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

    // Read server signature
    let mut server_sig = [0u8; 64];
    if let Err(e) = stream.read_exact(&mut server_sig) {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            return Err(crate::error::VaultError::DecryptionError("Server rejected client authentication: Master password mismatch".into()));
        }
        return Err(crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)));
    }

    if &server_sig[..8] == b"UNAUTHOR" {
        return Err(crate::error::VaultError::DecryptionError("Server rejected client authentication: Master password mismatch".into()));
    }

    // Verify server signature
    if verify_hmac(&client_challenge, &server_sig, &subkeys.hmac_key).is_err() {
        let _ = stream.write_all(&P2P_AUTH_FAILED_SIG);
        return Err(crate::error::VaultError::DecryptionError("Server verification failed: Master password mismatch".into()));
    }

    // Await server authentication acknowledgment before transmitting database
    let mut auth_ack = [0u8; 8];
    if let Err(e) = stream.read_exact(&mut auth_ack) {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            return Err(crate::error::VaultError::DecryptionError("Server rejected client authentication: Master password mismatch".into()));
        }
        return Err(crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)));
    }

    if &auth_ack == b"UNAUTHOR" {
        return Err(crate::error::VaultError::DecryptionError("Server rejected client authentication: Master password mismatch".into()));
    }
    if &auth_ack != b"AUTH__OK" {
        return Err(crate::error::VaultError::EncryptionError("Invalid authentication handshake response from server".into()));
    }

    // 2. Database Transfer Phase (Send local DB to server)
    let file_data = fs::read(db_filepath)
        .map_err(|e| crate::error::VaultError::SerializationError(format!("Failed to read database: {}", e)))?;

    let db_size = file_data.len() as u64;
    stream.write_all(&db_size.to_be_bytes())
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("Failed to send database size: {}", e)))?;

    stream.write_all(&file_data)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("Failed to send database data: {}", e)))?;
    stream.flush()
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("Failed to flush database to server: {}", e)))?;

    // 3. Database Receive Phase (Receive merged DB back from server)
    let mut size_buf = [0u8; 8];
    stream.read_exact(&mut size_buf)
        .map_err(|e| crate::error::VaultError::DecryptionError(format!("Failed to read merged database size: {}", e)))?;

    if &size_buf[..] == b"SALT_MIS" {
        return Err(crate::error::VaultError::SyncError(
            "P2P sync rejected: Remote peer vault originates from a different root salt. Both devices must share the same initial vault database to sync.".into(),
        ));
    }
    if &size_buf[..] == b"UNAUTHOR" {
        return Err(crate::error::VaultError::DecryptionError("Server rejected client authentication: Master password mismatch".into()));
    }
    if &size_buf[..] == b"SIZE_REJ" {
        return Err(crate::error::VaultError::InvalidFormat("Server rejected database size".into()));
    }

    let merged_size = u64::from_be_bytes(size_buf) as usize;
    if merged_size > MAX_DB_SIZE {
        return Err(crate::error::VaultError::InvalidFormat(
            format!("Received merged DB size {} exceeds maximum {} bytes", merged_size, MAX_DB_SIZE)
        ));
    }

    let mut merged_data = vec![0u8; merged_size];
    stream.read_exact(&mut merged_data)
        .map_err(|e| crate::error::VaultError::DecryptionError(format!("Failed to read merged database data: {}", e)))?;

    // Merge server response into local database and save
    let (stats, final_data, _) = apply_and_save_remote_vault(&merged_data, subkeys, db_filepath)?;

    Ok((stats, final_data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_p2p_handshake_and_sync() {
        let temp_dir = tempdir().unwrap();
        let server_db_path = temp_dir.path().join("server.vdb");
        let client_db_path = temp_dir.path().join("client.vdb");

        let password = "TestP2pSyncPassword#2026";
        let mut server_mgr = crate::vault::manager::VaultManager::create("Sync Vault", password, &server_db_path).unwrap();

        // Simulate second device holding a copy of the vault before local edits
        fs::copy(&server_db_path, &client_db_path).unwrap();
        let mut client_mgr = crate::vault::manager::VaultManager::open(&client_db_path, password).unwrap();

        // Server adds a server-only entry
        server_mgr.add_entry(crate::vault::manager::NewEntry {
            title: "Server Item".to_string(),
            username: "server_user".to_string(),
            password: "server_password".to_string(),
            url: "https://server.example.com".to_string(),
            email: "server@example.com".to_string(),
            notes: "".to_string(),
            tags: Vec::new(),
            totp_secret: None,
            custom_fields: Vec::new(),
            entry_type: Some(crate::vault::types::EntryType::Login),
            generate_passkey: None,
            attachments: None,
        }).unwrap();
        let server_subkeys = server_mgr.get_subkeys().unwrap().clone();

        // Client adds a client-only entry
        client_mgr.add_entry(crate::vault::manager::NewEntry {
            title: "Client Item".to_string(),
            username: "client_user".to_string(),
            password: "client_password".to_string(),
            url: "https://client.example.com".to_string(),
            email: "client@example.com".to_string(),
            notes: "".to_string(),
            tags: Vec::new(),
            totp_secret: None,
            custom_fields: Vec::new(),
            entry_type: Some(crate::vault::types::EntryType::Login),
            generate_passkey: None,
            attachments: None,
        }).unwrap();
        let client_subkeys = client_mgr.get_subkeys().unwrap().clone();

        let addr = "127.0.0.1:49158"; // High ephemeral port

        // Start listener on a background thread
        let srv_path = server_db_path.clone();
        let srv_keys = server_subkeys.clone();
        let handle = std::thread::spawn(move || {
            run_p2p_sync_listener(addr, &srv_keys, &srv_path)
        });

        // Small pause to allow thread listener to bind
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Connect as client (two-way sync)
        let (client_stats, client_merged_data) = run_p2p_sync_client(addr, &client_subkeys, &client_db_path).unwrap();

        // Join thread and assert success on listener
        let (stats, merged_data) = handle.join().unwrap().unwrap();
        assert_eq!(stats.entries_added, 1);
        assert_eq!(merged_data.entries.len(), 2);

        // Verify that server DB on disk can be decrypted and contains both entries
        let opened_server = crate::vault::manager::VaultManager::open(&server_db_path, password).unwrap();
        assert_eq!(opened_server.data.entries.len(), 2);
        let titles: Vec<String> = opened_server.data.entries.iter().map(|e| e.title.clone()).collect();
        assert!(titles.contains(&"Server Item".to_string()));
        assert!(titles.contains(&"Client Item".to_string()));

        // Verify that client DB in memory and on disk also contains both entries (two-way sync)
        assert_eq!(client_stats.entries_added, 1);
        assert_eq!(client_merged_data.entries.len(), 2);
        let opened_client = crate::vault::manager::VaultManager::open(&client_db_path, password).unwrap();
        assert_eq!(opened_client.data.entries.len(), 2);
        let client_titles: Vec<String> = opened_client.data.entries.iter().map(|e| e.title.clone()).collect();
        assert!(client_titles.contains(&"Server Item".to_string()));
        assert!(client_titles.contains(&"Client Item".to_string()));
    }

    #[test]
    fn test_3way_vault_data_merge() {
        use chrono::Utc;
        use uuid::Uuid;
        use crate::vault::types::{VaultData, VaultMetadata, Entry, EntryType, BreachStatus, VaultSettings};
        use crate::crypto::cipher::EncryptedBlob;

        let now = Utc::now();
        let old_time = now - chrono::Duration::hours(2);
        let new_time = now - chrono::Duration::minutes(10);

        let id_common = Uuid::new_v4();
        let id_local_only = Uuid::new_v4();
        let id_remote_only = Uuid::new_v4();

        // Local vault
        let entry_common_local = Entry {
            id: id_common,
            title: "Common Title (Old Local)".into(),
            username: "user".into(),
            encrypted_password: EncryptedBlob { nonce: vec![0; 24], ciphertext: vec![1] },
            url: "https://example.com".into(),
            email: "test@example.com".into(),
            notes: "".into(),
            tags: vec!["LocalTag".into()],
            favorite: false,
            pinned: false,
            encrypted_totp_secret: None,
            custom_fields: vec![],
            entry_type: EntryType::Login,
            created_at: old_time,
            updated_at: old_time,
            password_history: vec![],
            breach_status: BreachStatus::Unknown,
            strength_score: None,
            password_changed_at: old_time,
            encrypted_passkey: None,
            passkey_public_key: None,
            attachments: vec![],
        };

        let entry_local_only = Entry {
            id: id_local_only,
            title: "Local Only".into(),
            username: "user_local".into(),
            encrypted_password: EncryptedBlob { nonce: vec![0; 24], ciphertext: vec![2] },
            url: "".into(),
            email: "".into(),
            notes: "".into(),
            tags: vec![],
            favorite: true,
            pinned: false,
            encrypted_totp_secret: None,
            custom_fields: vec![],
            entry_type: EntryType::Login,
            created_at: now,
            updated_at: now,
            password_history: vec![],
            breach_status: BreachStatus::Unknown,
            strength_score: None,
            password_changed_at: now,
            encrypted_passkey: None,
            passkey_public_key: None,
            attachments: vec![],
        };

        let mut local = VaultData {
            metadata: VaultMetadata { id: Uuid::new_v4(), name: "Local".into(), created_at: now, updated_at: now, entry_count: 2, version: 3 },
            entries: vec![entry_common_local, entry_local_only],
            tags: vec![],
            trash: vec![],
            settings: VaultSettings::default(),
        };

        // Remote vault (has updated common entry and a remote-only entry)
        let entry_common_remote = Entry {
            id: id_common,
            title: "Common Title (Updated Remote)".into(),
            username: "user".into(),
            encrypted_password: EncryptedBlob { nonce: vec![0; 24], ciphertext: vec![3] },
            url: "https://example.com".into(),
            email: "test@example.com".into(),
            notes: "Updated note".into(),
            tags: vec!["RemoteTag".into()],
            favorite: true,
            pinned: false,
            encrypted_totp_secret: None,
            custom_fields: vec![],
            entry_type: EntryType::Login,
            created_at: old_time,
            updated_at: new_time, // Newer timestamp!
            password_history: vec![],
            breach_status: BreachStatus::Unknown,
            strength_score: None,
            password_changed_at: new_time,
            encrypted_passkey: None,
            passkey_public_key: None,
            attachments: vec![],
        };

        let entry_remote_only = Entry {
            id: id_remote_only,
            title: "Remote Only".into(),
            username: "user_remote".into(),
            encrypted_password: EncryptedBlob { nonce: vec![0; 24], ciphertext: vec![4] },
            url: "".into(),
            email: "".into(),
            notes: "".into(),
            tags: vec![],
            favorite: false,
            pinned: false,
            encrypted_totp_secret: None,
            custom_fields: vec![],
            entry_type: EntryType::Login,
            created_at: now,
            updated_at: now,
            password_history: vec![],
            breach_status: BreachStatus::Unknown,
            strength_score: None,
            password_changed_at: now,
            encrypted_passkey: None,
            passkey_public_key: None,
            attachments: vec![],
        };

        let remote = VaultData {
            metadata: VaultMetadata { id: Uuid::new_v4(), name: "Remote".into(), created_at: now, updated_at: now, entry_count: 2, version: 3 },
            entries: vec![entry_common_remote, entry_remote_only],
            tags: vec![],
            trash: vec![],
            settings: VaultSettings::default(),
        };

        let stats = merge_vault_data(&mut local, remote);

        assert_eq!(stats.entries_added, 1);
        assert_eq!(stats.entries_updated, 1);
        assert_eq!(local.entries.len(), 3);

        let common_res = local.entries.iter().find(|e| e.id == id_common).unwrap();
        assert_eq!(common_res.title, "Common Title (Updated Remote)");
    }

    #[test]
    fn test_tombstone_aware_trash_merge() {
        use chrono::Utc;
        use uuid::Uuid;
        use crate::vault::types::{VaultData, VaultMetadata, Entry, EntryType, BreachStatus, VaultSettings, TrashedEntry};
        use crate::crypto::cipher::EncryptedBlob;

        let now = Utc::now();
        let old_time = now - chrono::Duration::hours(2);
        let trash_time = now - chrono::Duration::minutes(10);

        let id_trashed_local = Uuid::new_v4();

        // Local entry was trashed at `trash_time`
        let entry_trashed = Entry {
            id: id_trashed_local,
            title: "Trashed Item".into(),
            username: "user".into(),
            encrypted_password: EncryptedBlob { nonce: vec![0; 24], ciphertext: vec![1] },
            url: "".into(),
            email: "".into(),
            notes: "".into(),
            tags: vec![],
            favorite: false,
            pinned: false,
            encrypted_totp_secret: None,
            custom_fields: vec![],
            entry_type: EntryType::Login,
            created_at: old_time,
            updated_at: old_time,
            password_history: vec![],
            breach_status: BreachStatus::Unknown,
            strength_score: None,
            password_changed_at: old_time,
            encrypted_passkey: None,
            passkey_public_key: None,
            attachments: vec![],
        };

        let mut local = VaultData {
            metadata: VaultMetadata { id: Uuid::new_v4(), name: "Local".into(), created_at: now, updated_at: now, entry_count: 0, version: 3 },
            entries: vec![],
            tags: vec![],
            trash: vec![TrashedEntry { entry: entry_trashed.clone(), deleted_at: trash_time }],
            settings: VaultSettings::default(),
        };

        // Remote vault still has the old entry in active entries (updated at `old_time` < `trash_time`)
        let remote = VaultData {
            metadata: VaultMetadata { id: Uuid::new_v4(), name: "Remote".into(), created_at: now, updated_at: now, entry_count: 1, version: 3 },
            entries: vec![entry_trashed],
            tags: vec![],
            trash: vec![],
            settings: VaultSettings::default(),
        };

        let stats = merge_vault_data(&mut local, remote);

        // Active entries should remain empty (not resurrected!), item remains in trash
        assert_eq!(local.entries.len(), 0);
        assert_eq!(local.trash.len(), 1);
        assert_eq!(stats.entries_added, 0);
    }

    #[test]
    fn test_restored_entry_wins_over_remote_trash_tombstone() {
        use chrono::Utc;
        use uuid::Uuid;
        use crate::vault::types::{VaultData, VaultMetadata, Entry, EntryType, BreachStatus, VaultSettings, TrashedEntry};
        use crate::crypto::cipher::EncryptedBlob;

        let now = Utc::now();
        let old_time = now - chrono::Duration::hours(2);
        let trash_time = now - chrono::Duration::minutes(30);
        let restore_time = now - chrono::Duration::minutes(5);

        let id = Uuid::new_v4();

        // Local restored this entry at `restore_time` (restore_time > trash_time)
        let mut entry = Entry {
            id,
            title: "Restored Item".into(),
            username: "user".into(),
            encrypted_password: EncryptedBlob { nonce: vec![0; 24], ciphertext: vec![1] },
            url: "".into(),
            email: "".into(),
            notes: "".into(),
            tags: vec![],
            favorite: false,
            pinned: false,
            encrypted_totp_secret: None,
            custom_fields: vec![],
            entry_type: EntryType::Login,
            created_at: old_time,
            updated_at: restore_time,
            password_history: vec![],
            breach_status: BreachStatus::Unknown,
            strength_score: None,
            password_changed_at: old_time,
            encrypted_passkey: None,
            passkey_public_key: None,
            attachments: vec![],
        };

        let mut local = VaultData {
            metadata: VaultMetadata { id: Uuid::new_v4(), name: "Local".into(), created_at: now, updated_at: now, entry_count: 1, version: 3 },
            entries: vec![entry.clone()],
            tags: vec![],
            trash: vec![],
            settings: VaultSettings::default(),
        };

        // Remote still has the entry in trash with deleted_at = trash_time (< restore_time)
        entry.updated_at = old_time;
        let remote = VaultData {
            metadata: VaultMetadata { id: Uuid::new_v4(), name: "Remote".into(), created_at: now, updated_at: now, entry_count: 0, version: 3 },
            entries: vec![],
            tags: vec![],
            trash: vec![TrashedEntry { entry, deleted_at: trash_time }],
            settings: VaultSettings::default(),
        };

        let stats = merge_vault_data(&mut local, remote);

        // Entry should remain active and NOT be re-trashed by stale remote tombstone
        assert_eq!(local.entries.len(), 1);
        assert_eq!(local.trash.len(), 0);
        assert_eq!(stats.trash_merged, 0);
    }

    #[test]
    fn test_remote_restored_entry_wins_over_local_trash_tombstone() {
        use chrono::Utc;
        use uuid::Uuid;
        use crate::vault::types::{VaultData, VaultMetadata, Entry, EntryType, BreachStatus, VaultSettings, TrashedEntry};
        use crate::crypto::cipher::EncryptedBlob;

        let now = Utc::now();
        let old_time = now - chrono::Duration::hours(2);
        let trash_time = now - chrono::Duration::minutes(30);
        let restore_time = now - chrono::Duration::minutes(5);

        let id = Uuid::new_v4();

        let mut entry = Entry {
            id,
            title: "Restored Remotely".into(),
            username: "user".into(),
            encrypted_password: EncryptedBlob { nonce: vec![0; 24], ciphertext: vec![1] },
            url: "".into(),
            email: "".into(),
            notes: "".into(),
            tags: vec![],
            favorite: false,
            pinned: false,
            encrypted_totp_secret: None,
            custom_fields: vec![],
            entry_type: EntryType::Login,
            created_at: old_time,
            updated_at: old_time,
            password_history: vec![],
            breach_status: BreachStatus::Unknown,
            strength_score: None,
            password_changed_at: old_time,
            encrypted_passkey: None,
            passkey_public_key: None,
            attachments: vec![],
        };

        // Local has item in trash with deleted_at = trash_time
        let mut local = VaultData {
            metadata: VaultMetadata { id: Uuid::new_v4(), name: "Local".into(), created_at: now, updated_at: now, entry_count: 0, version: 3 },
            entries: vec![],
            tags: vec![],
            trash: vec![TrashedEntry { entry: entry.clone(), deleted_at: trash_time }],
            settings: VaultSettings::default(),
        };

        // Remote restored entry at restore_time (> trash_time)
        entry.updated_at = restore_time;
        let remote = VaultData {
            metadata: VaultMetadata { id: Uuid::new_v4(), name: "Remote".into(), created_at: now, updated_at: now, entry_count: 1, version: 3 },
            entries: vec![entry],
            tags: vec![],
            trash: vec![],
            settings: VaultSettings::default(),
        };

        let stats = merge_vault_data(&mut local, remote);

        // Remote restore should resurrect entry on local, removing local tombstone
        assert_eq!(local.entries.len(), 1);
        assert_eq!(local.trash.len(), 0);
        assert_eq!(stats.entries_added, 1);
    }

    #[test]
    fn test_normalize_etag() {
        assert_eq!(normalize_etag("\"12345\""), "12345");
        assert_eq!(normalize_etag("W/\"abcde\""), "abcde");
        assert_eq!(normalize_etag("w/\"xyz123\""), "xyz123");
        assert_eq!(normalize_etag("  \"hello\"  "), "hello");
    }

    #[test]
    fn test_validate_webdav_url() {
        assert!(validate_webdav_url("https://dav.example.com/vault.vdb").is_ok());
        assert!(validate_webdav_url("http://localhost:8080/vault.vdb").is_ok());
        assert!(validate_webdav_url("http://127.0.0.1:8080/vault.vdb").is_ok());
        assert!(validate_webdav_url("http://[::1]:8080/vault.vdb").is_ok());

        // Adversarial userinfo and spoofing bypass attempts must be rejected
        assert!(validate_webdav_url("http://localhost:80@attacker.com/vault.vdb").is_err());
        assert!(validate_webdav_url("http://attacker.com#localhost").is_err());
        assert!(validate_webdav_url("http://attacker.com/localhost").is_err());
        assert!(validate_webdav_url("http://unencrypted.com/vault.vdb").is_err());
        assert!(validate_webdav_url("ftp://server.com/vault.vdb").is_err());
        assert!(validate_webdav_url("").is_err());
    }

    #[test]
    fn test_decrypt_remote_vault_bytes_checked_salt_mismatch() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("vault.vdb");
        let password = "TestSaltMismatchPassword#1";

        let manager = crate::vault::manager::VaultManager::create("Vault", password, &db_path).unwrap();
        let subkeys = manager.get_subkeys().unwrap().clone();
        let correct_salt = manager.salt();

        let bytes = fs::read(&db_path).unwrap();

        // Success with matching salt
        let res_ok = decrypt_remote_vault_bytes_checked(&bytes, &subkeys, Some(&correct_salt));
        assert!(res_ok.is_ok());

        // Failure with wrong expected salt
        let wrong_salt = [99u8; 32];
        let res_err = decrypt_remote_vault_bytes_checked(&bytes, &subkeys, Some(&wrong_salt));
        assert!(res_err.is_err());
        match res_err.err().unwrap() {
            crate::error::VaultError::SyncError(msg) => {
                assert!(msg.contains("different root salt"));
            }
            other => panic!("Expected SyncError for salt mismatch, got {:?}", other),
        }
    }

    #[test]
    fn test_apply_and_save_remote_vault_different_salts_rejected() {
        let temp_dir = tempdir().unwrap();
        let vault_a_path = temp_dir.path().join("vault_a.vdb");
        let vault_b_path = temp_dir.path().join("vault_b.vdb");
        let password = "IdenticalPassword#123";

        // Vault A and Vault B are independently created with the same password -> different salts
        let mgr_a = crate::vault::manager::VaultManager::create("Vault A", password, &vault_a_path).unwrap();
        let _mgr_b = crate::vault::manager::VaultManager::create("Vault B", password, &vault_b_path).unwrap();

        let subkeys_a = mgr_a.get_subkeys().unwrap().clone();
        let bytes_b = fs::read(&vault_b_path).unwrap();

        // Attempting to merge Vault B into Vault A's file path using Vault A's subkeys
        let res = apply_and_save_remote_vault(&bytes_b, &subkeys_a, &vault_a_path);
        assert!(res.is_err());
        match res.err().unwrap() {
            crate::error::VaultError::SyncError(msg) => {
                assert!(msg.contains("different root salt"));
            }
            other => panic!("Expected SyncError for different root salts, got {:?}", other),
        }
    }

    #[test]
    fn test_p2p_sync_different_salts_rejected_with_diagnostic_error() {
        let temp_dir = tempdir().unwrap();
        let server_path = temp_dir.path().join("server.vdb");
        let client_path = temp_dir.path().join("client.vdb");
        let password = "SamePasswordBothDevices#123";

        // Two separately created vaults with different salts
        let server_mgr = crate::vault::manager::VaultManager::create("Server Vault", password, &server_path).unwrap();
        let client_mgr = crate::vault::manager::VaultManager::create("Client Vault", password, &client_path).unwrap();

        let srv_keys = server_mgr.get_subkeys().unwrap().clone();
        let client_keys = client_mgr.get_subkeys().unwrap().clone();

        let addr = "127.0.0.1:49159";

        let srv_path = server_path.clone();
        let srv_handle = std::thread::spawn(move || {
            run_p2p_sync_listener(addr, &srv_keys, &srv_path)
        });

        std::thread::sleep(std::time::Duration::from_millis(100));

        let client_res = run_p2p_sync_client(addr, &client_keys, &client_path);
        let srv_res = srv_handle.join().unwrap();

        // Both client and server must reject with SyncError indicating root salt divergence
        assert!(client_res.is_err());
        match client_res.err().unwrap() {
            crate::error::VaultError::SyncError(msg) => {
                assert!(msg.contains("different root salt"));
            }
            other => panic!("Expected SyncError for client, got {:?}", other),
        }

        assert!(srv_res.is_err());
        match srv_res.err().unwrap() {
            crate::error::VaultError::SyncError(msg) => {
                assert!(msg.contains("different root salt"));
            }
            other => panic!("Expected SyncError for server, got {:?}", other),
        }
    }

    #[test]
    fn test_p2p_sync_same_salt_wrong_password_rejected_with_password_error() {
        let temp_dir = tempdir().unwrap();
        let server_path = temp_dir.path().join("server.vdb");
        let client_path = temp_dir.path().join("client.vdb");
        let server_password = "ServerPasswordCorrect#123";

        let server_mgr = crate::vault::manager::VaultManager::create("Server Vault", server_password, &server_path).unwrap();

        // Replicate database file so client has the EXACT same salt
        fs::copy(&server_path, &client_path).unwrap();

        // Client derives subkeys using a wrong password with the server's salt
        let master_key_wrong = crate::crypto::derive_master_key(b"WrongPasswordEntirely#456", &server_mgr.salt()).unwrap();
        let client_keys_wrong = crate::crypto::derive_subkeys(&master_key_wrong).unwrap();
        let srv_keys = server_mgr.get_subkeys().unwrap().clone();

        let addr = "127.0.0.1:49160";

        let srv_path = server_path.clone();
        let srv_handle = std::thread::spawn(move || {
            run_p2p_sync_listener(addr, &srv_keys, &srv_path)
        });

        std::thread::sleep(std::time::Duration::from_millis(100));

        let client_res = run_p2p_sync_client(addr, &client_keys_wrong, &client_path);
        let srv_res = srv_handle.join().unwrap();

        // Both sides must report password mismatch specifically
        assert!(client_res.is_err());
        match client_res.err().unwrap() {
            crate::error::VaultError::DecryptionError(msg) => {
                assert!(msg.contains("Master password mismatch"));
            }
            other => panic!("Expected DecryptionError with password mismatch, got {:?}", other),
        }

        assert!(srv_res.is_err());
        match srv_res.err().unwrap() {
            crate::error::VaultError::DecryptionError(msg) => {
                assert!(msg.contains("Master password mismatch"));
            }
            crate::error::VaultError::EncryptionError(msg) => {
                assert!(msg.contains("P2P handshake failed"));
            }
            other => panic!("Expected DecryptionError or handshake abortion, got {:?}", other),
        }
    }
}

