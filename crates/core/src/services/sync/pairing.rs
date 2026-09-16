//! Zero-Knowledge Device Pairing Protocol
//!
//! Allows instant, secure pairing between devices (e.g. Desktop & Mobile) via an ephemeral
//! 6-digit numeric PIN or pairing code, without manual file copying over USB.
//!
//! Cryptographic Pipeline:
//! 1. Pairing Secret: Derived from Master Password + Pairing PIN via BLAKE3 + Argon2id / HKDF
//! 2. Ephemeral UDP Discovery: Beacon on port 5323 keyed by BLAKE3(Master Password, Pairing Code)
//! 3. Mutual Authentication: Constant-time challenge-response HMAC handshake
//! 4. Cross-Vault CRDT Merge: Decrypts remote entries in transit, merges into local data,
//!    adopts host root salt, and writes re-encrypted database atomically.

use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::path::Path;
use std::time::Duration;
use chrono::Utc;
use rand::Rng;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use yntra_crypto::{
    derive_master_key, derive_subkeys, compute_hmac, verify_hmac,
    cipher::{encrypt_vault_with_aad, decrypt_vault_with_aad, EncryptedBlob},
    kdf::SubKeys,
};
use crate::error::VaultError;
use crate::vault::{
    types::{VaultData, VaultMetadata, VaultSettings},
    format::{VaultFile, FileHeader, FORMAT_VERSION, KdfParams},
    manager::VaultManager,
};
use crate::services::sync::{
    merge_vault_data, DEFAULT_DISCOVERY_PORT, DEFAULT_P2P_PORT,
    P2P_AUTH_FAILED_SIG, MAX_DB_SIZE,
};

pub const PAIRING_BEACON_MAGIC: [u8; 4] = *b"YPAR";
pub const PAIRING_AUTH_OK: [u8; 8] = *b"PAIR__OK";
pub const PAIRING_AAD_CLIENT: &[u8] = b"yntra-pairing-client-v1";
pub const PAIRING_AAD_HOST: &[u8] = b"yntra-pairing-host-v1";

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct PairingStats {
    pub entries_sent: usize,
    pub entries_received: usize,
    pub entries_merged: usize,
    pub total_entries: usize,
    #[serde(default)]
    pub vault_path: Option<String>,
}

/// Encrypted container payload returned by Host to Client during device pairing.
#[derive(Serialize, Deserialize)]
pub struct PairingHostPayload {
    pub salt: [u8; 32],
    pub data: VaultData,
}

/// Normalizes a user-entered pairing PIN or code by removing spaces, hyphens, and whitespace.
pub fn normalize_pairing_code(code: &str) -> String {
    code.chars().filter(|c| !c.is_whitespace() && *c != '-').collect()
}

/// Generates a random 6-digit numeric pairing PIN formatted as "NNN NNN".
pub fn generate_pairing_code() -> String {
    let mut rng = rand::rng();
    let p1: u32 = rng.random_range(100..1000);
    let p2: u32 = rng.random_range(100..1000);
    format!("{:03} {:03}", p1, p2)
}

/// Derives ephemeral SubKeys for the pairing transit tunnel from Master Password + Pairing Code.
pub fn derive_pairing_subkeys(master_password: &str, pairing_code: &str) -> crate::Result<SubKeys> {
    let clean = normalize_pairing_code(pairing_code);
    if clean.is_empty() {
        return Err(VaultError::InvalidPassword);
    }

    let salt_hash = blake3::hash(format!("yntra-pairing-salt-v1:{}", clean).as_bytes());
    let master_key = derive_master_key(master_password.as_bytes(), salt_hash.as_bytes())?;
    let subkeys = derive_subkeys(&master_key)?;
    Ok(subkeys)
}

/// Computes the zero-knowledge UDP discovery beacon token directly from active pairing subkeys.
/// Eliminates redundant Argon2id invocations when subkeys are already computed.
pub fn compute_pairing_beacon_id_from_subkeys(subkeys: &SubKeys) -> [u8; 32] {
    let key_hash = *blake3::hash(&subkeys.hmac_key.bytes).as_bytes();
    *blake3::keyed_hash(&key_hash, b"yntra-pairing-beacon-v2").as_bytes()
}

/// Computes the zero-knowledge UDP discovery beacon token for pairing discovery.
/// Cryptographically stretched with Argon2id via pairing subkeys to prevent offline GPU wordlist cracking on LAN.
pub fn compute_pairing_beacon_id(master_password: &str, pairing_code: &str) -> crate::Result<[u8; 32]> {
    let subkeys = derive_pairing_subkeys(master_password, pairing_code)?;
    Ok(compute_pairing_beacon_id_from_subkeys(&subkeys))
}

/// Broadcasts an ephemeral UDP discovery beacon for the active pairing session.
pub fn broadcast_pairing_beacon(pairing_id: &[u8; 32], tcp_port: u16) -> crate::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| VaultError::EncryptionError(format!("Failed to bind pairing UDP socket: {}", e)))?;
    let _ = socket.set_broadcast(true);

    let mut packet = [0u8; 38];
    packet[..4].copy_from_slice(&PAIRING_BEACON_MAGIC);
    packet[4..36].copy_from_slice(pairing_id);
    packet[36..38].copy_from_slice(&tcp_port.to_be_bytes());

    let dest = format!("255.255.255.255:{}", DEFAULT_DISCOVERY_PORT);
    let _ = socket.send_to(&packet, &dest);
    Ok(())
}

/// Listens for a pairing discovery beacon matching the expected pairing token within a timeout.
pub fn listen_pairing_beacon(
    expected_pairing_id: &[u8; 32],
    timeout: Duration,
) -> crate::Result<Option<SocketAddr>> {
    let listen_addr = format!("0.0.0.0:{}", DEFAULT_DISCOVERY_PORT);
    let socket = match UdpSocket::bind(&listen_addr) {
        Ok(s) => s,
        Err(e) => {
            return Err(VaultError::EncryptionError(format!(
                "Failed to bind UDP pairing discovery on {}: {}", listen_addr, e
            )));
        }
    };

    socket.set_read_timeout(Some(Duration::from_millis(200)))
        .map_err(|e| VaultError::EncryptionError(format!("Failed to set UDP socket timeout: {}", e)))?;

    let start = std::time::Instant::now();
    let mut buf = [0u8; 64];

    while start.elapsed() < timeout {
        match socket.recv_from(&mut buf) {
            Ok((len, peer_addr)) => {
                if len >= 38 && &buf[..4] == &PAIRING_BEACON_MAGIC {
                    let received_id = &buf[4..36];
                    if received_id == expected_pairing_id {
                        let peer_port = u16::from_be_bytes([buf[36], buf[37]]);
                        return Ok(Some(SocketAddr::new(peer_addr.ip(), peer_port)));
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                std::thread::yield_now();
            }
            Err(e) => {
                return Err(VaultError::EncryptionError(format!("UDP pairing discovery recv error: {}", e)));
            }
        }
    }

    Ok(None)
}

/// Runs the Pairing Host (e.g. Desktop).
/// Listens for incoming client connection, authenticates via pairing transit keys,
/// receives client's decrypted entries, merges with host entries, saves to host disk,
/// and returns the complete merged database + host root salt back to the client.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct DeviceInfo {
    pub id: uuid::Uuid,
    pub name: String,
    #[serde(default = "default_device_type_info")]
    pub device_type: String,
    #[serde(default = "default_os_info")]
    pub os: String,
}

fn default_device_type_info() -> String {
    "desktop".to_string()
}

fn default_os_info() -> String {
    "Windows".to_string()
}

pub fn resolve_local_device_info(custom_name: Option<&str>) -> DeviceInfo {
    let hostname = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "Yntra Device".to_string());

    #[cfg(target_os = "windows")]
    let (os, dtype) = ("Windows".to_string(), "desktop".to_string());
    #[cfg(target_os = "macos")]
    let (os, dtype) = ("macOS".to_string(), "desktop".to_string());
    #[cfg(target_os = "linux")]
    let (os, dtype) = ("Linux".to_string(), "desktop".to_string());
    #[cfg(target_os = "ios")]
    let (os, dtype) = ("iOS".to_string(), "mobile".to_string());
    #[cfg(target_os = "android")]
    let (os, dtype) = ("Android".to_string(), "mobile".to_string());
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux", target_os = "ios", target_os = "android")))]
    let (os, dtype) = ("Unknown".to_string(), "other".to_string());

    let name = match custom_name {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => hostname.clone(),
    };

    let user = std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_default();
    let seed = format!("{}:{}:{}:{}", std::env::consts::OS, hostname, user, name);
    let hash = blake3::hash(seed.as_bytes());
    let mut id_bytes = [0u8; 16];
    id_bytes.copy_from_slice(&hash.as_bytes()[..16]);
    id_bytes[6] = (id_bytes[6] & 0x0f) | 0x40;
    id_bytes[8] = (id_bytes[8] & 0x3f) | 0x80;
    let id = uuid::Uuid::from_bytes(id_bytes);

    DeviceInfo {
        id,
        name,
        device_type: dtype,
        os,
    }
}

/// Runs the Pairing Host (e.g. Desktop).
/// Listens for incoming client connection, authenticates via pairing transit keys,
/// receives client's decrypted entries, merges with host entries, saves to host disk,
/// and returns the complete merged database + host root salt back to the client.
pub fn run_p2p_pairing_host(
    listen_addr: &str,
    master_password: &str,
    pairing_code: &str,
    host_db_path: &Path,
    accept_timeout: Duration,
) -> crate::Result<(PairingStats, VaultData)> {
    run_p2p_pairing_host_with_device(listen_addr, master_password, pairing_code, host_db_path, accept_timeout, None)
}

/// Runs the Pairing Host with custom device name identification.
pub fn run_p2p_pairing_host_with_device(
    listen_addr: &str,
    master_password: &str,
    pairing_code: &str,
    host_db_path: &Path,
    accept_timeout: Duration,
    host_device_name: Option<String>,
) -> crate::Result<(PairingStats, VaultData)> {
    let pairing_subkeys = derive_pairing_subkeys(master_password, pairing_code)?;
    let pairing_beacon_id = compute_pairing_beacon_id_from_subkeys(&pairing_subkeys);

    let listener = TcpListener::bind(listen_addr)
        .map_err(|e| VaultError::EncryptionError(format!("Failed to bind pairing listener on {}: {}", listen_addr, e)))?;
    listener.set_nonblocking(true)
        .map_err(|e| VaultError::EncryptionError(format!("Failed to set non-blocking TCP listener: {}", e)))?;

    let local_port = listener.local_addr().map(|a| a.port()).unwrap_or(DEFAULT_P2P_PORT);
    let start_time = std::time::Instant::now();
    let mut last_beacon = std::time::Instant::now() - Duration::from_secs(10);

    let (mut stream, _) = loop {
        if last_beacon.elapsed() >= Duration::from_millis(1500) {
            let _ = broadcast_pairing_beacon(&pairing_beacon_id, local_port);
            last_beacon = std::time::Instant::now();
        }

        match listener.accept() {
            Ok(res) => break res,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if start_time.elapsed() >= accept_timeout {
                    return Err(VaultError::SyncError("Pairing listener timed out waiting for device connection".into()));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(VaultError::EncryptionError(format!("Failed to accept pairing connection: {}", e))),
        }
    };

    stream.set_nonblocking(false)
        .map_err(|e| VaultError::EncryptionError(format!("Failed to restore blocking socket: {}", e)))?;
    let sock_timeout = Some(Duration::from_secs(30));
    let _ = stream.set_read_timeout(sock_timeout);
    let _ = stream.set_write_timeout(sock_timeout);

    // 1. Mutual Challenge-Response Handshake
    let mut client_challenge = [0u8; 32];
    stream.read_exact(&mut client_challenge)
        .map_err(|e| VaultError::EncryptionError(format!("Pairing handshake read client challenge failed: {}", e)))?;

    let mut host_challenge = [0u8; 32];
    rand::rng().fill(&mut host_challenge);
    stream.write_all(&host_challenge)
        .map_err(|e| VaultError::EncryptionError(format!("Pairing handshake write host challenge failed: {}", e)))?;
    stream.flush().map_err(|e| VaultError::EncryptionError(format!("Flush host challenge failed: {}", e)))?;

    let mut client_sig = [0u8; 64];
    stream.read_exact(&mut client_sig)
        .map_err(|e| VaultError::EncryptionError(format!("Pairing handshake read client signature failed: {}", e)))?;

    if verify_hmac(&host_challenge, &client_sig, &pairing_subkeys.hmac_key).is_err() {
        let _ = stream.write_all(&P2P_AUTH_FAILED_SIG);
        let _ = stream.flush();
        return Err(VaultError::DecryptionError("Client pairing verification failed: Invalid pairing code or password".into()));
    }

    let host_sig = compute_hmac(&client_challenge, &pairing_subkeys.hmac_key);
    stream.write_all(&host_sig)
        .map_err(|e| VaultError::EncryptionError(format!("Pairing handshake send host signature failed: {}", e)))?;
    stream.write_all(&PAIRING_AUTH_OK)
        .map_err(|e| VaultError::EncryptionError(format!("Pairing handshake send auth ack failed: {}", e)))?;
    stream.flush().map_err(|e| VaultError::EncryptionError(format!("Flush auth ack failed: {}", e)))?;

    // 2. Exchange Device Metadata for Trusted Device Registry
    let mut dev_len_buf = [0u8; 4];
    stream.read_exact(&mut dev_len_buf)
        .map_err(|e| VaultError::DecryptionError(format!("Read client device info length failed: {}", e)))?;
    let client_dev_len = u32::from_be_bytes(dev_len_buf) as usize;
    if client_dev_len > 16384 {
        return Err(VaultError::InvalidFormat("Client device info exceeds maximum allowed size".into()));
    }
    let mut client_dev_bytes = vec![0u8; client_dev_len];
    stream.read_exact(&mut client_dev_bytes)
        .map_err(|e| VaultError::DecryptionError(format!("Read client device info data failed: {}", e)))?;
    let client_info: DeviceInfo = rmp_serde::from_slice(&client_dev_bytes)
        .map_err(|e| VaultError::SerializationError(format!("Deserialize client device info: {}", e)))?;

    let host_info = resolve_local_device_info(host_device_name.as_deref());
    let host_dev_bytes = rmp_serde::to_vec(&host_info)
        .map_err(|e| VaultError::SerializationError(format!("Serialize host device info: {}", e)))?;
    let host_dev_len = host_dev_bytes.len() as u32;
    stream.write_all(&host_dev_len.to_be_bytes())
        .map_err(|e| VaultError::EncryptionError(format!("Send host device info length failed: {}", e)))?;
    stream.write_all(&host_dev_bytes)
        .map_err(|e| VaultError::EncryptionError(format!("Send host device info data failed: {}", e)))?;
    stream.flush().map_err(|e| VaultError::EncryptionError(format!("Flush host device info failed: {}", e)))?;

    // 3. Receive Client's encrypted data payload
    let mut size_buf = [0u8; 8];
    stream.read_exact(&mut size_buf)
        .map_err(|e| VaultError::DecryptionError(format!("Failed to read client pairing payload size: {}", e)))?;
    let client_payload_size = u64::from_be_bytes(size_buf) as usize;

    if client_payload_size > MAX_DB_SIZE {
        return Err(VaultError::InvalidFormat("Client payload exceeds maximum size".into()));
    }

    let mut client_encrypted_bytes = vec![0u8; client_payload_size];
    stream.read_exact(&mut client_encrypted_bytes)
        .map_err(|e| VaultError::DecryptionError(format!("Failed to read client encrypted payload: {}", e)))?;

    let client_blob: EncryptedBlob = rmp_serde::from_slice(&client_encrypted_bytes)
        .map_err(|e| VaultError::SerializationError(format!("Deserialize client encrypted blob: {}", e)))?;
    let client_decrypted = Zeroizing::new(
        decrypt_vault_with_aad(&client_blob, &pairing_subkeys.vault_key, PAIRING_AAD_CLIENT)?
    );
    let client_data: VaultData = rmp_serde::from_slice(&client_decrypted)
        .map_err(|e| VaultError::SerializationError(format!("Deserialize client vault data: {}", e)))?;

    let client_entries_count = client_data.entries.len();

    // 4. Open Host's local vault and execute merge
    let mut host_manager = VaultManager::open(host_db_path, master_password)?;
    let host_entries_before = host_manager.data.entries.len();

    let merge_stats = merge_vault_data(&mut host_manager.data, client_data);

    // Register paired client as a Trusted Device in vault settings
    let trusted_client = crate::vault::types::TrustedDevice {
        id: client_info.id,
        name: client_info.name,
        device_type: client_info.device_type,
        os: client_info.os,
        paired_at: Utc::now(),
        last_sync_at: Some(Utc::now()),
        token_hash: String::new(),
    };
    host_manager.data.settings.trusted_devices.retain(|d| d.id != trusted_client.id);
    host_manager.data.settings.trusted_devices.push(trusted_client);

    // Ensure host itself is registered as a trusted device if not already present
    if !host_manager.data.settings.trusted_devices.iter().any(|d| d.id == host_info.id) {
        host_manager.data.settings.trusted_devices.push(crate::vault::types::TrustedDevice {
            id: host_info.id,
            name: host_info.name,
            device_type: host_info.device_type,
            os: host_info.os,
            paired_at: Utc::now(),
            last_sync_at: Some(Utc::now()),
            token_hash: String::new(),
        });
    }

    host_manager.data.metadata.updated_at = Utc::now();
    host_manager.data.metadata.entry_count = host_manager.data.entries.len();
    host_manager.save()?;

    // 5. Return merged database + host root salt to Client (transit-encrypted inside AEAD blob)
    let host_payload = PairingHostPayload {
        salt: host_manager.salt,
        data: host_manager.data.clone(),
    };
    let serialized_merged = Zeroizing::new(
        rmp_serde::to_vec(&host_payload)
            .map_err(|e| VaultError::SerializationError(format!("Serialize merged data: {}", e)))?
    );
    let encrypted_merged = encrypt_vault_with_aad(&serialized_merged, &pairing_subkeys.vault_key, PAIRING_AAD_HOST)?;
    let host_blob_bytes = rmp_serde::to_vec(&encrypted_merged)
        .map_err(|e| VaultError::SerializationError(format!("Serialize host encrypted blob: {}", e)))?;

    let blob_len = host_blob_bytes.len() as u64;
    stream.write_all(&blob_len.to_be_bytes())
        .map_err(|e| VaultError::EncryptionError(format!("Send host payload size failed: {}", e)))?;
    stream.write_all(&host_blob_bytes)
        .map_err(|e| VaultError::EncryptionError(format!("Send host payload data failed: {}", e)))?;
    stream.flush().map_err(|e| VaultError::EncryptionError(format!("Flush pairing data stream failed: {}", e)))?;

    let stats = PairingStats {
        entries_sent: host_entries_before,
        entries_received: client_entries_count,
        entries_merged: merge_stats.entries_added + merge_stats.entries_updated,
        total_entries: host_manager.data.entries.len(),
        vault_path: None,
    };

    Ok((stats, host_manager.data))
}

/// Runs the Pairing Client (e.g. Mobile).
/// Connects to host, authenticates via pairing transit keys, sends its local entries,
/// receives the complete merged database + host salt, and writes the adopted vault to disk.
pub fn run_p2p_pairing_client(
    server_addr: &str,
    master_password: &str,
    pairing_code: &str,
    client_db_path: &Path,
) -> crate::Result<(PairingStats, VaultData)> {
    run_p2p_pairing_client_with_device(server_addr, master_password, pairing_code, client_db_path, None)
}

/// Runs the Pairing Client with custom device name identification.
pub fn run_p2p_pairing_client_with_device(
    server_addr: &str,
    master_password: &str,
    pairing_code: &str,
    client_db_path: &Path,
    client_device_name: Option<String>,
) -> crate::Result<(PairingStats, VaultData)> {
    let pairing_subkeys = derive_pairing_subkeys(master_password, pairing_code)?;

    let addrs: Vec<SocketAddr> = server_addr.to_socket_addrs()
        .map_err(|e| VaultError::EncryptionError(format!("Invalid pairing host address '{}': {}", server_addr, e)))?
        .collect();

    if addrs.is_empty() {
        return Err(VaultError::EncryptionError(format!("Could not resolve pairing host address '{}'", server_addr)));
    }

    let mut stream = None;
    let connect_timeout = Duration::from_secs(2);

    for attempt in 0..5 {
        for addr in &addrs {
            if let Ok(s) = TcpStream::connect_timeout(addr, connect_timeout) {
                stream = Some(s);
                break;
            }
        }
        if stream.is_some() {
            break;
        }
        if attempt < 4 {
            std::thread::sleep(Duration::from_millis(150));
        }
    }

    let mut stream = stream.ok_or_else(|| {
        VaultError::EncryptionError(format!("Failed to connect to pairing host at {}", server_addr))
    })?;

    let sock_timeout = Some(Duration::from_secs(30));
    let _ = stream.set_read_timeout(sock_timeout);
    let _ = stream.set_write_timeout(sock_timeout);

    // 1. Mutual Challenge-Response Handshake
    let mut client_challenge = [0u8; 32];
    rand::rng().fill(&mut client_challenge);
    stream.write_all(&client_challenge)
        .map_err(|e| VaultError::EncryptionError(format!("Write client challenge failed: {}", e)))?;
    stream.flush().map_err(|e| VaultError::EncryptionError(format!("Flush client challenge failed: {}", e)))?;

    let mut host_challenge = [0u8; 32];
    stream.read_exact(&mut host_challenge)
        .map_err(|e| VaultError::EncryptionError(format!("Read host challenge failed: {}", e)))?;

    let client_sig = compute_hmac(&host_challenge, &pairing_subkeys.hmac_key);
    stream.write_all(&client_sig)
        .map_err(|e| VaultError::EncryptionError(format!("Send client signature failed: {}", e)))?;
    stream.flush().map_err(|e| VaultError::EncryptionError(format!("Flush client signature failed: {}", e)))?;

    let mut host_sig = [0u8; 64];
    stream.read_exact(&mut host_sig)
        .map_err(|e| VaultError::DecryptionError(format!("Read host signature failed: {}", e)))?;

    if host_sig == P2P_AUTH_FAILED_SIG {
        return Err(VaultError::DecryptionError("Host rejected pairing authentication: Invalid pairing code or password".into()));
    }

    if verify_hmac(&client_challenge, &host_sig, &pairing_subkeys.hmac_key).is_err() {
        let _ = stream.write_all(&P2P_AUTH_FAILED_SIG);
        let _ = stream.flush();
        return Err(VaultError::DecryptionError("Host verification failed: Invalid pairing code or password".into()));
    }

    let mut auth_ack = [0u8; 8];
    stream.read_exact(&mut auth_ack)
        .map_err(|e| VaultError::EncryptionError(format!("Read auth ack failed: {}", e)))?;
    if auth_ack != PAIRING_AUTH_OK {
        return Err(VaultError::EncryptionError("Invalid pairing handshake response from host".into()));
    }

    // 2. Exchange Device Metadata for Trusted Device Registry
    let client_info = resolve_local_device_info(client_device_name.as_deref());
    let client_dev_bytes = rmp_serde::to_vec(&client_info)
        .map_err(|e| VaultError::SerializationError(format!("Serialize client device info: {}", e)))?;
    let client_dev_len = client_dev_bytes.len() as u32;
    stream.write_all(&client_dev_len.to_be_bytes())
        .map_err(|e| VaultError::EncryptionError(format!("Send client device info length failed: {}", e)))?;
    stream.write_all(&client_dev_bytes)
        .map_err(|e| VaultError::EncryptionError(format!("Send client device info data failed: {}", e)))?;
    stream.flush().map_err(|e| VaultError::EncryptionError(format!("Flush client device info failed: {}", e)))?;

    let mut dev_len_buf = [0u8; 4];
    stream.read_exact(&mut dev_len_buf)
        .map_err(|e| VaultError::DecryptionError(format!("Read host device info length failed: {}", e)))?;
    let host_dev_len = u32::from_be_bytes(dev_len_buf) as usize;
    if host_dev_len > 16384 {
        return Err(VaultError::InvalidFormat("Host device info exceeds maximum allowed size".into()));
    }
    let mut host_dev_bytes = vec![0u8; host_dev_len];
    stream.read_exact(&mut host_dev_bytes)
        .map_err(|e| VaultError::DecryptionError(format!("Read host device info data failed: {}", e)))?;
    let _host_info: DeviceInfo = rmp_serde::from_slice(&host_dev_bytes)
        .map_err(|e| VaultError::SerializationError(format!("Deserialize host device info: {}", e)))?;

    // 3. Transmit Client's local data
    let (client_data, client_entries_count) = if client_db_path.exists() && fs::metadata(client_db_path).map(|m| m.len() > 0).unwrap_or(false) {
        let mgr = VaultManager::open(client_db_path, master_password)?;
        let count = mgr.data.entries.len();
        (mgr.data, count)
    } else {
        let empty = VaultData {
            metadata: VaultMetadata {
                id: uuid::Uuid::new_v4(),
                name: "Adopted Vault".into(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                entry_count: 0,
                version: FORMAT_VERSION,
            },
            entries: Vec::new(),
            tags: Vec::new(),
            trash: Vec::new(),
            settings: VaultSettings::default(),
        };
        (empty, 0)
    };

    let serialized_client = Zeroizing::new(
        rmp_serde::to_vec(&client_data)
            .map_err(|e| VaultError::SerializationError(format!("Serialize client data: {}", e)))?
    );
    let encrypted_client = encrypt_vault_with_aad(&serialized_client, &pairing_subkeys.vault_key, PAIRING_AAD_CLIENT)?;
    let client_blob_bytes = rmp_serde::to_vec(&encrypted_client)
        .map_err(|e| VaultError::SerializationError(format!("Serialize client encrypted blob: {}", e)))?;

    let payload_len = client_blob_bytes.len() as u64;
    stream.write_all(&payload_len.to_be_bytes())
        .map_err(|e| VaultError::EncryptionError(format!("Write client payload size failed: {}", e)))?;
    stream.write_all(&client_blob_bytes)
        .map_err(|e| VaultError::EncryptionError(format!("Write client payload data failed: {}", e)))?;
    stream.flush().map_err(|e| VaultError::EncryptionError(format!("Flush client payload failed: {}", e)))?;

    // 3. Receive Host's adopted root salt & merged database (transit-encrypted inside AEAD blob)
    let mut size_buf = [0u8; 8];
    stream.read_exact(&mut size_buf)
        .map_err(|e| VaultError::DecryptionError(format!("Read host merged payload size failed: {}", e)))?;
    let host_payload_size = u64::from_be_bytes(size_buf) as usize;

    if host_payload_size > MAX_DB_SIZE {
        return Err(VaultError::InvalidFormat("Host merged payload exceeds maximum size".into()));
    }

    let mut host_encrypted_bytes = vec![0u8; host_payload_size];
    stream.read_exact(&mut host_encrypted_bytes)
        .map_err(|e| VaultError::DecryptionError(format!("Read host merged payload data failed: {}", e)))?;

    let host_blob: EncryptedBlob = rmp_serde::from_slice(&host_encrypted_bytes)
        .map_err(|e| VaultError::SerializationError(format!("Deserialize host encrypted blob: {}", e)))?;
    let host_decrypted = Zeroizing::new(
        decrypt_vault_with_aad(&host_blob, &pairing_subkeys.vault_key, PAIRING_AAD_HOST)?
    );
    let host_payload: PairingHostPayload = rmp_serde::from_slice(&host_decrypted)
        .map_err(|e| VaultError::SerializationError(format!("Deserialize merged vault data: {}", e)))?;
    let host_salt = host_payload.salt;
    let merged_data = host_payload.data;

    // 4. Adopt Host's root salt & save newly adopted vault to client disk
    if let Some(parent) = client_db_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if client_db_path.exists() {
        let backup_path = client_db_path.with_extension("vdb.bak");
        let _ = fs::copy(client_db_path, &backup_path);
    }

    let master_key = derive_master_key(master_password.as_bytes(), &host_salt)?;
    let adopted_subkeys = derive_subkeys(&master_key)?;

    let serialized_to_save = Zeroizing::new(
        rmp_serde::to_vec(&merged_data)
            .map_err(|e| VaultError::SerializationError(format!("Serialize adopted vault: {}", e)))?
    );
    let header = FileHeader {
        version: FORMAT_VERSION,
        flags: 0,
        salt: host_salt,
        kdf_params: KdfParams::default(),
    };
    let aad = header.aad_bytes()?;
    let encrypted = encrypt_vault_with_aad(&serialized_to_save, &adopted_subkeys.vault_key, &aad)?;

    let mut payload = Vec::with_capacity(encrypted.nonce.len() + encrypted.ciphertext.len());
    payload.extend_from_slice(&encrypted.nonce);
    payload.extend_from_slice(&encrypted.ciphertext);

    let vault_file = VaultFile {
        header,
        hmac: None,
        biometric: None,
        hardware2fa: None,
        encrypted_payload: payload,
    };

    let file_bytes = vault_file.to_bytes()?;
    let temp_path = client_db_path.with_extension(format!("vdb.pairing.{}.tmp", uuid::Uuid::new_v4().simple()));
    fs::write(&temp_path, &file_bytes)?;
    fs::rename(&temp_path, client_db_path)?;

    let stats = PairingStats {
        entries_sent: client_entries_count,
        entries_received: merged_data.entries.len(),
        entries_merged: merged_data.entries.len(),
        total_entries: merged_data.entries.len(),
        vault_path: Some(client_db_path.to_string_lossy().to_string()),
    };

    Ok((stats, merged_data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use subtle::ConstantTimeEq;

    #[test]
    fn test_pairing_code_generation_and_normalization() {
        let code = generate_pairing_code();
        assert_eq!(code.len(), 7); // "NNN NNN"
        let norm = normalize_pairing_code(&code);
        assert_eq!(norm.len(), 6);
        assert!(norm.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_pairing_subkeys_deterministic() {
        let pw = "SecurePairingPassword#1";
        let code = "749 201";

        let keys1 = derive_pairing_subkeys(pw, code).unwrap();
        let keys2 = derive_pairing_subkeys(pw, "749201").unwrap();
        assert!(bool::from(keys1.ct_eq(&keys2)));

        let keys3 = derive_pairing_subkeys(pw, "749 202").unwrap();
        assert!(!bool::from(keys1.ct_eq(&keys3)));
    }

    #[test]
    fn test_pairing_beacon_id_hardened_and_deterministic() {
        let pw = "HardenedPairingPassword#2026";
        let pin = "123 456";

        let id1 = compute_pairing_beacon_id(pw, pin).unwrap();
        let id2 = compute_pairing_beacon_id(pw, "123456").unwrap();
        assert_eq!(id1, id2);

        let id3 = compute_pairing_beacon_id(pw, "123 457").unwrap();
        assert_ne!(id1, id3);

        let id4 = compute_pairing_beacon_id("DifferentPassword#1", pin).unwrap();
        assert_ne!(id1, id4);
    }

    #[test]
    fn test_cross_vault_pairing_and_merge_lifecycle() {
        let temp_dir = tempdir().unwrap();
        let host_path = temp_dir.path().join("host_pc.vdb");
        let client_path = temp_dir.path().join("client_mobile.vdb");
        let password = "CommonPassphrase#123";
        let pairing_pin = "381 924";

        // Create independent host vault with 2 items
        let mut host_mgr = VaultManager::create("Host Vault", password, &host_path).unwrap();
        host_mgr.add_entry(crate::vault::manager::NewEntry {
            title: "Host Item 1".into(),
            username: "user1".into(),
            password: "pw1".into(),
            url: "".into(),
            email: "".into(),
            notes: "".into(),
            tags: vec![],
            totp_secret: None,
            custom_fields: vec![],
            entry_type: None,
            generate_passkey: None,
            attachments: None,
        }).unwrap();
        host_mgr.add_entry(crate::vault::manager::NewEntry {
            title: "Host Item 2".into(),
            username: "user2".into(),
            password: "pw2".into(),
            url: "".into(),
            email: "".into(),
            notes: "".into(),
            tags: vec![],
            totp_secret: None,
            custom_fields: vec![],
            entry_type: None,
            generate_passkey: None,
            attachments: None,
        }).unwrap();
        let host_salt = host_mgr.salt;

        // Create independent client vault with 1 different item and DIFFERENT salt
        let mut client_mgr = VaultManager::create("Client Vault", password, &client_path).unwrap();
        client_mgr.add_entry(crate::vault::manager::NewEntry {
            title: "Client Item 1".into(),
            username: "user3".into(),
            password: "pw3".into(),
            url: "".into(),
            email: "".into(),
            notes: "".into(),
            tags: vec![],
            totp_secret: None,
            custom_fields: vec![],
            entry_type: None,
            generate_passkey: None,
            attachments: None,
        }).unwrap();
        assert_ne!(host_salt, client_mgr.salt);

        let addr = "127.0.0.1:49163";

        // Spawn host pairing listener in background thread
        let h_path = host_path.clone();
        let h_pw = password.to_string();
        let h_pin = pairing_pin.to_string();
        let host_handle = std::thread::spawn(move || {
            run_p2p_pairing_host(addr, &h_pw, &h_pin, &h_path, Duration::from_secs(5))
        });

        std::thread::sleep(Duration::from_millis(100));

        // Connect as client and pair
        let (c_stats, c_data) = run_p2p_pairing_client(addr, password, pairing_pin, &client_path).unwrap();
        let (h_stats, h_data) = host_handle.join().unwrap().unwrap();

        // Verify both sides now have all 3 entries!
        assert_eq!(h_stats.total_entries, 3);
        assert_eq!(c_stats.total_entries, 3);
        assert_eq!(h_data.entries.len(), 3);
        assert_eq!(c_data.entries.len(), 3);

        // Verify client adopted host's salt on disk
        let reopened_client = VaultManager::open(&client_path, password).unwrap();
        assert_eq!(reopened_client.salt, host_salt);
        assert_eq!(reopened_client.data.entries.len(), 3);

        let reopened_host = VaultManager::open(&host_path, password).unwrap();
        assert_eq!(reopened_host.salt, host_salt);
        assert_eq!(reopened_host.data.entries.len(), 3);

        // Verify both sides registered each other as trusted devices!
        assert!(!reopened_host.data.settings.trusted_devices.is_empty());
        assert!(!reopened_client.data.settings.trusted_devices.is_empty());
    }

    #[test]
    fn test_trusted_device_registration_and_revocation() {
        let temp_dir = tempdir().unwrap();
        let host_path = temp_dir.path().join("host_revocation.vdb");
        let client_path = temp_dir.path().join("client_revocation.vdb");
        let password = "TestPassword#Revoke123";

        let _ = VaultManager::create("Host", password, &host_path).unwrap();
        let _ = VaultManager::create("Client", password, &client_path).unwrap();

        let addr = "127.0.0.1:49165";
        let pin = "456 789";

        let h_path = host_path.clone();
        let host_handle = std::thread::spawn(move || {
            run_p2p_pairing_host_with_device(
                addr,
                password,
                pin,
                &h_path,
                Duration::from_secs(5),
                Some("Host PC".into()),
            )
        });

        std::thread::sleep(Duration::from_millis(100));

        let (_, _client_data) = run_p2p_pairing_client_with_device(
            addr,
            password,
            pin,
            &client_path,
            Some("Client Phone".into()),
        ).unwrap();
        let (_, host_data) = host_handle.join().unwrap().unwrap();

        // Verify trusted device was added
        let client_device = host_data.settings.trusted_devices.iter()
            .find(|d| d.name == "Client Phone")
            .expect("Client Phone must be in host trusted devices");
        let client_id = client_device.id;

        // Verify subsequent sync succeeds with device ID
        let sync_addr = "127.0.0.1:49166";
        let h_path_sync = host_path.clone();
        let host_sync_handle = std::thread::spawn(move || {
            let mgr = VaultManager::open(&h_path_sync, password).unwrap();
            let keys = mgr.get_subkeys().unwrap().clone();
            crate::services::sync::run_p2p_sync_listener(sync_addr, &keys, &h_path_sync)
        });

        std::thread::sleep(Duration::from_millis(100));

        let client_mgr = VaultManager::open(&client_path, password).unwrap();
        let c_keys = client_mgr.get_subkeys().unwrap().clone();
        let sync_res = crate::services::sync::run_p2p_sync_client_with_device(
            sync_addr,
            &c_keys,
            &client_path,
            Some(client_id),
        );
        assert!(sync_res.is_ok());
        let _ = host_sync_handle.join().unwrap();

        // Now revoke the client device on the host!
        let mut host_mgr = VaultManager::open(&host_path, password).unwrap();
        host_mgr.revoke_trusted_device(client_id).unwrap();

        // Attempting to sync again should fail because the client device was revoked!
        let sync_addr_revoked = "127.0.0.1:49167";
        let h_path_revoked = host_path.clone();
        let host_revoked_handle = std::thread::spawn(move || {
            let mgr = VaultManager::open(&h_path_revoked, password).unwrap();
            let keys = mgr.get_subkeys().unwrap().clone();
            crate::services::sync::run_p2p_sync_listener(sync_addr_revoked, &keys, &h_path_revoked)
        });

        std::thread::sleep(Duration::from_millis(100));

        let client_mgr2 = VaultManager::open(&client_path, password).unwrap();
        let c_keys2 = client_mgr2.get_subkeys().unwrap().clone();
        let fail_res = crate::services::sync::run_p2p_sync_client_with_device(
            sync_addr_revoked,
            &c_keys2,
            &client_path,
            Some(client_id),
        );
        assert!(fail_res.is_err());
        let err_msg = fail_res.err().unwrap().to_string();
        assert!(err_msg.contains("disconnected by the host") || err_msg.contains("re-pair"));
        let _ = host_revoked_handle.join();
    }

    #[test]
    fn test_pairing_wrong_pin_rejected() {
        let temp_dir = tempdir().unwrap();
        let host_path = temp_dir.path().join("host_wrong_pin.vdb");
        let client_path = temp_dir.path().join("client_wrong_pin.vdb");
        let password = "TestPassword#123";

        let _ = VaultManager::create("Host", password, &host_path).unwrap();
        let _ = VaultManager::create("Client", password, &client_path).unwrap();

        let addr = "127.0.0.1:49164";

        let h_path = host_path.clone();
        let host_handle = std::thread::spawn(move || {
            run_p2p_pairing_host(addr, password, "111 222", &h_path, Duration::from_secs(2))
        });

        std::thread::sleep(Duration::from_millis(50));

        // Client attempts to connect with wrong PIN
        let res = run_p2p_pairing_client(addr, password, "999 888", &client_path);
        assert!(res.is_err());
        let _ = host_handle.join();
    }
}
