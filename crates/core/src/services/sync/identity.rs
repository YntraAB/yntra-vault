//! Installation-local signing identity. Private material is never synchronized.
use std::{io::{Read, Write}, path::Path, sync::OnceLock};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey, Signature};
use zeroize::Zeroizing;
use crate::{Result, VaultError, crypto::mem::ProtectedSecret};

static IDENTITY: OnceLock<ProtectedSecret> = OnceLock::new();

pub fn initialize(path: &Path) -> Result<()> {
    if IDENTITY.get().is_some() { return Ok(()); }
    let read = || -> Result<Zeroizing<Vec<u8>>> {
        let metadata = std::fs::symlink_metadata(path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 16384 {
            return Err(VaultError::SyncError("Invalid local signing identity file".into()));
        }
        Ok(Zeroizing::new(crate::crypto::hardware_unwrap_key(&std::fs::read(path)?)?))
    };
    let secret = match read() {
        Ok(secret) => secret,
        Err(VaultError::IoError(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut key = Zeroizing::new([0u8; 32]);
            use rand::RngCore;
            rand::rng().fill_bytes(key.as_mut());
            let wrapped = crate::crypto::hardware_wrap_key(key.as_ref())?;
            let parent = path.parent().ok_or_else(|| VaultError::SyncError("Missing identity directory".into()))?;
            std::fs::create_dir_all(parent)?;
            let mut staged = tempfile::NamedTempFile::new_in(parent)?;
            staged.write_all(&wrapped)?;
            staged.as_file().sync_all()?;
            match staged.persist_noclobber(path) {
                Ok(_) => Zeroizing::new(key.to_vec()),
                Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => read()?,
                Err(error) => return Err(error.error.into()),
            }
        }
        Err(error) => return Err(error),
    };
    if secret.len() != 32 { return Err(VaultError::SyncError("Invalid local signing identity".into())); }
    let encoded = Zeroizing::new(data_encoding::HEXLOWER.encode(&secret));
    let _ = IDENTITY.set(ProtectedSecret::new(&encoded)?);
    Ok(())
}

fn with_key<T>(f: impl FnOnce(SigningKey) -> T) -> Result<T> {
    #[cfg(test)]
    IDENTITY.get_or_init(|| {
        let mut secret = Zeroizing::new([0u8; 32]);
        use rand::RngCore;
        rand::rng().fill_bytes(secret.as_mut());
        let encoded = Zeroizing::new(data_encoding::HEXLOWER.encode(secret.as_ref()));
        ProtectedSecret::new(&encoded).expect("synthetic protected signing key")
    });
    IDENTITY.get().ok_or_else(|| VaultError::SyncError("Initialize this device's signing identity and re-pair before syncing".into()))?
        .with_secret(|encoded| -> Result<T> {
            let bytes = Zeroizing::new(data_encoding::HEXLOWER.decode(encoded.as_bytes()).map_err(|_| VaultError::IntegrityError)?);
            let key: &[u8;32] = bytes.as_slice().try_into().map_err(|_| VaultError::IntegrityError)?;
            Ok(f(SigningKey::from_bytes(key)))
        })?
}

pub fn public_key() -> Result<Vec<u8>> { with_key(|key| key.verifying_key().to_bytes().to_vec()) }
pub fn sign(message: &[u8]) -> Result<[u8;64]> { with_key(|key| key.sign(message).to_bytes()) }

pub fn verify(public: &[u8], message: &[u8], signature: &[u8;64]) -> Result<()> {
    let key: &[u8;32] = public.try_into().map_err(|_| VaultError::SyncError("This device needs to be re-paired before syncing".into()))?;
    VerifyingKey::from_bytes(key).map_err(|_| VaultError::IntegrityError)?
        .verify_strict(message, &Signature::from_bytes(signature)).map_err(|_| VaultError::IntegrityError)
}

pub fn trusted_key(devices: &[crate::vault::types::TrustedDevice], id: uuid::Uuid) -> Result<&[u8]> {
    let mut matches = devices.iter().filter(|device| device.id == id && !id.is_nil());
    let device = matches.next().ok_or_else(|| VaultError::SyncError("Device revoked or not paired; re-pair to synchronize".into()))?;
    if matches.next().is_some() || device.signing_public_key.len() != 32 {
        return Err(VaultError::SyncError("Device identity is missing or ambiguous; re-pair to synchronize".into()));
    }
    Ok(&device.signing_public_key)
}

pub fn transcript(role: &[u8], parts: &[&[u8]]) -> Vec<u8> {
    let mut result = b"yntra-device-auth-v3\0".to_vec();
    result.extend_from_slice(&(role.len() as u64).to_be_bytes());
    result.extend_from_slice(role);
    for part in parts {
        result.extend_from_slice(&(part.len() as u64).to_be_bytes());
        result.extend_from_slice(part);
    }
    result
}

const SYNC_VERSION: &[u8;8] = b"YNSYN003";
pub struct Session {
    pub peer_id: uuid::Uuid,
    pub key: crate::crypto::VaultKey,
    pub client_aad: Vec<u8>,
    pub server_aad: Vec<u8>,
}

fn session(peer_id: uuid::Uuid, shared: p256::ecdh::SharedSecret, context: &[u8]) -> Result<Session> {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(context);
    let mut key = crate::crypto::VaultKey { bytes:[0;32] };
    hkdf::Hkdf::<sha2::Sha256>::new(Some(&digest),shared.raw_secret_bytes())
        .expand(b"yntra-p2p-session-v3",&mut key.bytes).map_err(|_| VaultError::IntegrityError)?;
    Ok(Session { peer_id, key,
        client_aad:transcript(b"client-data",&[context]),
        server_aad:transcript(b"server-data",&[context]),
    })
}

pub fn authenticate_listener(stream: &mut (impl Read + Write), keys: &crate::crypto::SubKeys,
    trusted: &[crate::vault::types::TrustedDevice], id: uuid::Uuid, server_commitment:&[u8;32], client_commitment:&[u8;32]) -> Result<Session> {
    use p256::elliptic_curve::{rand_core::OsRng, sec1::ToEncodedPoint};
    let ephemeral = p256::ecdh::EphemeralSecret::random(&mut OsRng);
    let public = ephemeral.public_key().to_encoded_point(false);
    let mut challenge=[0;32]; use rand::RngCore; rand::rng().fill_bytes(&mut challenge);
    stream.write_all(SYNC_VERSION)?;
    stream.write_all(&challenge)?;
    stream.write_all(id.as_bytes())?;
    stream.write_all(public.as_bytes())?;
    stream.flush()?;
    let mut version=[0;8]; stream.read_exact(&mut version)?;
    if &version != SYNC_VERSION { return Err(VaultError::SyncError("Update both devices and re-pair for secure sync".into())); }
    let mut client_challenge=[0;32]; stream.read_exact(&mut client_challenge)?;
    let mut client_id=[0;16]; stream.read_exact(&mut client_id)?;
    let mut client_public=[0;65]; stream.read_exact(&mut client_public)?;
    let mut mac=[0;64]; stream.read_exact(&mut mac)?;
    let mut signature=[0;64]; stream.read_exact(&mut signature)?;
    let peer_id=uuid::Uuid::from_bytes(client_id);
    let context=transcript(b"session",&[SYNC_VERSION,server_commitment,client_commitment,&challenge,&client_challenge,id.as_bytes(),&client_id,public.as_bytes(),&client_public]);
    let client_proof=transcript(b"client-proof",&[&context]);
    if crate::crypto::verify_hmac(&client_proof,&mac,&keys.hmac_key).is_err() {
        let _=stream.write_all(b"BADPASS!");
        return Err(VaultError::DecryptionError("Peer verification failed: Master password mismatch".into()));
    }
    if trusted_key(trusted,peer_id).and_then(|key| verify(key,&client_proof,&signature)).is_err() {
        let _=stream.write_all(b"REPAIR!!");
        return Err(VaultError::SyncError("Device revoked or not securely paired; re-pair to synchronize".into()));
    }
    let peer_public=p256::PublicKey::from_sec1_bytes(&client_public).map_err(|_| VaultError::IntegrityError)?;
    let server_proof=transcript(b"server-proof",&[&context]);
    let server_signature=sign(&server_proof)?;
    stream.write_all(b"AUTHV3OK")?;
    stream.write_all(&crate::crypto::compute_hmac(&server_proof,&keys.hmac_key))?;
    stream.write_all(&server_signature)?;
    stream.flush()?;
    session(peer_id,ephemeral.diffie_hellman(&peer_public),&context)
}

pub fn authenticate_client(stream: &mut (impl Read + Write), keys: &crate::crypto::SubKeys,
    trusted: &[crate::vault::types::TrustedDevice], id:uuid::Uuid, server_commitment:&[u8;32], client_commitment:&[u8;32]) -> Result<Session> {
    use p256::elliptic_curve::{rand_core::OsRng, sec1::ToEncodedPoint};
    let mut version=[0;8]; stream.read_exact(&mut version)?;
    if &version != SYNC_VERSION { return Err(VaultError::SyncError("Update both devices and re-pair for secure sync".into())); }
    let mut server_challenge=[0;32]; stream.read_exact(&mut server_challenge)?;
    let mut server_id=[0;16]; stream.read_exact(&mut server_id)?;
    let mut server_public=[0;65]; stream.read_exact(&mut server_public)?;
    let peer_id=uuid::Uuid::from_bytes(server_id);
    let peer_key=trusted_key(trusted,peer_id)?;
    let ephemeral=p256::ecdh::EphemeralSecret::random(&mut OsRng);
    let public=ephemeral.public_key().to_encoded_point(false);
    let mut challenge=[0;32]; use rand::RngCore; rand::rng().fill_bytes(&mut challenge);
    let context=transcript(b"session",&[SYNC_VERSION,server_commitment,client_commitment,&server_challenge,&challenge,&server_id,id.as_bytes(),&server_public,public.as_bytes()]);
    let client_proof=transcript(b"client-proof",&[&context]);
    stream.write_all(SYNC_VERSION)?;
    stream.write_all(&challenge)?;
    stream.write_all(id.as_bytes())?;
    stream.write_all(public.as_bytes())?;
    stream.write_all(&crate::crypto::compute_hmac(&client_proof,&keys.hmac_key))?;
    stream.write_all(&sign(&client_proof)?)?;
    stream.flush()?;
    let mut status=[0;8]; stream.read_exact(&mut status)?;
    if &status == b"BADPASS!" { return Err(VaultError::DecryptionError("Peer verification failed: Master password mismatch".into())); }
    if &status != b"AUTHV3OK" { return Err(VaultError::SyncError("Device revoked or not securely paired; re-pair to synchronize".into())); }
    let mut mac=[0;64]; stream.read_exact(&mut mac)?;
    let mut signature=[0;64]; stream.read_exact(&mut signature)?;
    let server_proof=transcript(b"server-proof",&[&context]);
    crate::crypto::verify_hmac(&server_proof,&mac,&keys.hmac_key)?;
    verify(peer_key,&server_proof,&signature)?;
    let peer_public=p256::PublicKey::from_sec1_bytes(&server_public).map_err(|_| VaultError::IntegrityError)?;
    session(peer_id,ephemeral.diffie_hellman(&peer_public),&context)
}

pub fn write_metadata(stream: &mut impl Write, bytes: &[u8], keys: &crate::crypto::SubKeys, role: &[u8], client: &[u8;32], host: &[u8;32]) -> Result<()> {
    stream.write_all(&(bytes.len() as u32).to_be_bytes())?;
    stream.write_all(bytes)?;
    stream.write_all(&crate::crypto::compute_hmac(&transcript(role,&[client,host,bytes]),&keys.hmac_key))?;
    stream.flush()?;
    Ok(())
}

pub fn read_metadata(stream: &mut impl Read, keys: &crate::crypto::SubKeys, role: &[u8], client: &[u8;32], host: &[u8;32]) -> Result<super::pairing::DeviceInfo> {
    let mut length = [0;4]; stream.read_exact(&mut length)?;
    let length = u32::from_be_bytes(length) as usize;
    if length > 16384 { return Err(VaultError::SyncError("Device metadata exceeds maximum size".into())); }
    let mut bytes = vec![0;length]; stream.read_exact(&mut bytes)?;
    let mut mac = [0;64]; stream.read_exact(&mut mac)?;
    crate::crypto::verify_hmac(&transcript(role,&[client,host,&bytes]),&mac,&keys.hmac_key)?;
    let info: super::pairing::DeviceInfo = rmp_serde::from_slice(&bytes).map_err(|_| VaultError::IntegrityError)?;
    if info.id.is_nil() || info.signing_public_key.len() != 32 { return Err(VaultError::SyncError("Update both devices and re-pair for authenticated synchronization".into())); }
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fresh_session_keys_require_ephemeral_private_keys_and_bound_context() {
        use p256::elliptic_curve::rand_core::OsRng;
        let alice=p256::ecdh::EphemeralSecret::random(&mut OsRng);
        let bob=p256::ecdh::EphemeralSecret::random(&mut OsRng);
        let observer=p256::ecdh::EphemeralSecret::random(&mut OsRng);
        let id=uuid::Uuid::new_v4();
        let a=session(id,alice.diffie_hellman(&bob.public_key()),b"session one").unwrap();
        let b=session(id,bob.diffie_hellman(&alice.public_key()),b"session one").unwrap();
        let observer=session(id,observer.diffie_hellman(&alice.public_key()),b"session one").unwrap();
        let altered=session(id,bob.diffie_hellman(&alice.public_key()),b"session two").unwrap();
        let encrypted=crate::crypto::encrypt_vault_with_aad(b"synthetic payload",&a.key,&a.client_aad).unwrap();
        assert!(crate::crypto::decrypt_vault_with_aad(&encrypted,&b.key,&b.client_aad).is_ok());
        assert!(crate::crypto::decrypt_vault_with_aad(&encrypted,&b.key,&b.server_aad).is_err());
        assert!(crate::crypto::decrypt_vault_with_aad(&encrypted,&observer.key,&observer.client_aad).is_err());
        assert!(crate::crypto::decrypt_vault_with_aad(&encrypted,&altered.key,&altered.client_aad).is_err());
    }

    #[test]
    fn pairing_metadata_cannot_be_replayed_changed_or_reflected() {
        let keys=crate::crypto::SubKeys::from_bytes(&[7;160]).unwrap();
        let info=super::super::pairing::DeviceInfo { id:uuid::Uuid::new_v4(),signing_public_key:vec![8;32],..Default::default() };
        let bytes=rmp_serde::to_vec(&info).unwrap();
        let mut wire=Vec::new();
        write_metadata(&mut wire,&bytes,&keys,b"client",&[1;32],&[2;32]).unwrap();
        assert!(read_metadata(&mut wire.as_slice(),&keys,b"client",&[1;32],&[2;32]).is_ok());
        assert!(read_metadata(&mut wire.as_slice(),&keys,b"server",&[1;32],&[2;32]).is_err());
        assert!(read_metadata(&mut wire.as_slice(),&keys,b"client",&[3;32],&[2;32]).is_err());
        wire[10]^=1;
        assert!(read_metadata(&mut wire.as_slice(),&keys,b"client",&[1;32],&[2;32]).is_err());
    }
    #[test]
    fn shared_vault_keys_cannot_impersonate_another_device_or_replay_transcripts() {
        let removed = SigningKey::from_bytes(&[1;32]);
        let host = SigningKey::from_bytes(&[2;32]);
        let id = uuid::Uuid::new_v4();
        let trusted = [crate::vault::types::TrustedDevice {id, signing_public_key:host.verifying_key().to_bytes().to_vec(),..Default::default()}];
        let message = transcript(b"client",&[&[3;32],&[4;32],id.as_bytes()]);
        let key = trusted_key(&trusted,id).unwrap();
        assert!(verify(key,&message,&removed.sign(&message).to_bytes()).is_err());
        let signature=host.sign(&message).to_bytes();
        assert!(verify(key,&message,&signature).is_ok());
        assert!(verify(key,&transcript(b"server",&[&[3;32],&[4;32],id.as_bytes()]),&signature).is_err());
        assert!(verify(key,&transcript(b"client",&[&[5;32],&[4;32],id.as_bytes()]),&signature).is_err());
        assert!(trusted_key(&[],id).is_err());
        assert!(trusted_key(&[crate::vault::types::TrustedDevice{id,..Default::default()}],id).is_err());
    }
}


