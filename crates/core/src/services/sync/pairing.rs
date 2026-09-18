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
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket, Ipv4Addr};
use std::path::Path;
use std::time::Duration;
use chrono::Utc;
use rand::Rng;
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
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
    merge_vault_data, DEFAULT_DISCOVERY_PORT, DEFAULT_P2P_PORT, DEFAULT_PAIRING_PORT,
    P2P_AUTH_FAILED_SIG, MAX_DB_SIZE, DISCOVERY_MULTICAST_ADDR, PAIRING_QUERY_MAGIC,
    get_local_lan_ips,
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
    #[serde(default)]
    pub peer_addr: Option<String>,
}

/// Encrypted container payload returned by Host to Client during device pairing.
#[derive(Serialize, Deserialize)]
pub struct PairingHostPayload {
    pub salt: [u8; 32],
    pub data: VaultData,
    #[serde(default)]
    pub error_msg: Option<String>,
}

/// Execution mode for device pairing client.
#[derive(Clone, Debug)]
pub enum ClientPairingMode {
    /// Adopts a remote vault into the given directory, generating a collision-free filename
    /// from the host vault's name. In this mode, no existing local vault is ever read or opened,
    /// and no existing file on disk is ever overwritten.
    AdoptIntoDir { target_dir: std::path::PathBuf },
    /// Adopts a remote vault directly into a specified file path.
    AdoptIntoFile { target_file: std::path::PathBuf },
    /// Synchronizes with an existing, authenticated local vault at the specified path.
    ExistingVault { path: std::path::PathBuf },
}

/// Sanitizes a vault name into a safe, valid filesystem filename without invalid characters.
pub fn sanitize_vault_filename(name: &str) -> String {
    let forbidden = ['\\', '/', ':', '*', '?', '"', '<', '>', '|', '\0', '\n', '\r', '\t'];
    let cleaned: String = name
        .chars()
        .map(|c| if forbidden.contains(&c) { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').trim();
    if trimmed.is_empty() {
        return "Adopted Vault".to_string();
    }

    let upper = trimmed.to_ascii_uppercase();
    let is_dos_device = matches!(
        upper.as_str(),
        "CON" | "PRN" | "AUX" | "NUL"
            | "COM1" | "COM2" | "COM3" | "COM4" | "COM5" | "COM6" | "COM7" | "COM8" | "COM9"
            | "LPT1" | "LPT2" | "LPT3" | "LPT4" | "LPT5" | "LPT6" | "LPT7" | "LPT8" | "LPT9"
    );
    if is_dos_device {
        format!("Adopted_{}", trimmed)
    } else {
        trimmed.to_string()
    }
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

/// Broadcasts an ephemeral UDP discovery beacon for the active pairing session using provided LAN adapter IPs.
pub fn broadcast_pairing_beacon_with_ips(pairing_id: &[u8; 32], tcp_port: u16, local_ips: &[std::net::IpAddr]) -> crate::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| VaultError::EncryptionError(format!("Failed to bind pairing UDP socket: {}", e)))?;
    let _ = socket.set_broadcast(true);

    let mut packet = [0u8; 38];
    packet[..4].copy_from_slice(&PAIRING_BEACON_MAGIC);
    packet[4..36].copy_from_slice(pairing_id);
    packet[36..38].copy_from_slice(&tcp_port.to_be_bytes());

    // 1. Global broadcast
    let dest = format!("255.255.255.255:{}", DEFAULT_DISCOVERY_PORT);
    let _ = socket.send_to(&packet, &dest);

    // 2. Multicast group (RFC 2365 local administrative scope)
    let multi_dest = format!("{}:{}", DISCOVERY_MULTICAST_ADDR, DEFAULT_DISCOVERY_PORT);
    let _ = socket.send_to(&packet, &multi_dest);

    // 3. Directed subnet broadcasts for all detected local IPv4 adapters
    for ip in local_ips {
        if let std::net::IpAddr::V4(ipv4) = ip {
            let octets = ipv4.octets();
            let directed_dest = format!("{}.{}.{}.255:{}", octets[0], octets[1], octets[2], DEFAULT_DISCOVERY_PORT);
            let _ = socket.send_to(&packet, &directed_dest);
        }
    }

    Ok(())
}

/// Broadcasts an ephemeral UDP discovery beacon for the active pairing session.
pub fn broadcast_pairing_beacon(pairing_id: &[u8; 32], tcp_port: u16) -> crate::Result<()> {
    broadcast_pairing_beacon_with_ips(pairing_id, tcp_port, &get_local_lan_ips())
}

/// Listens for a pairing discovery beacon matching the expected pairing token within a timeout.
/// Employs active UDP query-response: sends periodic query pulses to solicit immediate host unicast responses.
pub fn listen_pairing_beacon(
    expected_pairing_id: &[u8; 32],
    timeout: Duration,
) -> crate::Result<Option<SocketAddr>> {
    let listen_addr = format!("0.0.0.0:{}", DEFAULT_DISCOVERY_PORT);
    let socket = match UdpSocket::bind(&listen_addr) {
        Ok(s) => s,
        Err(_) => {
            // Fallback to ephemeral port if 5323 is busy (e.g. host on same machine)
            UdpSocket::bind("0.0.0.0:0")
                .map_err(|e| VaultError::EncryptionError(format!("Failed to bind UDP pairing listener: {}", e)))?
        }
    };

    let _ = socket.set_broadcast(true);
    if let Ok(multi_ip) = DISCOVERY_MULTICAST_ADDR.parse::<Ipv4Addr>() {
        let _ = socket.join_multicast_v4(&multi_ip, &Ipv4Addr::UNSPECIFIED);
    }

    socket.set_read_timeout(Some(Duration::from_millis(150)))
        .map_err(|e| VaultError::EncryptionError(format!("Failed to set UDP socket timeout: {}", e)))?;

    // Prepare active query packet: [YQRY (4B) | expected_pairing_id (32B)]
    let mut query_packet = [0u8; 36];
    query_packet[..4].copy_from_slice(&PAIRING_QUERY_MAGIC);
    query_packet[4..36].copy_from_slice(expected_pairing_id);

    let send_query = |sock: &UdpSocket| {
        let _ = sock.send_to(&query_packet, format!("255.255.255.255:{}", DEFAULT_DISCOVERY_PORT));
        let _ = sock.send_to(&query_packet, format!("{}:{}", DISCOVERY_MULTICAST_ADDR, DEFAULT_DISCOVERY_PORT));
        for ip in get_local_lan_ips() {
            if let std::net::IpAddr::V4(ipv4) = ip {
                let octets = ipv4.octets();
                let directed = format!("{}.{}.{}.255:{}", octets[0], octets[1], octets[2], DEFAULT_DISCOVERY_PORT);
                let _ = sock.send_to(&query_packet, &directed);
            }
        }
    };

    // Initial query pulse immediately
    send_query(&socket);

    let start = std::time::Instant::now();
    let mut last_query = std::time::Instant::now();
    let mut buf = [0u8; 64];

    while start.elapsed() < timeout {
        // Send query pulse every 350ms for snappy discovery
        if last_query.elapsed() >= Duration::from_millis(350) {
            send_query(&socket);
            last_query = std::time::Instant::now();
        }

        match socket.recv_from(&mut buf) {
            Ok((len, peer_addr)) => {
                if len >= 38 && buf[..4] == PAIRING_BEACON_MAGIC {
                    let received_id = &buf[4..36];
                    if received_id.ct_eq(expected_pairing_id).into() {
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
    run_p2p_pairing_host_with_device_and_cancel(
        listen_addr,
        master_password,
        pairing_code,
        host_db_path,
        accept_timeout,
        host_device_name,
        None,
    )
}

/// Runs the Pairing Host with custom device name identification and cancellation flag support.
pub fn run_p2p_pairing_host_with_device_and_cancel(
    listen_addr: &str,
    master_password: &str,
    pairing_code: &str,
    host_db_path: &Path,
    accept_timeout: Duration,
    host_device_name: Option<String>,
    cancel_flag: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
) -> crate::Result<(PairingStats, VaultData)> {
    let pairing_subkeys = derive_pairing_subkeys(master_password, pairing_code)?;
    let pairing_beacon_id = compute_pairing_beacon_id_from_subkeys(&pairing_subkeys);

    let mut listeners: Vec<TcpListener> = Vec::new();

    // Bind primary pairing port (5324)
    if let Ok(l) = TcpListener::bind(format!("0.0.0.0:{}", DEFAULT_PAIRING_PORT)) {
        let _ = l.set_nonblocking(true);
        listeners.push(l);
    }

    // Bind legacy pairing / P2P port (5322) so older clients or connections without port succeed
    if let Ok(l) = TcpListener::bind(format!("0.0.0.0:{}", DEFAULT_P2P_PORT)) {
        let _ = l.set_nonblocking(true);
        listeners.push(l);
    }

    // Also bind explicit listen_addr if specified and not already bound
    if !listen_addr.is_empty() && !listen_addr.ends_with(":5324") && !listen_addr.ends_with(":5322")
        && let Ok(l) = TcpListener::bind(listen_addr) {
            let _ = l.set_nonblocking(true);
            listeners.push(l);
        }

    // Fallback if none could be bound
    if listeners.is_empty() {
        let l = TcpListener::bind("0.0.0.0:5325")
            .or_else(|_| TcpListener::bind("0.0.0.0:0"))
            .map_err(|e| VaultError::EncryptionError(format!("Failed to bind pairing listener on {}: {}", listen_addr, e)))?;
        let _ = l.set_nonblocking(true);
        listeners.push(l);
    }

    let local_port = listeners.first().and_then(|l| l.local_addr().ok().map(|a| a.port())).unwrap_or(DEFAULT_PAIRING_PORT);
    let start_time = std::time::Instant::now();
    let mut last_beacon = std::time::Instant::now() - Duration::from_secs(10);
    let mut local_ips = get_local_lan_ips();
    let mut last_ips_refresh = std::time::Instant::now();

    // Bind non-blocking UDP socket for active query-response pairing
    let udp_socket = match UdpSocket::bind(format!("0.0.0.0:{}", DEFAULT_DISCOVERY_PORT)) {
        Ok(s) => {
            let _ = s.set_nonblocking(true);
            let _ = s.set_broadcast(true);
            if let Ok(multi_ip) = DISCOVERY_MULTICAST_ADDR.parse::<Ipv4Addr>() {
                let _ = s.join_multicast_v4(&multi_ip, &Ipv4Addr::UNSPECIFIED);
            }
            Some(s)
        }
        Err(_) => None,
    };

    let (mut stream, peer_sock_addr) = loop {
        if let Some(ref cancel) = cancel_flag
            && cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(VaultError::SyncError("Pairing host listening was cancelled by user".into()));
            }

        // Periodically refresh adapter IPs (at most every 10s) to avoid excessive socket probing
        if last_ips_refresh.elapsed() >= Duration::from_secs(10) {
            local_ips = get_local_lan_ips();
            last_ips_refresh = std::time::Instant::now();
        }

        // Fast warm-up beacon interval (500ms for first 10s, then 1500ms)
        let beacon_interval = if start_time.elapsed() < Duration::from_secs(10) {
            Duration::from_millis(500)
        } else {
            Duration::from_millis(1500)
        };

        if last_beacon.elapsed() >= beacon_interval {
            let _ = broadcast_pairing_beacon_with_ips(&pairing_beacon_id, local_port, &local_ips);
            last_beacon = std::time::Instant::now();
        }

        // Active query processing: handle incoming client UDP queries with immediate unicast responses
        if let Some(ref sock) = udp_socket {
            let mut qbuf = [0u8; 64];
            while let Ok((qlen, client_addr)) = sock.recv_from(&mut qbuf) {
                if qlen >= 36 && qbuf[..4] == PAIRING_QUERY_MAGIC
                    && qbuf[4..36].ct_eq(&pairing_beacon_id).into() {
                        let mut reply = [0u8; 38];
                        reply[..4].copy_from_slice(&PAIRING_BEACON_MAGIC);
                        reply[4..36].copy_from_slice(&pairing_beacon_id);
                        reply[36..38].copy_from_slice(&local_port.to_be_bytes());
                        let _ = sock.send_to(&reply, client_addr);
                    }
            }
        }

        let mut accepted = None;
        for listener in &listeners {
            match listener.accept() {
                Ok(res) => {
                    accepted = Some(res);
                    break;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => {}
            }
        }

        if let Some(res) = accepted {
            break res;
        }

        if start_time.elapsed() >= accept_timeout {
            return Err(VaultError::SyncError("Pairing listener timed out waiting for device connection".into()));
        }
        std::thread::sleep(Duration::from_millis(40));
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
    host_manager.data.settings.trusted_devices.retain(|d| {
        d.id != trusted_client.id && !(d.name.eq_ignore_ascii_case(&trusted_client.name) && d.os == trusted_client.os)
    });
    host_manager.data.settings.trusted_devices.push(trusted_client);

    // Ensure host itself is registered as a trusted device if not already present
    host_manager.data.settings.trusted_devices.retain(|d| {
        d.id != host_info.id && !(d.name.eq_ignore_ascii_case(&host_info.name) && d.os == host_info.os)
    });
    host_manager.data.settings.trusted_devices.push(crate::vault::types::TrustedDevice {
        id: host_info.id,
        name: host_info.name,
        device_type: host_info.device_type,
        os: host_info.os,
        paired_at: Utc::now(),
        last_sync_at: Some(Utc::now()),
        token_hash: String::new(),
    });

    // Run deduplication pass to ensure absolute integrity
    let mut seen_ids = std::collections::HashSet::new();
    let mut seen_names = std::collections::HashSet::new();
    let mut deduped = Vec::new();
    for dev in host_manager.data.settings.trusted_devices.drain(..) {
        let name_key = (dev.name.to_lowercase(), dev.os.to_lowercase());
        if seen_ids.insert(dev.id) && seen_names.insert(name_key) {
            deduped.push(dev);
        }
    }
    host_manager.data.settings.trusted_devices = deduped;

    host_manager.data.metadata.updated_at = Utc::now();
    host_manager.data.metadata.entry_count = host_manager.data.entries.len();
    host_manager.save()?;

    // 5. Return merged database + host root salt to Client (transit-encrypted inside AEAD blob)
    let host_payload = PairingHostPayload {
        salt: host_manager.salt,
        data: host_manager.data.clone(),
        error_msg: None,
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
        peer_addr: Some(peer_sock_addr.ip().to_string()),
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
    let mode = if client_db_path.exists() && fs::metadata(client_db_path).map(|m| m.len() > 0).unwrap_or(false) {
        ClientPairingMode::ExistingVault { path: client_db_path.to_path_buf() }
    } else {
        ClientPairingMode::AdoptIntoFile { target_file: client_db_path.to_path_buf() }
    };
    run_p2p_pairing_client_with_device(server_addr, master_password, pairing_code, mode, None)
}

/// Runs the Pairing Client with custom device name identification.
pub fn run_p2p_pairing_client_with_device(
    server_addr: &str,
    master_password: &str,
    pairing_code: &str,
    client_mode: ClientPairingMode,
    client_device_name: Option<String>,
) -> crate::Result<(PairingStats, VaultData)> {
    let pairing_subkeys = derive_pairing_subkeys(master_password, pairing_code)?;

    let target_str = if !server_addr.contains(':') {
        format!("{}:{}", server_addr, DEFAULT_PAIRING_PORT)
    } else {
        server_addr.to_string()
    };

    let addrs: Vec<SocketAddr> = target_str.to_socket_addrs()
        .map_err(|e| VaultError::EncryptionError(format!("Invalid pairing host address '{}': {}", server_addr, e)))?
        .collect();

    if addrs.is_empty() {
        return Err(VaultError::EncryptionError(format!("Could not resolve pairing host address '{}'", server_addr)));
    }

    let mut stream = None;
    let mut last_io_err: Option<std::io::Error> = None;
    let connect_timeout = Duration::from_millis(350);

    // Active direct unicast UDP query probe: queries target IP directly to confirm reachability and discover listening port
    let mut confirmed_host_port: Option<u16> = None;
    if let Some(target_addr) = addrs.first() {
        let probe_dest = SocketAddr::new(target_addr.ip(), DEFAULT_DISCOVERY_PORT);
        if let Ok(probe_sock) = UdpSocket::bind("0.0.0.0:0") {
            let _ = probe_sock.set_read_timeout(Some(Duration::from_millis(150)));
            let mut q_packet = [0u8; 36];
            q_packet[..4].copy_from_slice(&PAIRING_QUERY_MAGIC);
            let beacon_id = compute_pairing_beacon_id_from_subkeys(&pairing_subkeys);
            q_packet[4..36].copy_from_slice(&beacon_id);
            let _ = probe_sock.send_to(&q_packet, probe_dest);

            let mut resp_buf = [0u8; 64];
            if let Ok((rlen, _)) = probe_sock.recv_from(&mut resp_buf)
                && rlen >= 38 && resp_buf[..4] == PAIRING_BEACON_MAGIC && resp_buf[4..36].ct_eq(&beacon_id).into() {
                    confirmed_host_port = Some(u16::from_be_bytes([resp_buf[36], resp_buf[37]]));
                }
        }
    }

    for attempt in 0..15 {
        for addr in &addrs {
            let mut candidate_ports = Vec::with_capacity(4);
            if let Some(port) = confirmed_host_port {
                candidate_ports.push(port);
            }
            if server_addr.contains(':') && !candidate_ports.contains(&addr.port()) {
                candidate_ports.push(addr.port());
            }
            for fallback in [DEFAULT_PAIRING_PORT, DEFAULT_P2P_PORT, 5325] {
                if !candidate_ports.contains(&fallback) {
                    candidate_ports.push(fallback);
                }
            }

            for p in candidate_ports {
                let mut candidate_addr = *addr;
                candidate_addr.set_port(p);
                match TcpStream::connect_timeout(&candidate_addr, connect_timeout) {
                    Ok(s) => {
                        stream = Some(s);
                        break;
                    }
                    Err(e) => {
                        last_io_err = Some(e);
                    }
                }
            }
            if stream.is_some() {
                break;
            }
        }
        if stream.is_some() {
            break;
        }
        if attempt < 14 {
            std::thread::sleep(Duration::from_millis(400));
        }
    }

    let mut stream = stream.ok_or_else(|| {
        let diagnostic = match last_io_err.as_ref().map(|e| e.kind()) {
            Some(std::io::ErrorKind::ConnectionRefused) => {
                "Anslutning nekad: Värddatorn lyssnar inte på denna port. Kontrollera att värden klickat på 'Börja lyssna & vänta' först."
            }
            Some(std::io::ErrorKind::TimedOut) => {
                "Tidsgräns överskreds: Värddatorn svarar inte. Detta beror oftast på att Windows Brandvägg på värddatorn blockerar inkommande anslutningar, eller att nätverket är inställt som Publikt istället för Privat."
            }
            _ => "Kunde inte ansluta till värddatorn. Kontrollera IP-adress och nätverk.",
        };
        VaultError::EncryptionError(format!("Failed to connect to pairing host at {}: {}", server_addr, diagnostic))
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
    // INVARIANT: When in Adopt mode (e.g. from logged-out VaultSelect screen),
    // we NEVER open or read any local vault file from disk! Transmit empty vault data.
    let (client_data, client_entries_count) = match &client_mode {
        ClientPairingMode::ExistingVault { path } => {
            if path.exists() && fs::metadata(path).map(|m| m.len() > 0).unwrap_or(false) {
                let mgr = VaultManager::open(path, master_password)?;
                let count = mgr.data.entries.len();
                (mgr.data, count)
            } else {
                return Err(VaultError::InvalidFormat(format!(
                    "Valvfilen '{}' existerar inte eller är tom.",
                    path.display()
                )));
            }
        }
        ClientPairingMode::AdoptIntoDir { .. } | ClientPairingMode::AdoptIntoFile { .. } => {
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
        }
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

    // 4. Receive Host's adopted root salt & merged database (transit-encrypted inside AEAD blob)
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

    if let Some(err_msg) = host_payload.error_msg {
        return Err(VaultError::InvalidFormat(err_msg));
    }

    let host_salt = host_payload.salt;
    let merged_data = host_payload.data;

    // 5. Determine destination path with strict collision isolation
    let (final_dest_path, should_backup) = match &client_mode {
        ClientPairingMode::ExistingVault { path } => (path.clone(), true),
        ClientPairingMode::AdoptIntoFile { target_file } => (target_file.clone(), false),
        ClientPairingMode::AdoptIntoDir { target_dir } => {
            let safe_name = sanitize_vault_filename(&merged_data.metadata.name);
            let mut dest = target_dir.join(format!("{}.vdb", safe_name));
            if dest.exists() {
                let mut counter = 1;
                while dest.exists() {
                    dest = target_dir.join(format!("{} ({}).vdb", safe_name, counter));
                    counter += 1;
                }
            }
            (dest, false)
        }
    };

    if let Some(parent) = final_dest_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if should_backup && final_dest_path.exists() {
        let backup_path = final_dest_path.with_extension("vdb.bak");
        let _ = fs::copy(&final_dest_path, &backup_path);
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
    let temp_path = final_dest_path.with_extension(format!("vdb.pairing.{}.tmp", uuid::Uuid::new_v4().simple()));
    fs::write(&temp_path, &file_bytes)?;
    fs::rename(&temp_path, &final_dest_path)?;

    let stats = PairingStats {
        entries_sent: client_entries_count,
        entries_received: merged_data.entries.len(),
        entries_merged: if client_entries_count == 0 { 0 } else { merged_data.entries.len() },
        total_entries: merged_data.entries.len(),
        vault_path: Some(final_dest_path.to_string_lossy().to_string()),
        peer_addr: Some(server_addr.to_string()),
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
            ClientPairingMode::ExistingVault { path: client_path.clone() },
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

    #[test]
    fn test_pairing_adopt_mode_does_not_touch_existing_vault() {
        let temp_dir = tempdir().unwrap();
        let host_path = temp_dir.path().join("host_adopt.vdb");
        let client_existing_vault = temp_dir.path().join("yntra-vault.vdb");
        let password = "AdoptPassword#123";

        // Create a host vault with 2 entries
        let mut host_mgr = VaultManager::create("Host Vault", password, &host_path).unwrap();
        host_mgr.add_entry(crate::vault::manager::NewEntry {
            title: "Host Item 1".into(),
            username: "huser1".into(),
            password: "hpw".into(),
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
            username: "huser2".into(),
            password: "hpw".into(),
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

        // Create an existing unauthenticated client vault on disk with 5 private entries
        let mut client_local_mgr = VaultManager::create("My Local Vault", password, &client_existing_vault).unwrap();
        for i in 1..=5 {
            client_local_mgr.add_entry(crate::vault::manager::NewEntry {
                title: format!("Private Item {}", i),
                username: "localuser".into(),
                password: "secretpassword".into(),
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
        }
        let original_client_len = fs::metadata(&client_existing_vault).unwrap().len();

        let addr = "127.0.0.1:49168";
        let pin = "123 456";

        let h_path = host_path.clone();
        let host_handle = std::thread::spawn(move || {
            run_p2p_pairing_host(addr, password, pin, &h_path, Duration::from_secs(5))
        });

        std::thread::sleep(Duration::from_millis(100));

        // Connect as client in AdoptIntoDir mode pointing to temp_dir
        let (c_stats, c_data) = run_p2p_pairing_client_with_device(
            addr,
            password,
            pin,
            ClientPairingMode::AdoptIntoDir { target_dir: temp_dir.path().to_path_buf() },
            Some("New Client Device".into()),
        ).unwrap();

        let (h_stats, h_data) = host_handle.join().unwrap().unwrap();

        // 1. Client transmitted 0 entries in Adopt mode
        assert_eq!(c_stats.entries_sent, 0);

        // 2. Host received 0 entries and its entries count remained exactly 2
        assert_eq!(h_stats.entries_received, 0);
        assert_eq!(h_data.entries.len(), 2);

        // 3. Client received 2 entries and adopted vault was saved to "Host Vault.vdb"
        assert_eq!(c_data.entries.len(), 2);
        let adopted_path = temp_dir.path().join("Host Vault.vdb");
        assert!(adopted_path.exists());
        assert_eq!(c_stats.vault_path, Some(adopted_path.to_string_lossy().to_string()));

        // 4. CRITICAL INVARIANT: The unauthenticated existing local vault "yntra-vault.vdb" was NEVER touched or altered!
        let current_client_len = fs::metadata(&client_existing_vault).unwrap().len();
        assert_eq!(original_client_len, current_client_len);
        let reopened_local = VaultManager::open(&client_existing_vault, password).unwrap();
        assert_eq!(reopened_local.data.entries.len(), 5);
        assert_eq!(reopened_local.data.metadata.name, "My Local Vault");
    }

    #[test]
    fn test_pairing_adopt_mode_collision_avoidance() {
        let temp_dir = tempdir().unwrap();
        let host_path = temp_dir.path().join("host_collision.vdb");
        let existing_colliding_path = temp_dir.path().join("Colliding.vdb");
        let password = "CollisionPassword#123";

        // Pre-create an unrelated file named "Colliding.vdb" in target directory
        let mut pre_existing = VaultManager::create("Colliding", password, &existing_colliding_path).unwrap();
        pre_existing.add_entry(crate::vault::manager::NewEntry {
            title: "Original Existing".into(),
            username: "u1".into(),
            password: "p1".into(),
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

        // Host vault is also named "Colliding"
        let host_mgr = VaultManager::create("Colliding", password, &host_path).unwrap();
        assert_ne!(host_mgr.data.metadata.id, pre_existing.data.metadata.id);

        let addr = "127.0.0.1:49169";
        let pin = "321 654";

        let h_path = host_path.clone();
        let host_handle = std::thread::spawn(move || {
            run_p2p_pairing_host(addr, password, pin, &h_path, Duration::from_secs(5))
        });

        std::thread::sleep(Duration::from_millis(100));

        let (c_stats, _) = run_p2p_pairing_client_with_device(
            addr,
            password,
            pin,
            ClientPairingMode::AdoptIntoDir { target_dir: temp_dir.path().to_path_buf() },
            None,
        ).unwrap();

        let _ = host_handle.join().unwrap().unwrap();

        // Adopted vault should have avoided collision by saving to "Colliding (1).vdb"
        let collision_path = temp_dir.path().join("Colliding (1).vdb");
        assert!(collision_path.exists());
        assert_eq!(c_stats.vault_path, Some(collision_path.to_string_lossy().to_string()));

        // Verify original "Colliding.vdb" remains untouched with its original 1 entry
        let reopened_orig = VaultManager::open(&existing_colliding_path, password).unwrap();
        assert_eq!(reopened_orig.data.entries.len(), 1);
        assert_eq!(reopened_orig.data.entries[0].title, "Original Existing");
    }

    #[test]
    fn test_pairing_active_query_response_roundtrip() {
        let password = "ActiveQueryTestPassword#999";
        let pin = "987 654";
        let subkeys = derive_pairing_subkeys(password, pin).unwrap();
        let beacon_id = compute_pairing_beacon_id_from_subkeys(&subkeys);

        // Host responder socket on 5323 (or random port for test)
        let responder_sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        let responder_port = responder_sock.local_addr().unwrap().port();
        let b_id_clone = beacon_id;

        let host_thread = std::thread::spawn(move || {
            let _ = responder_sock.set_read_timeout(Some(Duration::from_millis(1500)));
            let mut qbuf = [0u8; 64];
            if let Ok((qlen, client_addr)) = responder_sock.recv_from(&mut qbuf)
                && qlen >= 36 && qbuf[..4] == PAIRING_QUERY_MAGIC && qbuf[4..36] == b_id_clone
            {
                let mut reply = [0u8; 38];
                reply[..4].copy_from_slice(&PAIRING_BEACON_MAGIC);
                reply[4..36].copy_from_slice(&b_id_clone);
                reply[36..38].copy_from_slice(&5324u16.to_be_bytes());
                let _ = responder_sock.send_to(&reply, client_addr);
            }
        });

        // Client sends query directly to responder
        let client_sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        let mut query = [0u8; 36];
        query[..4].copy_from_slice(&PAIRING_QUERY_MAGIC);
        query[4..36].copy_from_slice(&beacon_id);
        client_sock.send_to(&query, format!("127.0.0.1:{}", responder_port)).unwrap();

        let mut resp = [0u8; 64];
        client_sock.set_read_timeout(Some(Duration::from_millis(1000))).unwrap();
        let (rlen, _) = client_sock.recv_from(&mut resp).unwrap();
        assert_eq!(rlen, 38);
        assert_eq!(&resp[..4], &PAIRING_BEACON_MAGIC);
        assert_eq!(&resp[4..36], &beacon_id);
        let discovered_port = u16::from_be_bytes([resp[36], resp[37]]);
        assert_eq!(discovered_port, 5324);

        host_thread.join().unwrap();
    }
}
