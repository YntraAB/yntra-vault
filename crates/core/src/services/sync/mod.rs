//! Vault Synchronization Protocols (WebDAV cloud sync and local network P2P sync).

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket, SocketAddr, ToSocketAddrs, IpAddr, Ipv4Addr};
use std::path::Path;
use rand::Rng;
use subtle::ConstantTimeEq;
use crate::crypto::{compute_hmac, verify_hmac};
use crate::vault::format::VaultFile;

/// Maximum database size accepted during P2P sync (256 MB)
const MAX_DB_SIZE: usize = 256 * 1024 * 1024;

pub const DEFAULT_P2P_PORT: u16 = 5322;
pub const DEFAULT_DISCOVERY_PORT: u16 = 5323;
pub const DEFAULT_PAIRING_PORT: u16 = 5324;
pub const DISCOVERY_BEACON_MAGIC: [u8; 4] = *b"YBEA";
pub const PAIRING_QUERY_MAGIC: [u8; 4] = *b"YQRY";
pub const DISCOVERY_MULTICAST_ADDR: &str = "239.255.53.23";

pub mod pairing;
pub use pairing::{
    PairingStats, generate_pairing_code, normalize_pairing_code,
    derive_pairing_subkeys, compute_pairing_beacon_id,
    broadcast_pairing_beacon, listen_pairing_beacon,
    run_p2p_pairing_host, run_p2p_pairing_client,
    run_p2p_pairing_host_with_device, run_p2p_pairing_host_with_device_and_cancel,
    run_p2p_pairing_client_with_device,
    DeviceInfo, resolve_local_device_info, ClientPairingMode, sanitize_vault_filename,
    QrSessionInfo, QrClientPairingResult, QrPairingPayload, QrPairingSession,
    PendingAdoptedVault, complete_adopted_vault_save,
    generate_qr_pairing_session, parse_qr_pairing_payload,
    run_p2p_qr_pairing_host, run_p2p_qr_pairing_client,
    compute_qr_sas_code, derive_qr_pairing_subkeys,
};

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
    use crate::vault::types::{Entry, TrashedEntry};

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

    // 2. Merge Tags by UUID preserving order from the more recently modified vault
    let local_tags = std::mem::take(&mut local.tags);
    let newer_is_remote = remote.metadata.updated_at > local.metadata.updated_at;
    let (primary_tags, secondary_tags) = if newer_is_remote {
        (remote.tags, local_tags)
    } else {
        (local_tags, remote.tags)
    };

    let mut merged_tags = Vec::with_capacity(primary_tags.len() + secondary_tags.len());
    let mut seen_ids = std::collections::HashSet::with_capacity(primary_tags.len() + secondary_tags.len());

    for tag in primary_tags {
        seen_ids.insert(tag.id);
        merged_tags.push(tag);
    }
    for tag in secondary_tags {
        if seen_ids.insert(tag.id) {
            merged_tags.push(tag);
            stats.tags_merged += 1;
        }
    }
    local.tags = merged_tags;

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
    dest.sort_by_key(|b| std::cmp::Reverse(b.changed_at));
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

    if let Ok(response) = req.send().await
        && response.status().is_success()
            && let Some(etag) = response.headers().get("ETag").and_then(|h| h.to_str().ok()) {
                let trimmed = normalize_etag(etag);
                if !trimmed.is_empty() {
                    return Ok(Some(trimmed));
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

    if let Ok(pf_resp) = pf_req.send().await
        && (pf_resp.status().is_success() || pf_resp.status().as_u16() == 207) {
            if let Some(etag) = pf_resp.headers().get("ETag").and_then(|h| h.to_str().ok()) {
                let trimmed = normalize_etag(etag);
                if !trimmed.is_empty() {
                    return Ok(Some(trimmed));
                }
            }
            if let Ok(body) = pf_resp.text().await
                && let Some(start) = body.find("<getetag>") {
                    let rest = &body[start + 9..];
                    if let Some(end) = rest.find("</getetag>") {
                        let etag_val = normalize_etag(&rest[..end]);
                        if !etag_val.is_empty() {
                            return Ok(Some(etag_val));
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
    use crate::vault::format::VaultFile;
    use crate::crypto::cipher::{decrypt_vault, decrypt_vault_with_aad, EncryptedBlob};
    use crate::crypto::verify_hmac;

    let vault_file = VaultFile::from_bytes(bytes)?;

    if let Some(expected) = expected_salt
        && &vault_file.header.salt != expected {
            return Err(crate::error::VaultError::SyncError(
                "Remote vault originates from a different root salt. Synchronizing two independently created vaults is not supported; use the same vault file across devices.".into(),
            ));
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

    let decrypted = zeroize::Zeroizing::new(if vault_file.header.version >= 3 {
        let aad = vault_file.header.aad_bytes()?;
        decrypt_vault_with_aad(&encrypted_blob, &subkeys.vault_key, &aad)?
    } else {
        decrypt_vault(&encrypted_blob, &subkeys.vault_key)?
    });

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
    peer_device_id: Option<uuid::Uuid>,
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
        let stats = merge_vault_data(&mut local_data, remote_data.clone());

        // If peer_device_id is provided, update its last_sync_at timestamp in local trusted devices
        if let Some(peer_id) = peer_device_id {
            if !peer_id.is_nil() {
                for dev in &mut local_data.settings.trusted_devices {
                    if dev.id == peer_id {
                        dev.last_sync_at = Some(chrono::Utc::now());
                    }
                }
            }
        } else {
            // When client receives merged DB back from server, adopt the authoritative trusted devices list from server
            if !remote_data.settings.trusted_devices.is_empty() {
                local_data.settings.trusted_devices = remote_data.settings.trusted_devices;
            }
        }

        // Deduplicate local trusted devices to guarantee clean device list
        let mut seen_ids = std::collections::HashSet::new();
        let mut seen_names = std::collections::HashSet::new();
        let mut deduped = Vec::new();
        for dev in local_data.settings.trusted_devices.drain(..) {
            let name_key = (dev.name.to_lowercase(), dev.os.to_lowercase());
            if seen_ids.insert(dev.id) && seen_names.insert(name_key) {
                deduped.push(dev);
            }
        }
        local_data.settings.trusted_devices = deduped;

        // Update metadata timestamp and entry count
        local_data.metadata.updated_at = chrono::Utc::now();
        local_data.metadata.entry_count = local_data.entries.len();

        // Clean up old trash (> 30 days)
        let cutoff = chrono::Utc::now() - chrono::Duration::days(30);
        local_data.trash.retain(|t| t.deleted_at > cutoff);

        // Re-serialize vault data as MessagePack with zeroized memory on scope exit
        let serialized = zeroize::Zeroizing::new(
            rmp_serde::to_vec(&local_data)
                .map_err(|e| crate::error::VaultError::SerializationError(format!("Vault serialize: {}", e)))?
        );

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
        let tmp_path = db_filepath.with_extension(format!("vdb.sync.{}.tmp", uuid::Uuid::new_v4().simple()));
        fs::write(&tmp_path, &merged_bytes)
            .map_err(|e| crate::error::VaultError::SerializationError(format!("Failed to write temp sync file: {}", e)))?;
        fs::rename(&tmp_path, db_filepath)
            .map_err(|e| crate::error::VaultError::SerializationError(format!("Failed to rename sync file: {}", e)))?;

        (stats, local_data, merged_bytes)
    } else {
        let remote_data = decrypt_remote_vault_bytes_checked(remote_bytes, subkeys, Some(&remote_vault_file.header.salt))?;
        let tmp_path = db_filepath.with_extension(format!("vdb.sync.{}.tmp", uuid::Uuid::new_v4().simple()));
        fs::write(&tmp_path, remote_bytes)
            .map_err(|e| crate::error::VaultError::SerializationError(format!("Failed to write temp sync file: {}", e)))?;
        fs::rename(&tmp_path, db_filepath)
            .map_err(|e| crate::error::VaultError::SerializationError(format!("Failed to rename sync file: {}", e)))?;

        let stats = MergeStats {
            entries_added: remote_data.entries.len(),
            ..Default::default()
        };
        (stats, remote_data, remote_bytes.to_vec())
    };

    Ok((stats, final_data, saved_bytes))
}

const P2P_SALT_COMMITMENT_KEY: [u8; 32] = *b"yntra-p2p-salt-commitment-v1----";
pub const P2P_TRANSIT_AAD: &[u8] = b"yntra-p2p-transit-v1";
pub const P2P_SALT_MISMATCH_MARKER: [u8; 32] = *b"YNTRA_SYNC_ERR_SALT_MISMATCH____";
pub const P2P_AUTH_FAILED_SIG: [u8; 64] = *b"YNTRA_SYNC_ERR_AUTH_FAILED_MASTER_PASSWORD_MISMATCH_____________";
pub const P2P_REVOKED_SIG: [u8; 64] = *b"YNTRA_SYNC_ERR_DEVICE_REVOKED_UNTRUSTED_RE_PAIR_NEEDED__________";

/// Compute a zero-knowledge 32-byte salt commitment from a root salt.
/// Enables early detection of divergent root salts without leaking the raw Argon2id salt.
pub fn compute_salt_commitment(salt: &[u8; 32]) -> [u8; 32] {
    if salt == &[0u8; 32] {
        return [0u8; 32];
    }
    *blake3::keyed_hash(&P2P_SALT_COMMITMENT_KEY, salt).as_bytes()
}

/// Compute a zero-knowledge 32-byte beacon ID from the active HMAC subkey.
/// Mathematically isolates the token: only peers holding the exact same master password
/// and root salt can calculate this token, preventing discovery by unrelated devices.
pub fn compute_p2p_discovery_id(subkeys: &crate::crypto::SubKeys) -> [u8; 32] {
    let key_hash = *blake3::hash(&subkeys.hmac_key.bytes).as_bytes();
    *blake3::keyed_hash(&key_hash, b"yntra-vault-lan-discovery-beacon-v1").as_bytes()
}

fn is_valid_lan_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            !ipv4.is_loopback()
                && !ipv4.is_unspecified()
                && !ipv4.is_link_local()
                && !ipv4.is_broadcast()
        }
        IpAddr::V6(ipv6) => {
            !ipv6.is_loopback()
                && !ipv6.is_unspecified()
                && !ipv6.is_unicast_link_local()
                && !ipv6.is_multicast()
        }
    }
}

/// Enumerate all active non-loopback, non-link-local IPv4 and IPv6 addresses across all network adapters.
/// IPv4 addresses are ordered first to ensure reliable LAN discovery and display.
pub fn get_local_lan_ips() -> Vec<IpAddr> {
    let mut v4_ips = Vec::new();
    let mut v6_ips = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // 1. Hostname DNS resolution (resolves adapters configured on local host via OS)
    if let Ok(hostname) = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME"))
        && let Ok(addrs) = format!("{}:0", hostname).to_socket_addrs() {
            for addr in addrs {
                let ip = addr.ip();
                if is_valid_lan_ip(&ip) && seen.insert(ip) {
                    if ip.is_ipv4() {
                        v4_ips.push(ip);
                    } else {
                        v6_ips.push(ip);
                    }
                }
            }
        }

    // 2. Gateway and route socket probes across standard subnets
    let probes = [
        "8.8.8.8:80",
        "1.1.1.1:80",
        "192.168.1.1:80",
        "192.168.0.1:80",
        "192.168.2.1:80",
        "192.168.137.1:80",
        "10.0.0.1:80",
        "10.0.1.1:80",
        "172.16.0.1:80",
    ];

    for probe in probes {
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0")
            && socket.connect(probe).is_ok()
                && let Ok(local_addr) = socket.local_addr() {
                    let ip = local_addr.ip();
                    if is_valid_lan_ip(&ip) && seen.insert(ip) {
                        if ip.is_ipv4() {
                            v4_ips.push(ip);
                        } else {
                            v6_ips.push(ip);
                        }
                    }
                }
    }

    v4_ips.extend(v6_ips);
    v4_ips
}

/// Query the host's primary LAN IP address without external network traffic.
/// Guarantees an active IPv4 address when available.
pub fn get_local_lan_ip() -> Option<IpAddr> {
    get_local_lan_ips().into_iter().find(|ip| ip.is_ipv4())
}

/// Broadcast a single discovery beacon packet over the local network.
pub fn broadcast_discovery_beacon(discovery_id: &[u8; 32], tcp_port: u16) -> crate::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("Failed to bind UDP discovery socket: {}", e)))?;
    socket.set_broadcast(true)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("Failed to enable UDP broadcast: {}", e)))?;

    let mut packet = [0u8; 38];
    packet[..4].copy_from_slice(&DISCOVERY_BEACON_MAGIC);
    packet[4..36].copy_from_slice(discovery_id);
    packet[36..38].copy_from_slice(&tcp_port.to_be_bytes());

    // 1. Global broadcast
    let dest = format!("255.255.255.255:{}", DEFAULT_DISCOVERY_PORT);
    let _ = socket.send_to(&packet, &dest);

    // 2. Multicast group (RFC 2365 local administrative scope)
    let multi_dest = format!("{}:{}", DISCOVERY_MULTICAST_ADDR, DEFAULT_DISCOVERY_PORT);
    let _ = socket.send_to(&packet, &multi_dest);

    // 3. Directed subnet broadcasts for all detected local IPv4 adapters
    for ip in get_local_lan_ips() {
        if let IpAddr::V4(ipv4) = ip {
            let octets = ipv4.octets();
            let directed_dest = format!("{}.{}.{}.255:{}", octets[0], octets[1], octets[2], DEFAULT_DISCOVERY_PORT);
            let _ = socket.send_to(&packet, &directed_dest);
        }
    }

    Ok(())
}

/// Listen for an incoming discovery beacon matching the expected discovery ID within a timeout.
/// Returns the peer's resolved `SocketAddr` (peer IP + peer TCP port).
pub fn listen_discovery_beacon(
    expected_discovery_id: &[u8; 32],
    timeout: std::time::Duration,
) -> crate::Result<Option<SocketAddr>> {
    let listen_addr = format!("0.0.0.0:{}", DEFAULT_DISCOVERY_PORT);
    let socket = match UdpSocket::bind(&listen_addr) {
        Ok(s) => s,
        Err(_) => {
            UdpSocket::bind("0.0.0.0:0")
                .map_err(|e| crate::error::VaultError::EncryptionError(format!("Failed to bind UDP discovery listener: {}", e)))?
        }
    };

    let _ = socket.set_broadcast(true);
    if let Ok(multi_ip) = DISCOVERY_MULTICAST_ADDR.parse::<Ipv4Addr>() {
        let _ = socket.join_multicast_v4(&multi_ip, &Ipv4Addr::UNSPECIFIED);
    }

    socket.set_read_timeout(Some(std::time::Duration::from_millis(200)))
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("Failed to set UDP socket timeout: {}", e)))?;

    let local_ips = get_local_lan_ips();
    let start = std::time::Instant::now();
    let mut buf = [0u8; 64];

    while start.elapsed() < timeout {
        match socket.recv_from(&mut buf) {
            Ok((len, peer_addr)) => {
                if len >= 38 && buf[..4] == DISCOVERY_BEACON_MAGIC {
                    let received_id = &buf[4..36];
                    if received_id.ct_eq(expected_discovery_id).into() {
                        // Skip self-echo from our own local network adapters and loopback
                        if peer_addr.ip().is_loopback() || local_ips.iter().any(|lip| *lip == peer_addr.ip()) {
                            continue;
                        }
                        let peer_port = u16::from_be_bytes([buf[36], buf[37]]);
                        return Ok(Some(SocketAddr::new(peer_addr.ip(), peer_port)));
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                std::thread::yield_now();
            }
            Err(e) => {
                return Err(crate::error::VaultError::EncryptionError(format!("UDP discovery recv error: {}", e)));
            }
        }
    }

    Ok(None)
}

/// Runs a secure TCP listener for vault synchronization with timeout protection and discovery beaconing.
/// Verifies peer credentials via a mutual challenge-response handshake signed with HMAC key,
/// receives the peer's encrypted vault, performs a 3-way item-level merge with the local vault,
/// saves the merged vault atomically to disk, transmits the merged database back to the peer,
/// and returns the merge statistics and merged vault data.
pub fn run_p2p_sync_listener(
    listen_addr: &str,
    subkeys: &crate::crypto::SubKeys,
    db_filepath: &Path,
) -> crate::Result<(MergeStats, crate::vault::types::VaultData)> {
    run_p2p_sync_listener_with_timeout(listen_addr, subkeys, db_filepath, std::time::Duration::from_secs(45))
}

/// Runs a secure TCP listener for vault synchronization with configurable timeout and discovery beaconing.
pub fn run_p2p_sync_listener_with_timeout(
    listen_addr: &str,
    subkeys: &crate::crypto::SubKeys,
    db_filepath: &Path,
    accept_timeout: std::time::Duration,
) -> crate::Result<(MergeStats, crate::vault::types::VaultData)> {
    let listener = TcpListener::bind(listen_addr)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("Failed to bind TCP listener on {}: {}", listen_addr, e)))?;

    listener.set_nonblocking(true)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("Failed to set non-blocking TCP listener: {}", e)))?;

    let local_port = listener.local_addr().map(|a| a.port()).unwrap_or(DEFAULT_P2P_PORT);
    let discovery_id = compute_p2p_discovery_id(subkeys);

    let start_time = std::time::Instant::now();
    let mut last_beacon = std::time::Instant::now() - std::time::Duration::from_secs(10);

    let mut stream = loop {
        if last_beacon.elapsed() >= std::time::Duration::from_millis(1500) {
            let _ = broadcast_discovery_beacon(&discovery_id, local_port);
            last_beacon = std::time::Instant::now();
        }

        match listener.accept() {
            Ok((s, _)) => {
                s.set_nonblocking(false)
                    .map_err(|e| crate::error::VaultError::EncryptionError(format!("Failed to set blocking stream: {}", e)))?;
                break s;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if start_time.elapsed() >= accept_timeout {
                    return Err(crate::error::VaultError::SyncError(
                        "P2P sync listener timed out waiting for peer connection (45s)".into(),
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => {
                return Err(crate::error::VaultError::EncryptionError(format!("Failed to accept peer connection: {}", e)));
            }
        }
    };

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
    // Step 1a: Exchange zero-knowledge salt commitments to detect divergent vaults early without leaking raw salt
    let server_has_salt = local_salt.is_some();
    let server_commitment = local_salt.map(|s| compute_salt_commitment(&s)).unwrap_or([0u8; 32]);
    stream.write_all(&server_commitment)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

    let mut client_commitment = [0u8; 32];
    stream.read_exact(&mut client_commitment)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

    if server_has_salt && client_commitment != [0u8; 32] && server_commitment != client_commitment {
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

    // Read client device id (16 bytes UUID)
    let mut client_device_id_bytes = [0u8; 16];
    if let Err(e) = stream.read_exact(&mut client_device_id_bytes) {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            return Err(crate::error::VaultError::DecryptionError("Peer verification failed: Incomplete handshake".into()));
        }
        return Err(crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)));
    }

    // Verify client signature: authenticates server_challenge bound to client_device_id (with fallback for legacy challenge-only peers)
    let mut client_auth_payload = Vec::with_capacity(48);
    client_auth_payload.extend_from_slice(&server_challenge);
    client_auth_payload.extend_from_slice(&client_device_id_bytes);

    let is_auth_valid = verify_hmac(&client_auth_payload, &client_sig, &subkeys.hmac_key).is_ok()
        || verify_hmac(&server_challenge, &client_sig, &subkeys.hmac_key).is_ok();

    if client_sig == P2P_AUTH_FAILED_SIG || !is_auth_valid {
        let _ = stream.write_all(&P2P_AUTH_FAILED_SIG);
        let _ = stream.flush();
        return Err(crate::error::VaultError::DecryptionError("Peer verification failed: Master password mismatch".into()));
    }

    let client_device_uuid = uuid::Uuid::from_bytes(client_device_id_bytes);

    // If local database has trusted_devices configured, strictly enforce that connecting device is trusted!
    // Rejects both unlisted device UUIDs and attempts to bypass via Uuid::nil().
    if db_filepath.exists() {
        let local_bytes = fs::read(db_filepath)
            .map_err(|e| crate::error::VaultError::SyncError(format!("Failed to read local vault file for device authorization: {}", e)))?;
        let local_data = decrypt_remote_vault_bytes_checked(&local_bytes, subkeys, None)?;
        let trusted = &local_data.settings.trusted_devices;
        if !trusted.is_empty() {
            let is_trusted = !client_device_uuid.is_nil()
                && trusted.iter().any(|d| d.id == client_device_uuid);
            if !is_trusted {
                let _ = stream.write_all(&P2P_REVOKED_SIG);
                let _ = stream.flush();
                return Err(crate::error::VaultError::SyncError(format!(
                    "P2P sync rejected: Device {} is not in trusted devices list (revoked by host)",
                    client_device_uuid
                )));
            }
        }
    }

    // Compute and send server signature only after client has successfully authenticated
    let sig_to_send = compute_hmac(&client_challenge, &subkeys.hmac_key);
    stream.write_all(&sig_to_send)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

    // Acknowledge mutual authentication success
    stream.write_all(b"AUTH__OK")
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

    // 2. Database Transfer Phase (Receive transit-encrypted DB from client)
    let mut size_buf = [0u8; 8];
    stream.read_exact(&mut size_buf)
        .map_err(|e| crate::error::VaultError::DecryptionError(format!("Failed to read database size: {}", e)))?;
    let payload_size = u64::from_be_bytes(size_buf) as usize;

    if payload_size > MAX_DB_SIZE {
        let _ = stream.write_all(b"SIZE_REJ");
        return Err(crate::error::VaultError::InvalidFormat(
            format!("Received DB size {} exceeds maximum {} bytes", payload_size, MAX_DB_SIZE)
        ));
    }

    let mut payload_data = vec![0u8; payload_size];
    stream.read_exact(&mut payload_data)
        .map_err(|e| crate::error::VaultError::DecryptionError(format!("Failed to read database data: {}", e)))?;

    // Decrypt transit AEAD envelope (with transparent fallback for legacy unencrypted .vdb starting with YNTR)
    let db_data: zeroize::Zeroizing<Vec<u8>> = if payload_data.starts_with(crate::vault::format::MAGIC_BYTES) {
        zeroize::Zeroizing::new(payload_data)
    } else {
        let blob: crate::crypto::cipher::EncryptedBlob = rmp_serde::from_slice(&payload_data)
            .map_err(|e| crate::error::VaultError::SerializationError(format!("Failed to deserialize transit blob: {}", e)))?;
        crate::crypto::cipher::decrypt_vault_with_aad(&blob, &subkeys.vault_key, P2P_TRANSIT_AAD)?
    };

    // Merge client data into local database and save
    let (stats, final_data, saved_bytes) = match apply_and_save_remote_vault(&db_data, subkeys, db_filepath, Some(client_device_uuid)) {
        Ok(res) => res,
        Err(e) => {
            if let crate::error::VaultError::SyncError(_) = &e {
                let _ = stream.write_all(b"SALT_MIS");
            }
            return Err(e);
        }
    };

    // 3. Database Return Phase (Send transit-encrypted merged DB back to client for mutual two-way synchronization)
    let transit_blob = crate::crypto::cipher::encrypt_vault_with_aad(&saved_bytes, &subkeys.vault_key, P2P_TRANSIT_AAD)?;
    let transit_bytes = rmp_serde::to_vec(&transit_blob)
        .map_err(|e| crate::error::VaultError::SerializationError(format!("Failed to serialize transit return payload: {}", e)))?;

    let merged_size = transit_bytes.len() as u64;
    stream.write_all(&merged_size.to_be_bytes())
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("Failed to send merged database size: {}", e)))?;
    stream.write_all(&transit_bytes)
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
    run_p2p_sync_client_with_device(server_addr, subkeys, db_filepath, None)
}

/// Connects as a client to a run_p2p_sync_listener peer with device identity.
pub fn run_p2p_sync_client_with_device(
    server_addr: &str,
    subkeys: &crate::crypto::SubKeys,
    db_filepath: &Path,
    device_id: Option<uuid::Uuid>,
) -> crate::Result<(MergeStats, crate::vault::types::VaultData)> {
    let target_str = if !server_addr.contains(':') {
        format!("{}:{}", server_addr, DEFAULT_P2P_PORT)
    } else {
        server_addr.to_string()
    };

    let addrs: Vec<SocketAddr> = target_str.to_socket_addrs()
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("Invalid sync server address '{}': {}", server_addr, e)))?
        .collect();

    if addrs.is_empty() {
        return Err(crate::error::VaultError::EncryptionError(format!("Could not resolve sync server address '{}'", server_addr)));
    }

    let mut stream = None;
    let connect_timeout = std::time::Duration::from_millis(400);

    for attempt in 0..10 {
        for addr in &addrs {
            let candidate_ports = [addr.port(), DEFAULT_P2P_PORT, DEFAULT_PAIRING_PORT, 5325];
            for p in candidate_ports {
                let mut alt_addr = *addr;
                alt_addr.set_port(p);
                if let Ok(s) = TcpStream::connect_timeout(&alt_addr, connect_timeout) {
                    stream = Some(s);
                    break;
                }
            }
            if stream.is_some() {
                break;
            }
        }
        if stream.is_some() {
            break;
        }
        if attempt < 9 {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }
    let mut stream = stream.ok_or_else(|| {
        crate::error::VaultError::EncryptionError(format!("Failed to connect to sync server at {}", server_addr))
    })?;

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
    // Step 1a: Exchange zero-knowledge salt commitments to detect divergent vaults early without leaking raw salt
    let mut server_commitment = [0u8; 32];
    stream.read_exact(&mut server_commitment)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

    let client_has_salt = local_salt.is_some();
    let client_commitment = local_salt.map(|s| compute_salt_commitment(&s)).unwrap_or([0u8; 32]);
    stream.write_all(&client_commitment)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

    if client_has_salt && server_commitment != [0u8; 32] && client_commitment != server_commitment {
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

    // Send client device id (16 bytes UUID)
    let client_device_uuid = device_id.unwrap_or_else(|| pairing::resolve_local_device_info(None).id);
    let client_device_id_bytes = client_device_uuid.into_bytes();

    // Bind server challenge and device ID in client signature
    let mut client_auth_payload = Vec::with_capacity(48);
    client_auth_payload.extend_from_slice(&server_challenge);
    client_auth_payload.extend_from_slice(&client_device_id_bytes);
    let client_sig = compute_hmac(&client_auth_payload, &subkeys.hmac_key);

    stream.write_all(&client_sig)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

    stream.write_all(&client_device_id_bytes)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;
    stream.flush().map_err(|e| crate::error::VaultError::EncryptionError(format!("Flush client handshake failed: {}", e)))?;

    // Read server signature
    let mut server_sig = [0u8; 64];
    if let Err(e) = stream.read_exact(&mut server_sig) {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            return Err(crate::error::VaultError::DecryptionError("Server rejected client authentication: Master password mismatch".into()));
        }
        return Err(crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)));
    }

    if server_sig == P2P_REVOKED_SIG {
        return Err(crate::error::VaultError::SyncError(
            "This device has been disconnected by the host. Please re-pair using a 6-digit code.".into()
        ));
    }

    if server_sig == P2P_AUTH_FAILED_SIG || &server_sig[..8] == b"UNAUTHOR" {
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

    // 2. Database Transfer Phase (Send local DB to server wrapped in AEAD transit encryption)
    let file_data = fs::read(db_filepath)
        .map_err(|e| crate::error::VaultError::SerializationError(format!("Failed to read database: {}", e)))?;

    let transit_blob = crate::crypto::cipher::encrypt_vault_with_aad(&file_data, &subkeys.vault_key, P2P_TRANSIT_AAD)?;
    let transit_bytes = rmp_serde::to_vec(&transit_blob)
        .map_err(|e| crate::error::VaultError::SerializationError(format!("Failed to serialize transit payload: {}", e)))?;

    let db_size = transit_bytes.len() as u64;
    stream.write_all(&db_size.to_be_bytes())
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("Failed to send database size: {}", e)))?;

    stream.write_all(&transit_bytes)
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

    let merged_size = u64::from_be_bytes(size_buf) as usize;
    if merged_size > MAX_DB_SIZE {
        return Err(crate::error::VaultError::InvalidFormat(
            format!("Received merged DB size {} exceeds maximum {} bytes", merged_size, MAX_DB_SIZE)
        ));
    }

    let mut server_payload_data = vec![0u8; merged_size];
    stream.read_exact(&mut server_payload_data)
        .map_err(|e| crate::error::VaultError::DecryptionError(format!("Failed to read merged database data: {}", e)))?;

    let server_db_data: zeroize::Zeroizing<Vec<u8>> = if server_payload_data.starts_with(crate::vault::format::MAGIC_BYTES) {
        zeroize::Zeroizing::new(server_payload_data)
    } else {
        let blob: crate::crypto::cipher::EncryptedBlob = rmp_serde::from_slice(&server_payload_data)
            .map_err(|e| crate::error::VaultError::SerializationError(format!("Failed to deserialize transit blob: {}", e)))?;
        crate::crypto::cipher::decrypt_vault_with_aad(&blob, &subkeys.vault_key, P2P_TRANSIT_AAD)?
    };

    // Apply merged DB received from server to local database file
    let (stats, final_data, _) = apply_and_save_remote_vault(&server_db_data, subkeys, db_filepath, None)?;

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
    fn test_tag_order_preserved_on_merge() {
        use chrono::Utc;
        use uuid::Uuid;
        use crate::vault::types::{VaultData, VaultMetadata, Tag, VaultSettings};

        let now = Utc::now();
        let older = now - chrono::Duration::minutes(5);

        let t1 = Tag { id: Uuid::new_v4(), name: "Work".into(), color: "#f00".into(), icon: "briefcase".into() };
        let t2 = Tag { id: Uuid::new_v4(), name: "Personal".into(), color: "#0f0".into(), icon: "user".into() };
        let t3 = Tag { id: Uuid::new_v4(), name: "Finance".into(), color: "#00f".into(), icon: "wallet".into() };
        let t4 = Tag { id: Uuid::new_v4(), name: "Crypto".into(), color: "#ff0".into(), icon: "key".into() };

        // 1. Remote is newer: Remote order must win, with local-only tags appended
        let mut local = VaultData {
            metadata: VaultMetadata { id: Uuid::new_v4(), name: "Local".into(), created_at: older, updated_at: older, entry_count: 0, version: 3 },
            entries: vec![],
            tags: vec![t1.clone(), t2.clone(), t3.clone()],
            trash: vec![],
            settings: VaultSettings::default(),
        };

        let remote = VaultData {
            metadata: VaultMetadata { id: Uuid::new_v4(), name: "Remote".into(), created_at: older, updated_at: now, entry_count: 0, version: 3 },
            entries: vec![],
            tags: vec![t3.clone(), t1.clone(), t4.clone()],
            trash: vec![],
            settings: VaultSettings::default(),
        };

        let stats = merge_vault_data(&mut local, remote);
        assert_eq!(stats.tags_merged, 1); // t2 from local appended
        assert_eq!(local.tags.len(), 4);
        assert_eq!(local.tags[0].id, t3.id);
        assert_eq!(local.tags[1].id, t1.id);
        assert_eq!(local.tags[2].id, t4.id);
        assert_eq!(local.tags[3].id, t2.id);

        // 2. Local is newer: Local order must win, with remote-only tags appended
        let mut local_newer = VaultData {
            metadata: VaultMetadata { id: Uuid::new_v4(), name: "Local".into(), created_at: older, updated_at: now, entry_count: 0, version: 3 },
            entries: vec![],
            tags: vec![t2.clone(), t1.clone(), t3.clone()],
            trash: vec![],
            settings: VaultSettings::default(),
        };

        let remote_older = VaultData {
            metadata: VaultMetadata { id: Uuid::new_v4(), name: "Remote".into(), created_at: older, updated_at: older, entry_count: 0, version: 3 },
            entries: vec![],
            tags: vec![t3.clone(), t4.clone()],
            trash: vec![],
            settings: VaultSettings::default(),
        };

        let stats2 = merge_vault_data(&mut local_newer, remote_older);
        assert_eq!(stats2.tags_merged, 1); // t4 from remote appended
        assert_eq!(local_newer.tags.len(), 4);
        assert_eq!(local_newer.tags[0].id, t2.id);
        assert_eq!(local_newer.tags[1].id, t1.id);
        assert_eq!(local_newer.tags[2].id, t3.id);
        assert_eq!(local_newer.tags[3].id, t4.id);
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
        let res = apply_and_save_remote_vault(&bytes_b, &subkeys_a, &vault_a_path, None);
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

    #[test]
    fn test_p2p_listener_timeout() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("timeout_test.vdb");
        let password = "TimeoutPassword#123";

        let mgr = crate::vault::manager::VaultManager::create("Timeout Vault", password, &db_path).unwrap();
        let subkeys = mgr.get_subkeys().unwrap().clone();

        let addr = "127.0.0.1:49161";
        // Listener with a short 250ms timeout must return cleanly without hanging
        let start = std::time::Instant::now();
        let res = run_p2p_sync_listener_with_timeout(addr, &subkeys, &db_path, std::time::Duration::from_millis(250));
        let elapsed = start.elapsed();

        assert!(res.is_err());
        match res.err().unwrap() {
            crate::error::VaultError::SyncError(msg) => {
                assert!(msg.contains("timed out waiting for peer connection"));
            }
            other => panic!("Expected timeout SyncError, got {:?}", other),
        }
        assert!(elapsed >= std::time::Duration::from_millis(200));
        assert!(elapsed < std::time::Duration::from_secs(3));
    }

    #[test]
    fn test_p2p_discovery_id_distinct() {
        let salt = [42u8; 32];
        let mk1 = crate::crypto::derive_master_key(b"PassphraseOne#123", &salt).unwrap();
        let keys1 = crate::crypto::derive_subkeys(&mk1).unwrap();

        let mk2 = crate::crypto::derive_master_key(b"PassphraseTwo#456", &salt).unwrap();
        let keys2 = crate::crypto::derive_subkeys(&mk2).unwrap();

        let id1 = compute_p2p_discovery_id(&keys1);
        let id2 = compute_p2p_discovery_id(&keys2);

        // Same password & salt produces deterministic discovery ID
        assert_eq!(id1, compute_p2p_discovery_id(&keys1));
        // Distinct passwords produce completely distinct discovery IDs
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_p2p_discovery_beacon_roundtrip() {
        let salt = [123u8; 32];
        let mk = crate::crypto::derive_master_key(b"BeaconTestPassword#1", &salt).unwrap();
        let keys = crate::crypto::derive_subkeys(&mk).unwrap();
        let id = compute_p2p_discovery_id(&keys);

        let id_clone = id;
        let bcast_thread = std::thread::spawn(move || {
            for _ in 0..3 {
                std::thread::sleep(std::time::Duration::from_millis(50));
                let _ = broadcast_discovery_beacon(&id_clone, 5322);
            }
        });

        let res = listen_discovery_beacon(&id, std::time::Duration::from_millis(500));
        let _ = bcast_thread.join();

        if let Ok(Some(addr)) = res {
            assert_eq!(addr.port(), 5322);
        }
    }

    #[test]
    fn test_p2p_sync_nil_uuid_revocation_rejected() {
        let temp_dir = tempdir().unwrap();
        let host_path = temp_dir.path().join("host_nil_bypass.vdb");
        let client_path = temp_dir.path().join("client_nil_bypass.vdb");
        let password = "NilBypassTestPassword#123";

        let mut host_mgr = crate::vault::manager::VaultManager::create("Host", password, &host_path).unwrap();
        // Register an authorized trusted device
        let legitimate_id = uuid::Uuid::new_v4();
        host_mgr.data.settings.trusted_devices.push(crate::vault::types::TrustedDevice {
            id: legitimate_id,
            name: "Legitimate Client".into(),
            device_type: "mobile".into(),
            os: "iOS".into(),
            paired_at: chrono::Utc::now(),
            last_sync_at: Some(chrono::Utc::now()),
            token_hash: String::new(),
        });
        host_mgr.save().unwrap();

        // Copy identical vault to client so salt and keys are identical
        fs::copy(&host_path, &client_path).unwrap();
        let client_mgr = crate::vault::manager::VaultManager::open(&client_path, password).unwrap();
        let c_keys = client_mgr.get_subkeys().unwrap().clone();
        let h_keys = host_mgr.get_subkeys().unwrap().clone();

        let addr = "127.0.0.1:49170";
        let h_path = host_path.clone();
        let host_handle = std::thread::spawn(move || {
            run_p2p_sync_listener(addr, &h_keys, &h_path)
        });

        std::thread::sleep(std::time::Duration::from_millis(100));

        // An attacker/revoked client attempts to bypass verification by presenting Uuid::nil()
        let res = run_p2p_sync_client_with_device(addr, &c_keys, &client_path, Some(uuid::Uuid::nil()));
        assert!(res.is_err(), "Client with Uuid::nil() must be rejected when trusted devices are configured");
        let err_msg = res.err().unwrap().to_string();
        assert!(err_msg.contains("disconnected by the host") || err_msg.contains("re-pair") || err_msg.contains("not in trusted devices list"));

        let srv_res = host_handle.join().unwrap();
        assert!(srv_res.is_err());
    }

    #[test]
    fn test_p2p_salt_commitment_deterministic_and_one_way() {
        let salt1 = [17u8; 32];
        let salt2 = [18u8; 32];
        let zero_salt = [0u8; 32];

        let commit1_a = compute_salt_commitment(&salt1);
        let commit1_b = compute_salt_commitment(&salt1);
        assert_eq!(commit1_a, commit1_b);

        let commit2 = compute_salt_commitment(&salt2);
        assert_ne!(commit1_a, commit2);

        // Commitment must not be equal to the raw salt itself
        assert_ne!(commit1_a, salt1);

        // Zero salt produces zero commitment
        assert_eq!(compute_salt_commitment(&zero_salt), [0u8; 32]);
    }

    #[test]
    fn test_p2p_transit_aead_roundtrip() {
        let salt = [42u8; 32];
        let master_key = crate::crypto::derive_master_key(b"TransitAeadPassword#1", &salt).unwrap();
        let subkeys = crate::crypto::derive_subkeys(&master_key).unwrap();

        let plaintext = b"SensitiveVaultDatabasePayloadBytesForTesting";
        let encrypted = crate::crypto::cipher::encrypt_vault_with_aad(plaintext, &subkeys.vault_key, P2P_TRANSIT_AAD).unwrap();

        // Valid decryption
        let decrypted = crate::crypto::cipher::decrypt_vault_with_aad(&encrypted, &subkeys.vault_key, P2P_TRANSIT_AAD).unwrap();
        assert_eq!(&decrypted[..], plaintext);

        // Invalid AAD fails
        let bad_aad = b"malicious-aad-tamper";
        let res_bad_aad = crate::crypto::cipher::decrypt_vault_with_aad(&encrypted, &subkeys.vault_key, bad_aad);
        assert!(res_bad_aad.is_err());

        // Wrong key fails
        let wrong_master = crate::crypto::derive_master_key(b"WrongKey#999", &salt).unwrap();
        let wrong_subkeys = crate::crypto::derive_subkeys(&wrong_master).unwrap();
        let res_wrong_key = crate::crypto::cipher::decrypt_vault_with_aad(&encrypted, &wrong_subkeys.vault_key, P2P_TRANSIT_AAD);
        assert!(res_wrong_key.is_err());
    }
}


