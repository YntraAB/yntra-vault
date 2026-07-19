//! Vault Synchronization Protocols (WebDAV cloud sync and local network P2P sync).

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use rand::Rng;
use crate::crypto::{compute_hmac, verify_hmac};
use crate::crypto::kdf::HmacKey;

// ─── WebDAV Cloud Sync ──────────────────────────────────────────────────

/// Upload the local encrypted database file to a WebDAV server.
pub async fn webdav_upload(
    url: &str,
    username: &str,
    password: Option<&str>,
    db_filepath: &Path,
) -> crate::Result<()> {
    let client = reqwest::Client::builder()
        .user_agent("Yntra Vault-PasswordManager/1.0")
        .build()
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("HTTP client init: {}", e)))?;

    let file_data = fs::read(db_filepath)
        .map_err(|e| crate::error::VaultError::SerializationError(format!("Read DB: {}", e)))?;

    let mut req = client.put(url).body(file_data);
    if !username.is_empty() {
        req = req.basic_auth(username, password);
    }

    let response = req.send().await
        .map_err(|e| crate::error::VaultError::EncryptionError(format!("WebDAV PUT request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(crate::error::VaultError::EncryptionError(format!(
            "WebDAV server returned failure status: {}",
            response.status()
        )));
    }

    Ok(())
}

/// Download the encrypted database file from a WebDAV server.
pub async fn webdav_download(
    url: &str,
    username: &str,
    password: Option<&str>,
    dest_db_filepath: &Path,
) -> crate::Result<()> {
    let client = reqwest::Client::builder()
        .user_agent("Yntra Vault-PasswordManager/1.0")
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

    fs::write(dest_db_filepath, bytes)
        .map_err(|e| crate::error::VaultError::SerializationError(format!("Write downloaded DB: {}", e)))?;

    Ok(())
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

        // 2. Database Transfer Phase (Receive DB from client and merge/overwrite)
        let mut size_buf = [0u8; 8];
        stream.read_exact(&mut size_buf)
            .map_err(|e| crate::error::VaultError::DecryptionError(format!("Failed to read database size: {}", e)))?;
        let db_size = u64::from_be_bytes(size_buf) as usize;

        let mut db_data = vec![0u8; db_size];
        stream.read_exact(&mut db_data)
            .map_err(|e| crate::error::VaultError::DecryptionError(format!("Failed to read database data: {}", e)))?;

        // Save incoming DB to local filepath
        fs::write(db_filepath, &db_data)
            .map_err(|e| crate::error::VaultError::SerializationError(format!("Failed to save merged database: {}", e)))?;
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
        let temp_dir = tempdir().unwrap();
        let server_db_path = temp_dir.path().join("server.vdb");
        let client_db_path = temp_dir.path().join("client.vdb");

        // Write dummy data to client DB
        let client_data = b"yntra-vault-client-encrypted-database-content-12345";
        fs::write(&client_db_path, client_data).unwrap();

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

        // Check if server database has been updated with client's data
        let server_data = fs::read(&server_db_path).unwrap();
        assert_eq!(server_data, client_data);
    }
}
