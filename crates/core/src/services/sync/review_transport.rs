use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use uuid::Uuid;
use zeroize::Zeroizing;

use super::{identity, lifecycle::CancellableStream, review};
use crate::{
    Result, VaultError,
    crypto::SubKeys,
    vault::{VaultData, VaultManager},
};

const MAGIC: &[u8; 8] = b"YNREV001";
const MAX_FRAME: u64 = 256 * 1024 * 1024;

pub fn encode_vault(data: &VaultData, keys: &SubKeys, salt: [u8; 32]) -> Result<Vec<u8>> {
    let header = crate::vault::format::FileHeader {
        version: crate::vault::format::FORMAT_VERSION,
        flags: 0,
        salt,
        kdf_params: Default::default(),
    };
    let plain = crate::vault::storage::serialize_payload(data)?;
    let blob =
        crate::crypto::encrypt_vault_with_aad(&plain, &keys.vault_key, &header.aad_bytes()?)?;
    let mut encrypted_payload = blob.nonce;
    encrypted_payload.extend(blob.ciphertext);
    crate::vault::format::VaultFile {
        header,
        hmac: None,
        biometric: None,
        hardware2fa: None,
        encrypted_payload,
    }
    .to_bytes()
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub revision: String,
    pub data: VaultData,
    pub observed_at: chrono::DateTime<chrono::Utc>,
}

impl Snapshot {
    pub fn capture(manager: &VaultManager) -> Result<Self> {
        let disk = crate::vault::storage::read_bounded(&manager.path)?;
        if manager.disk_revision != Some(*blake3::hash(&disk).as_bytes()) {
            return Err(fail(
                "Vault changed in another session; reload or unlock again before synchronizing",
            ));
        }
        let revision = review::revision(&manager.data)?;
        let mut data = manager.data.clone();
        let sync = data.settings.sync.clone();
        let sync_policy = data.settings.sync_policy.clone();
        data.settings = Default::default();
        data.settings.sync = sync;
        data.settings.sync_policy = sync_policy;
        Ok(Self {
            revision,
            data,
            observed_at: chrono::Utc::now(),
        })
    }

    pub fn capture_with_trust(
        manager: &VaultManager,
        expected: &[crate::vault::TrustedDevice],
    ) -> Result<Self> {
        let current = &manager.data.settings.trusted_devices;
        if !current
            .iter()
            .map(|d| (d.id, &d.signing_public_key))
            .eq(expected.iter().map(|d| (d.id, &d.signing_public_key)))
        {
            return Err(fail(
                "Trusted devices changed; reconnect before synchronizing",
            ));
        }
        Self::capture(manager)
    }
}

pub fn apply(
    manager: &mut VaultManager,
    expected: &str,
    data: VaultData,
    operation: Uuid,
) -> Result<()> {
    apply_with_peer(manager, expected, data, operation, None)
}

pub fn apply_with_peer(
    manager: &mut VaultManager,
    expected: &str,
    mut data: VaultData,
    operation: Uuid,
    peer: Option<Uuid>,
) -> Result<()> {
    if review::revision(&manager.data)? != expected || manager.data.metadata.id != data.metadata.id
    {
        return Err(fail("Vault changed during review; compare again"));
    }
    let previous = manager.data.clone();
    let sync = std::mem::take(&mut data.settings.sync);
    data.settings = manager.data.settings.clone();
    data.settings.sync = sync;
    for receipt in data.settings.sync_pending_receipts.values_mut() {
        if let Some(previous) = receipt.peer_device_id {
            let aliases: std::collections::BTreeSet<_> = data.settings.trusted_devices.iter()
                .filter(|device| device.id == previous)
                .map(identity::canonical_device_id).collect();
            if aliases.len() == 1 { receipt.peer_device_id = aliases.first().copied(); }
        }
    }
    if let Some(legacy) = data.settings.sync_pending.take() {
        data.settings
            .sync_pending_receipts
            .entry(legacy)
            .or_default();
    }
    if data
        .settings
        .sync_pending_receipts
        .get(&operation)
        .is_some_and(|receipt| receipt.peer_device_id != peer)
    {
        return Err(fail(
            "Synchronization operation belongs to a different device",
        ));
    }
    // This approved result includes the current local state. Its receipt replaces
    // older unconfirmed results for this peer, while other peers stay pending.
    if peer.is_some() {
        data.settings
            .sync_pending_receipts
            .retain(|_, receipt| receipt.peer_device_id != peer);
    }
    if data.settings.sync_pending_receipts.len() >= 128
        && !data.settings.sync_pending_receipts.contains_key(&operation)
    {
        return Err(fail(
            "Too many unconfirmed synchronizations; confirm pending peers before continuing",
        ));
    }
    data.settings.sync_pending_receipts.insert(
        operation,
        crate::vault::PendingSyncReceipt {
            peer_device_id: peer,
        },
    );
    manager.data = data;
    manager.rebuild_search_index();
    if let Err(error) = manager.save_reviewed() {
        manager.data = previous;
        manager.rebuild_search_index();
        return Err(error);
    }
    Ok(())
}

pub fn acknowledge(manager: &mut VaultManager, operation: Uuid) -> Result<()> {
    acknowledge_with_peer(manager, operation, None)
}

pub fn acknowledge_with_peer(
    manager: &mut VaultManager,
    operation: Uuid,
    peer: Option<Uuid>,
) -> Result<()> {
    let pending = manager
        .data
        .settings
        .sync_pending_receipts
        .get(&operation)
        .cloned();
    if pending.is_none() && manager.data.settings.sync_pending != Some(operation) {
        return Ok(());
    }
    if pending
        .as_ref()
        .and_then(|receipt| receipt.peer_device_id)
        .is_some_and(|expected| peer != Some(expected))
    {
        return Err(fail("Storage confirmation came from a different device"));
    }
    let previous = manager.data.settings.clone();
    if let Some(peer) = peer {
        let now = chrono::Utc::now();
        let local = super::resolve_local_device_info(None).id;
        for device in &mut manager.data.settings.trusted_devices {
            if device.id == peer || device.id == local || identity::canonical_device_id(device) == peer || identity::canonical_device_id(device) == local {
                device.last_sync_at = Some(now);
            }
        }
    }
    if manager.data.settings.sync_pending == Some(operation) {
        manager.data.settings.sync_pending = None;
    }
    manager
        .data
        .settings
        .sync_pending_receipts
        .remove(&operation);
    if let Err(error) = manager.save_reviewed() {
        manager.data.settings = previous;
        return Err(error);
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct Proposal {
    operation: Uuid,
    expected_revision: String,
    data: VaultData,
}

#[derive(Serialize, Deserialize)]
struct Receipt {
    operation: Uuid,
    digest: String,
    phase: String,
}

fn fail(message: &str) -> VaultError {
    VaultError::SyncError(message.into())
}

fn write_frame(
    stream: &mut CancellableStream,
    session: &identity::Session,
    from_client: bool,
    phase: &str,
    value: &impl Serialize,
) -> Result<()> {
    let plain = Zeroizing::new(
        rmp_serde::to_vec(value).map_err(|_| fail("Cannot encode synchronization message"))?,
    );
    if plain.len() as u64 > MAX_FRAME - 1024 {
        return Err(fail("Synchronization payload exceeds the supported size"));
    }
    let aad = identity::transcript(
        phase.as_bytes(),
        &[
            MAGIC,
            if from_client {
                &session.client_aad
            } else {
                &session.server_aad
            },
        ],
    );
    let blob = crate::crypto::encrypt_vault_with_aad(&plain, &session.key, &aad)?;
    let bytes =
        rmp_serde::to_vec(&blob).map_err(|_| fail("Cannot encode synchronization envelope"))?;
    stream.write_all(&(bytes.len() as u64).to_be_bytes())?;
    stream.write_all(&bytes)?;
    stream.flush()?;
    Ok(())
}

fn read_frame<T: DeserializeOwned>(
    stream: &mut CancellableStream,
    session: &identity::Session,
    from_client: bool,
    phase: &str,
) -> Result<T> {
    let mut size = [0; 8];
    stream.read_exact(&mut size)?;
    let size = u64::from_be_bytes(size);
    if size > MAX_FRAME {
        return Err(fail("Synchronization payload exceeds the supported size"));
    }
    let mut bytes = vec![0; size as usize];
    stream.read_exact(&mut bytes)?;
    let blob =
        rmp_serde::from_slice(&bytes).map_err(|_| fail("Invalid synchronization envelope"))?;
    let aad = identity::transcript(
        phase.as_bytes(),
        &[
            MAGIC,
            if from_client {
                &session.client_aad
            } else {
                &session.server_aad
            },
        ],
    );
    let plain = crate::crypto::decrypt_vault_with_aad(&blob, &session.key, &aad)?;
    rmp_serde::from_slice(&plain).map_err(|_| fail("Invalid synchronization message"))
}

fn check_receipt(receipt: Receipt, operation: Uuid, digest: &str, phase: &str) -> Result<()> {
    if receipt.operation != operation || receipt.digest != digest {
        return Err(fail(
            "Synchronization receipt does not match the approved result",
        ));
    }
    if receipt.phase == "changed" {
        return Err(fail("Other device changed during review; compare again"));
    }
    if receipt.phase == "save_failed" {
        return Err(fail(
            "Peer could not confirm saving the approved result. Synchronize again.",
        ));
    }
    if receipt.phase != phase {
        return Err(fail(
            "Synchronization receipt does not match the approved result",
        ));
    }
    Ok(())
}

pub fn listen(
    address: &str,
    keys: &SubKeys,
    salt: &[u8; 32],
    trusted: &[crate::vault::TrustedDevice],
    cancel: Arc<AtomicBool>,
    ready: impl FnOnce(u16),
    capture: impl FnOnce() -> Result<Snapshot>,
    commit: impl FnOnce(&str, VaultData, Uuid, Uuid) -> Result<()>,
) -> Result<super::MergeStats> {
    let generation = crate::services::network::generation_token()?;
    let listener = TcpListener::bind(address)
        .map_err(|e| fail(&format!("Failed to bind TCP listener: {e}")))?;
    listener.set_nonblocking(true)?;
    let port = listener.local_addr()?.port();
    let beacon = super::compute_p2p_discovery_id(keys);
    let responder = super::discovery::responder();
    ready(port);
    let start = Instant::now();
    let mut last_beacon = start - Duration::from_secs(2);
    let stream = loop {
        crate::services::network::ensure_generation(generation)?;
        if cancel.load(Ordering::Acquire) {
            return Err(fail("Synchronization cancelled"));
        }
        if start.elapsed() > Duration::from_secs(45) {
            return Err(fail("No peer connected"));
        }
        if let Some(socket) = &responder {
            super::discovery::respond(socket, &beacon, port);
        }
        if last_beacon.elapsed() >= Duration::from_millis(1500) {
            let _ = super::broadcast_discovery_beacon(&beacon, port);
            last_beacon = Instant::now();
        }
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(40))
            }
            Err(e) => return Err(e.into()),
        }
    };
    let mut stream = CancellableStream::new_with_generation(stream, Some(cancel), generation)?;
    stream.set_deadline(Instant::now() + Duration::from_secs(900));
    let commitment = super::compute_salt_commitment(salt);
    stream.write_all(MAGIC)?;
    stream.write_all(&commitment)?;
    stream.flush()?;
    let mut remote_commitment = [0; 32];
    stream.read_exact(&mut remote_commitment)?;
    if commitment != remote_commitment {
        return Err(fail("Remote vault originates from a different root salt"));
    }
    let session = identity::authenticate_listener(
        &mut stream,
        keys,
        trusted,
        super::resolve_local_device_info(None).id,
        &commitment,
        &remote_commitment,
    )?;
    stream.set_idle_timeout(Duration::from_secs(900));
    let local = capture()?;
    write_frame(&mut stream, &session, false, "snapshot", &local)?;
    let proposal: Proposal = read_frame(&mut stream, &session, true, "proposal")?;
    if proposal.expected_revision != local.revision
        || proposal.data.metadata.id != local.data.metadata.id
    {
        return Err(fail("Vault changed during synchronization"));
    }
    let digest = review::revision(&proposal.data)?;
    let mut stats = stats(&local.data, &proposal.data, keys)?;
    stats.peer_device_id = Some(session.peer_id);
    write_frame(
        &mut stream,
        &session,
        false,
        "prepared",
        &Receipt {
            operation: proposal.operation,
            digest: digest.clone(),
            phase: "prepared".into(),
        },
    )?;
    let receipt = read_frame(&mut stream, &session, true, "commit")?;
    check_receipt(receipt, proposal.operation, &digest, "commit")?;
    if let Err(error) = commit(
        &local.revision,
        proposal.data,
        proposal.operation,
        session.peer_id,
    ) {
        let changed = matches!(&error, VaultError::InvalidState(message) | VaultError::SyncError(message) if message.starts_with("Vault changed"));
        let _ = write_frame(
            &mut stream,
            &session,
            false,
            "stored",
            &Receipt {
                operation: proposal.operation,
                digest,
                phase: if changed { "changed" } else { "save_failed" }.into(),
            },
        );
        return Err(error);
    }
    write_frame(
        &mut stream,
        &session,
        false,
        "stored",
        &Receipt {
            operation: proposal.operation,
            digest: digest.clone(),
            phase: "stored".into(),
        },
    )
    .map_err(|_| fail("Saved on this device; peer confirmation is pending. Synchronize again."))?;
    let receipt = read_frame(&mut stream, &session, true, "stored").map_err(|_| {
        fail("Saved on this device; peer confirmation is pending. Synchronize again.")
    })?;
    check_receipt(receipt, proposal.operation, &digest, "stored")?;
    Ok(stats)
}

pub fn connect(
    address: &str,
    keys: &SubKeys,
    salt: &[u8; 32],
    trusted: &[crate::vault::TrustedDevice],
    cancel: Arc<AtomicBool>,
    expected_peer: Option<Uuid>,
    local: Snapshot,
    review: impl FnOnce(&Snapshot, &Snapshot) -> Result<(Uuid, VaultData)>,
    validate: impl Fn(&str) -> Result<()>,
    commit: impl FnOnce(&str, VaultData, Uuid, Uuid) -> Result<()>,
) -> Result<super::MergeStats> {
    let generation = crate::services::network::generation_token()?;
    let addresses: Vec<_> = address.to_socket_addrs()?.collect();
    let mut connection = None;
    for _ in 0..10 {
        crate::services::network::ensure_generation(generation)?;
        if cancel.load(Ordering::Acquire) {
            return Err(fail("Synchronization cancelled"));
        }
        for address in &addresses {
            if let Ok(stream) = TcpStream::connect_timeout(address, Duration::from_millis(400)) {
                connection = Some(stream);
                break;
            }
        }
        if connection.is_some() {
            break;
        }
    }
    let mut stream = CancellableStream::new_with_generation(
        connection.ok_or_else(|| fail("Failed to connect to sync server"))?,
        Some(cancel),
        generation,
    )?;
    stream.set_deadline(Instant::now() + Duration::from_secs(900));
    let mut magic = [0; 8];
    stream.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(fail("Update both devices to use reviewed synchronization"));
    }
    let mut remote_commitment = [0; 32];
    stream.read_exact(&mut remote_commitment)?;
    let commitment = super::compute_salt_commitment(salt);
    stream.write_all(&commitment)?;
    stream.flush()?;
    if commitment != remote_commitment {
        return Err(fail("Remote vault originates from a different root salt"));
    }
    let session = identity::authenticate_client(
        &mut stream,
        keys,
        trusted,
        super::resolve_local_device_info(None).id,
        &remote_commitment,
        &commitment,
    )?;
    if expected_peer.is_some_and(|id| id != session.peer_id) {
        return Err(fail(
            "The connected device does not match the selected device",
        ));
    }
    stream.set_idle_timeout(Duration::from_secs(900));
    let remote: Snapshot = read_frame(&mut stream, &session, false, "snapshot")?;
    let (operation, merged) = review(&local, &remote)?;
    validate(&local.revision)?;
    let digest = review::revision(&merged)?;
    let mut stats = stats(&local.data, &merged, keys)?;
    stats.peer_device_id = Some(session.peer_id);
    write_frame(
        &mut stream,
        &session,
        true,
        "proposal",
        &Proposal {
            operation,
            expected_revision: remote.revision,
            data: merged.clone(),
        },
    )?;
    let prepared = read_frame(&mut stream, &session, false, "prepared")?;
    check_receipt(prepared, operation, &digest, "prepared")?;
    validate(&local.revision)?;
    validate_revision_before_commit(&mut stream, &session, operation, &digest)?;
    let stored = read_frame(&mut stream, &session, false, "stored")
        .map_err(|_| fail("Peer storage confirmation is missing. Synchronize again."))?;
    check_receipt(stored, operation, &digest, "stored")?;
    commit(&local.revision, merged, operation, session.peer_id).map_err(|_| {
        fail(
            "Peer saved the approved result, but this device could not save it. Synchronize again.",
        )
    })?;
    write_frame(
        &mut stream,
        &session,
        true,
        "stored",
        &Receipt {
            operation,
            digest,
            phase: "stored".into(),
        },
    )
    .map_err(|_| fail("Saved on this device; peer confirmation is pending. Synchronize again."))?;
    Ok(stats)
}

fn validate_revision_before_commit(
    stream: &mut CancellableStream,
    session: &identity::Session,
    operation: Uuid,
    digest: &str,
) -> Result<()> {
    write_frame(
        stream,
        session,
        true,
        "commit",
        &Receipt {
            operation,
            digest: digest.into(),
            phase: "commit".into(),
        },
    )
}

pub fn stats(local: &VaultData, merged: &VaultData, keys: &SubKeys) -> Result<super::MergeStats> {
    let changes = review::preview(local, merged, keys)?;
    let mut stats = super::MergeStats::default();
    let mut updated = std::collections::BTreeSet::new();
    for change in changes.changes {
        if change.field == "tag" {
            stats.tags_merged += 1;
        } else if change.field == "state" {
            if merged.entries.iter().any(|e| e.id == change.entry_id) {
                stats.entries_added += 1;
            } else {
                stats.trash_merged += 1;
            }
        } else {
            updated.insert(change.entry_id);
        }
    }
    stats.entries_updated = updated.len();
    Ok(stats)
}
