//! Vault Synchronization Protocols (WebDAV cloud sync and local network P2P sync).

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use rand::Rng;
use crate::crypto::{compute_hmac, verify_hmac};
use crate::crypto::kdf::HmacKey;
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
pub fn validate_webdav_url(url: &str) -> crate::Result<()> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err(crate::error::VaultError::InvalidFormat("WebDAV URL cannot be empty".into()));
    }

    if trimmed.starts_with("http://") {
        let after_scheme = &trimmed[7..];
        let host = after_scheme.split('/').next().unwrap_or("").split(':').next().unwrap_or("");
        if host != "localhost" && host != "127.0.0.1" && host != "[::1]" && host != "::1" {
            return Err(crate::error::VaultError::InvalidFormat(
                "WebDAV URL must use HTTPS for secure transport outside localhost".into()
            ));
        }
    } else if !trimmed.starts_with("https://") {
        return Err(crate::error::VaultError::InvalidFormat(
            "WebDAV URL must start with https:// (or http:// for localhost testing)".into()
        ));
    }

    Ok(())
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

/// Decrypt raw remote vault bytes using active derived SubKeys.
pub fn decrypt_remote_vault_bytes(
    bytes: &[u8],
    subkeys: &crate::crypto::SubKeys,
) -> crate::Result<crate::vault::types::VaultData> {
    use zeroize::Zeroize;
    use crate::vault::format::VaultFile;
    use crate::crypto::cipher::{decrypt_vault, decrypt_vault_with_aad, EncryptedBlob};
    use crate::crypto::verify_hmac;

    let vault_file = VaultFile::from_bytes(bytes)?;

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

// ─── Local Network P2P Sync ─────────────────────────────────────────────

/// Runs a secure TCP listener for vault synchronization.
/// Verifies peer credentials via a mutual challenge-response handshake signed with HMAC key.
pub fn run_p2p_sync_listener(
    listen_addr: &str,
    hmac_key: &HmacKey,
    db_filepath: &Path,
) -> crate::Result<()> {
    let listener = TcpListener::bind(listen_addr)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("Failed to bind TCP listener: {}", e)))?;

    // Wait for a single peer connection
    if let Ok((mut stream, _)) = listener.accept() {
        // 1. Handshake Phase
        let mut server_challenge = [0u8; 32];
        rand::rng().fill(&mut server_challenge);

        // Send server challenge
        stream.write_all(&server_challenge)
            .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

        // Read client challenge
        let mut client_challenge = [0u8; 32];
        stream.read_exact(&mut client_challenge)
            .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

        // Compute signatures
        let sig_to_send = compute_hmac(&client_challenge, hmac_key);

        // Send server signature
        stream.write_all(&sig_to_send)
            .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

        // Read client signature
        let mut client_sig = [0u8; 64];
        stream.read_exact(&mut client_sig)
            .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

        // Verify client signature
        if verify_hmac(&server_challenge, &client_sig, hmac_key).is_err() {
            let _ = stream.write_all(b"UNAUTHORIZED");
            return Err(crate::error::VaultError::DecryptionError("Peer verification failed".into()));
        }

        // 2. Database Transfer Phase (Receive DB from client and validate)
        let mut size_buf = [0u8; 8];
        stream.read_exact(&mut size_buf)
            .map_err(|e| crate::error::VaultError::DecryptionError(format!("Failed to read database size: {}", e)))?;
        let db_size = u64::from_be_bytes(size_buf) as usize;

        if db_size > MAX_DB_SIZE {
            let _ = stream.write_all(b"SIZE_REJECTED");
            return Err(crate::error::VaultError::InvalidFormat(
                format!("Received DB size {} exceeds maximum {} bytes", db_size, MAX_DB_SIZE)
            ));
        }

        let mut db_data = vec![0u8; db_size];
        stream.read_exact(&mut db_data)
            .map_err(|e| crate::error::VaultError::DecryptionError(format!("Failed to read database data: {}", e)))?;

        // Validate received data is a valid .vdb file before writing
        VaultFile::from_bytes(&db_data).map_err(|e| {
            crate::error::VaultError::InvalidFormat(format!("Received invalid vault file: {}", e))
        })?;

        // Atomic write: temp file then rename
        let tmp_path = db_filepath.with_extension("vdb.sync.tmp");
        fs::write(&tmp_path, &db_data)
            .map_err(|e| crate::error::VaultError::SerializationError(format!("Failed to write temp sync file: {}", e)))?;
        fs::rename(&tmp_path, db_filepath)
            .map_err(|e| crate::error::VaultError::SerializationError(format!("Failed to rename sync file: {}", e)))?;
    }

    Ok(())
}

/// Connects as a client to a run_p2p_sync_listener peer.
/// Authenticates using the mutual challenge-response, then sends the local database.
pub fn run_p2p_sync_client(
    server_addr: &str,
    hmac_key: &HmacKey,
    db_filepath: &Path,
) -> crate::Result<()> {
    let mut stream = TcpStream::connect(server_addr)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("Failed to connect to sync server: {}", e)))?;

    // 1. Handshake Phase
    let mut server_challenge = [0u8; 32];
    stream.read_exact(&mut server_challenge)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

    let mut client_challenge = [0u8; 32];
    rand::rng().fill(&mut client_challenge);

    // Send client challenge
    stream.write_all(&client_challenge)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

    // Read server signature
    let mut server_sig = [0u8; 64];
    stream.read_exact(&mut server_sig)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

    // Verify server signature
    if verify_hmac(&client_challenge, &server_sig, hmac_key).is_err() {
        return Err(crate::error::VaultError::DecryptionError("Server verification failed".into()));
    }

    // Send client signature
    let client_sig = compute_hmac(&server_challenge, hmac_key);
    stream.write_all(&client_sig)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("P2P handshake failed: {}", e)))?;

    // 2. Database Transfer Phase (Send local DB to server)
    let file_data = fs::read(db_filepath)
        .map_err(|e| crate::error::VaultError::SerializationError(format!("Failed to read database: {}", e)))?;

    let db_size = file_data.len() as u64;
    stream.write_all(&db_size.to_be_bytes())
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("Failed to send database size: {}", e)))?;

    stream.write_all(&file_data)
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("Failed to send database data: {}", e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_p2p_handshake_and_sync() {
        use crate::vault::format::{VaultFile, FileHeader, KdfParams, FORMAT_VERSION};

        let temp_dir = tempdir().unwrap();
        let server_db_path = temp_dir.path().join("server.vdb");
        let client_db_path = temp_dir.path().join("client.vdb");

        // Build a valid .vdb file for the client to send
        let vault_file = VaultFile {
            header: FileHeader {
                version: FORMAT_VERSION,
                flags: 0,
                salt: [42u8; 32],
                kdf_params: KdfParams::default(),
            },
            hmac: None,
            biometric: None,
            hardware2fa: None,
            encrypted_payload: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        let client_data = vault_file.to_bytes().unwrap();
        fs::write(&client_db_path, &client_data).unwrap();

        // Write empty file for server DB
        File::create(&server_db_path).unwrap();

        let hmac_key = HmacKey { bytes: [42u8; 64] };
        let addr = "127.0.0.1:49153"; // High ephemeral port

        // Start listener on a background thread
        let srv_path = server_db_path.clone();
        let key_clone = HmacKey { bytes: hmac_key.bytes };
        let handle = std::thread::spawn(move || {
            run_p2p_sync_listener(addr, &key_clone, &srv_path)
        });

        // Small pause to allow thread listener to bind
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Connect as client
        run_p2p_sync_client(addr, &hmac_key, &client_db_path).unwrap();

        // Join thread and assert success
        handle.join().unwrap().unwrap();

        // Verify server received a valid .vdb file
        let server_data = fs::read(&server_db_path).unwrap();
        assert_eq!(server_data, client_data);
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

        assert!(validate_webdav_url("http://unencrypted.com/vault.vdb").is_err());
        assert!(validate_webdav_url("ftp://server.com/vault.vdb").is_err());
        assert!(validate_webdav_url("").is_err());
    }
}

