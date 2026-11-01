//! Yntra Vault — Local IPC Session Daemon
//!
//! Provides ultra-fast (<5ms) sub-millisecond local vault access by keeping unlocked
//! vault data in zeroized memory inside a local OS pipe / socket server with YNTRA_SESSION token auth.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;
use uuid::Uuid;
use colored::*;
use rand::RngCore;

use yntra_vault_core::{
    Result, VaultError,
    vault::{
        VaultManager, EntryPreview,
        types::{SecurityAudit, EntryType},
        manager::{NewEntry, UpdateEntry, DecryptedEntry},
    },
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SshIdentityPayload {
    pub key_blob: Vec<u8>,
    pub comment: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IpcEnvelope {
    pub session_token: String,
    pub request: IpcRequest,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum IpcRequest {
    ForVault { path: PathBuf, request: Box<IpcRequest> },
    Ping,
    ListEntries,
    GetEntry { query: String },
    SearchEntries { query: String },
    AddEntry { new_entry: NewEntry },
    UpdateEntry { id: Uuid, update: UpdateEntry },
    DeleteEntry { id: Uuid, permanent: bool },
    SecurityAudit,
    Lock,
    SshListIdentities,
    SshSign { pubkey_blob: Vec<u8>, data: Vec<u8>, flags: u32 },
}

static VAULT_SCOPE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

pub fn bind_vault_scope(path: &std::path::Path) -> Result<()> {
    let absolute = std::path::absolute(path)?;
    VAULT_SCOPE.set(absolute).map_err(|_| VaultError::InvalidState("CLI vault scope already selected".into()))
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum IpcResponse {
    Pong,
    ListEntries(Vec<EntryPreview>),
    GetEntry(DecryptedEntry),
    SearchEntries(Vec<EntryPreview>),
    AddEntrySuccess(Uuid),
    UpdateEntrySuccess,
    DeleteEntrySuccess,
    AuditSuccess(SecurityAudit),
    LockSuccess,
    SshIdentities(Vec<SshIdentityPayload>),
    SshSignSuccess(Vec<u8>),
    Error(String),
}

pub fn get_ipc_pipe_name() -> Result<String> {
    #[cfg(windows)]
    {
        let username = std::env::var("USERNAME").or_else(|_| std::env::var("USER"))
            .unwrap_or_else(|_| "default".into());
        Ok(format!(r"\\.\pipe\yntra-session-{}", username))
    }
    #[cfg(unix)]
    {
        let path = crate::socket_security::socket_path("yntra-session.sock")?;
        path.into_os_string().into_string().map_err(|_| VaultError::InvalidState("Socket path must be valid UTF-8".into()))
    }
}

pub fn get_session_token_from_env() -> String {
    if let Ok(env_token) = std::env::var("YNTRA_SESSION")
        && !env_token.is_empty() {
            return env_token;
        }
    crate::keychain::load_session_token().unwrap_or_default()
}

/// None means no connection was established and no request was sent.
pub async fn try_ipc_request(req: &IpcRequest) -> Option<IpcResponse> {
    let token = get_session_token_from_env();
    let envelope = IpcEnvelope {
        session_token: token,
        request: match VAULT_SCOPE.get() {
            Some(path) if !matches!(req, IpcRequest::Ping | IpcRequest::Lock) => IpcRequest::ForVault { path: path.clone(), request: Box::new(req.clone_req()) },
            _ => req.clone_req(),
        },
    };

    #[cfg(windows)]
    let mut stream = tokio::net::windows::named_pipe::ClientOptions::new()
        .open(get_ipc_pipe_name().ok()?).ok()?;
    #[cfg(not(windows))]
    let mut stream = tokio::net::UnixStream::connect(get_ipc_pipe_name().ok()?).await.ok()?;
    Some(exchange_request(&mut stream, &envelope, Duration::from_secs(5)).await)
}

async fn exchange_request<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>(
    stream: &mut S,
    envelope: &IpcEnvelope,
    deadline: Duration,
) -> IpcResponse {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let exchange = async {
        let payload = Zeroizing::new(serde_json::to_vec(envelope).map_err(|_| ())?);
        let length = u32::try_from(payload.len()).map_err(|_| ())?;
        stream.write_all(&length.to_le_bytes()).await.map_err(|_| ())?;
        stream.write_all(&payload).await.map_err(|_| ())?;
        stream.flush().await.map_err(|_| ())?;
        let mut length = [0; 4];
        stream.read_exact(&mut length).await.map_err(|_| ())?;
        let length = u32::from_le_bytes(length) as usize;
        if length > 16 * 1024 * 1024 { return Err(()); }
        let mut response = Zeroizing::new(vec![0; length]);
        stream.read_exact(&mut response).await.map_err(|_| ())?;
        serde_json::from_slice(&response).map_err(|_| ())
    };
    match tokio::time::timeout(deadline, exchange).await {
        Ok(Ok(response)) => response,
        _ => IpcResponse::Error("Session response was not confirmed. The operation may have completed; check the vault before retrying.".into()),
    }
}

fn validate_mutation_response(req: &IpcRequest, response: Option<IpcResponse>) -> Result<Option<IpcResponse>> {
    match response {
        None => Ok(None),
        Some(IpcResponse::Error(message)) => Err(VaultError::InvalidState(message)),
        Some(response) => {
            let expected = matches!((req, &response),
                (IpcRequest::AddEntry { .. }, IpcResponse::AddEntrySuccess(_)) |
                (IpcRequest::UpdateEntry { .. }, IpcResponse::UpdateEntrySuccess) |
                (IpcRequest::DeleteEntry { .. }, IpcResponse::DeleteEntrySuccess));
            if expected { Ok(Some(response)) }
            else { Err(VaultError::InvalidState("Unexpected session response; operation was not retried".into())) }
        }
    }
}

pub async fn try_ipc_mutation(req: &IpcRequest) -> Result<Option<IpcResponse>> {
    validate_mutation_response(req, try_ipc_request(req).await)
}

impl IpcRequest {
    fn clone_req(&self) -> Self {
        self.clone()
    }
}

/// Run local IPC session daemon holding unlocked VaultManager in memory
pub async fn run_ipc_daemon(vault_path: PathBuf, password: Zeroizing<String>, keyfile: Option<PathBuf>) -> Result<()> {
    let manager = VaultManager::open_with_keyfile(&vault_path, &password, keyfile.as_deref())?;
    let manager = Arc::new(Mutex::new(manager));
    let last_activity = Arc::new(Mutex::new(Instant::now()));

    // Generate random 256-bit session token
    let mut token_bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut token_bytes);
    let session_token = data_encoding::HEXLOWER.encode(&token_bytes);
    let _ = crate::keychain::store_session_token(&session_token);

    let pipe_name = get_ipc_pipe_name()?;

    println!("{}", "═════════════════════════════════════════════".blue());
    println!("  {}", "YNTRA VAULT SESSION UNLOCKED".bold().green());
    println!("{}", "═════════════════════════════════════════════".blue());
    println!("  {:18} {}", "Pipe Name:".bold(), pipe_name.cyan());
    println!("  {:18} {}", "Session Token:".bold(), session_token.bold().yellow());
    println!("  {:18} 15 minutes (auto-locks on idle)", "Inactivity Timeout:".bold());
    println!("{}", "═════════════════════════════════════════════".blue());
    println!("  Export token to your shell for instant commands:");
    println!("  {}", format!("export YNTRA_SESSION={}", session_token).green());
    println!("  Use 'yntra lock' to terminate session.\n");

    let manager_clone = manager.clone();
    let manager_for_timeout = manager.clone();
    #[cfg(not(windows))]
    let pipe_name_for_timeout = pipe_name.clone();
    let last_act_clone = last_activity.clone();
    let valid_token = Arc::new(session_token);

    // Auto-lock inactivity checker task
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            let elapsed = last_act_clone.lock().unwrap().elapsed();
            if elapsed > Duration::from_secs(900) { // 15 minutes
                println!("\n{} Inactivity timeout reached (15m). Locking vault session...", "⌛".yellow().bold());
                if let Ok(mut mgr) = manager_for_timeout.lock() {
                    mgr.lock();
                }
                let _ = yntra_vault_core::crypto::clear_clipboard();
                let _ = crate::keychain::clear_session_token();
                #[cfg(not(windows))]
                let _ = std::fs::remove_file(&pipe_name_for_timeout);
                std::process::exit(0);
            }
        }
    });

    let timeout_dur = Duration::from_secs(5);

    #[cfg(windows)]
    {
        use crate::pipe_security::create_pipe;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let mut server = create_pipe(&pipe_name, true)
            .map_err(|e| VaultError::SyncError(format!("Failed to create named pipe server: {}", e)))?;

        loop {
            if server.connect().await.is_ok() {
                *last_activity.lock().unwrap() = Instant::now();

                let mut len_bytes = [0u8; 4];
                if tokio::time::timeout(timeout_dur, server.read_exact(&mut len_bytes)).await.ok().and_then(|r| r.ok()).is_some() {
                    let req_len = u32::from_le_bytes(len_bytes) as usize;
                    if req_len <= 16 * 1024 * 1024 { // 16MB OOM guard
                        let mut req_buf = vec![0u8; req_len];
                        if tokio::time::timeout(timeout_dur, server.read_exact(&mut req_buf)).await.ok().and_then(|r| r.ok()).is_some()
                            && let Ok(envelope) = serde_json::from_slice::<IpcEnvelope>(&req_buf) {
                                use subtle::ConstantTimeEq;
                                let is_authorized = !valid_token.is_empty()
                                    && !envelope.session_token.is_empty()
                                    && bool::from(envelope.session_token.as_bytes().ct_eq(valid_token.as_bytes()));
                                let should_exit = matches!(envelope.request, IpcRequest::Lock) && is_authorized;
                                
                                let response = if is_authorized {
                                    handle_ipc_request(&manager_clone, envelope.request)
                                } else {
                                    IpcResponse::Error("Unauthorized: Invalid YNTRA_SESSION token".into())
                                };

                                let resp_bytes = serde_json::to_vec(&response).unwrap();
                                let resp_len_bytes = (resp_bytes.len() as u32).to_le_bytes();

                                let _ = server.write_all(&resp_len_bytes).await;
                                let _ = server.write_all(&resp_bytes).await;
                                let _ = server.flush().await;

                                if should_exit {
                                    println!("{} Received lock command. Session terminated.", "🔒".yellow().bold());
                                    if let Ok(mut mgr) = manager_clone.lock() {
                                        mgr.lock();
                                    }
                                    let _ = yntra_vault_core::crypto::clear_clipboard();
                                    let _ = crate::keychain::clear_session_token();
                                    std::process::exit(0);
                                }
                            }
                    }
                }
            }

            // Recreate pipe instance for next client
            server = create_pipe(&pipe_name, false)
                .map_err(|e| VaultError::SyncError(format!("Pipe recreate failed: {}", e)))?;
        }
    }

    #[cfg(not(windows))]
    {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = crate::socket_security::bind(std::path::Path::new(&pipe_name)).await?;

        loop {
            if let Ok((mut socket, _)) = listener.accept().await {
                *last_activity.lock().unwrap() = Instant::now();

                let mut len_bytes = [0u8; 4];
                if tokio::time::timeout(timeout_dur, socket.read_exact(&mut len_bytes)).await.ok().and_then(|r| r.ok()).is_some() {
                    let req_len = u32::from_le_bytes(len_bytes) as usize;
                    if req_len <= 16 * 1024 * 1024 { // 16MB OOM guard
                        let mut req_buf = vec![0u8; req_len];
                        if tokio::time::timeout(timeout_dur, socket.read_exact(&mut req_buf)).await.ok().and_then(|r| r.ok()).is_some() {
                            if let Ok(envelope) = serde_json::from_slice::<IpcEnvelope>(&req_buf) {
                                use subtle::ConstantTimeEq;
                                let is_authorized = !valid_token.is_empty()
                                    && !envelope.session_token.is_empty()
                                    && bool::from(envelope.session_token.as_bytes().ct_eq(valid_token.as_bytes()));
                                let should_exit = matches!(envelope.request, IpcRequest::Lock) && is_authorized;
                                
                                let response = if is_authorized {
                                    handle_ipc_request(&manager_clone, envelope.request)
                                } else {
                                    IpcResponse::Error("Unauthorized: Invalid YNTRA_SESSION token".into())
                                };

                                let resp_bytes = serde_json::to_vec(&response).unwrap();
                                let resp_len_bytes = (resp_bytes.len() as u32).to_le_bytes();

                                let _ = socket.write_all(&resp_len_bytes).await;
                                let _ = socket.write_all(&resp_bytes).await;
                                let _ = socket.flush().await;

                                if should_exit {
                                    println!("{} Received lock command. Session terminated.", "🔒".yellow().bold());
                                    if let Ok(mut mgr) = manager_clone.lock() {
                                        mgr.lock();
                                    }
                                    let _ = yntra_vault_core::crypto::clear_clipboard();
                                    let _ = std::fs::remove_file(&pipe_name);
                                    let _ = crate::keychain::clear_session_token();
                                    std::process::exit(0);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn handle_ipc_request(manager: &Arc<Mutex<VaultManager>>, req: IpcRequest) -> IpcResponse {
    let Ok(mut mgr) = manager.lock() else { return IpcResponse::Error("Vault session unavailable".into()); };
    let req = match req {
        IpcRequest::ForVault { path, request } => {
            let requested = std::fs::canonicalize(path);
            let active = std::fs::canonicalize(&mgr.path);
            if !matches!((requested, active), (Ok(requested), Ok(active)) if requested == active) {
                return IpcResponse::Error("The active IPC session belongs to a different vault".into());
            }
            *request
        }
        request => request,
    };
    match req {
        IpcRequest::ForVault { .. } => IpcResponse::Error("Nested vault requests are not supported".into()),
        IpcRequest::Ping => IpcResponse::Pong,
        IpcRequest::ListEntries => {
            match mgr.list_entries() {
                Ok(entries) => IpcResponse::ListEntries(entries),
                Err(e) => IpcResponse::Error(e.to_string()),
            }
        }
        IpcRequest::GetEntry { query } => {
            match resolve_id(&mgr, &query) {
                Ok(id) => match mgr.get_entry(id) {
                    Ok(entry) => IpcResponse::GetEntry(entry),
                    Err(e) => IpcResponse::Error(e.to_string()),
                },
                Err(e) => IpcResponse::Error(e.to_string()),
            }
        }
        IpcRequest::SearchEntries { query } => {
            match mgr.search_entries(&query) {
                Ok(results) => IpcResponse::SearchEntries(results),
                Err(e) => IpcResponse::Error(e.to_string()),
            }
        }
        IpcRequest::AddEntry { new_entry } => {
            match mgr.add_entry(new_entry) {
                Ok(id) => {
                    let _ = mgr.save();
                    IpcResponse::AddEntrySuccess(id)
                }
                Err(e) => IpcResponse::Error(e.to_string()),
            }
        }
        IpcRequest::UpdateEntry { id, update } => {
            match mgr.update_entry(id, update) {
                Ok(_) => {
                    let _ = mgr.save();
                    IpcResponse::UpdateEntrySuccess
                }
                Err(e) => IpcResponse::Error(e.to_string()),
            }
        }
        IpcRequest::DeleteEntry { id, permanent } => {
            let res = if permanent { mgr.permanent_delete(id) } else { mgr.delete_entry(id) };
            match res {
                Ok(_) => {
                    let _ = mgr.save();
                    IpcResponse::DeleteEntrySuccess
                }
                Err(e) => IpcResponse::Error(e.to_string()),
            }
        }
        IpcRequest::SecurityAudit => {
            match mgr.security_audit() {
                Ok(audit) => IpcResponse::AuditSuccess(audit),
                Err(e) => IpcResponse::Error(e.to_string()),
            }
        }
        IpcRequest::Lock => IpcResponse::LockSuccess,
        IpcRequest::SshListIdentities => {
            let mut identities = Vec::new();
            if let Ok(entries) = mgr.list_entries() {
                let total_entries = entries.len();
                for entry in entries {
                    let title_lower = entry.title.to_ascii_lowercase();
                    let is_ssh_candidate = entry.entry_type == EntryType::SshKey
                        || entry.entry_type == EntryType::SecureNote
                        || entry.has_passkey
                        || title_lower.contains("ssh")
                        || title_lower.contains("key")
                        || title_lower.contains("id_")
                        || title_lower.contains("server")
                        || title_lower.contains("git")
                        || entry.tags.iter().any(|t| {
                            let t = t.to_ascii_lowercase();
                            t.contains("ssh") || t.contains("key") || t.contains("server")
                        });

                    // For vaults with <= 200 entries, inspect all entries so users don't miss un-tagged keys.
                    // For larger vaults, rely on candidate heuristics for responsiveness.
                    if !is_ssh_candidate && total_entries > 200 {
                        continue;
                    }

                    if let Ok(full_entry) = mgr.get_entry(entry.id) {
                        let comment = if !full_entry.username.is_empty() {
                            format!("{} ({})", full_entry.title, full_entry.username)
                        } else {
                            full_entry.title.clone()
                        };

                        // 1. Passkey P-256 public key
                        if let Some(ref pubkey_bytes) = full_entry.passkey_public_key {
                            let wire_blob = yntra_crypto::ssh::encode_p256_pubkey(pubkey_bytes);
                            identities.push(SshIdentityPayload {
                                key_blob: wire_blob,
                                comment: comment.clone(),
                            });
                        }

                        // 2. OpenSSH private key in password
                        if let Some(parsed) = yntra_crypto::ssh::parse_openssh_private_key(&full_entry.password)
                            && parsed.can_sign() {
                                let blob = parsed.pubkey_blob().to_vec();
                                if !identities.iter().any(|i| i.key_blob == blob) {
                                    identities.push(SshIdentityPayload {
                                        key_blob: blob,
                                        comment: comment.clone(),
                                    });
                                }
                            }

                        // 3. OpenSSH private key in notes
                        if let Some(parsed) = yntra_crypto::ssh::parse_openssh_private_key(&full_entry.notes)
                            && parsed.can_sign() {
                                let blob = parsed.pubkey_blob().to_vec();
                                if !identities.iter().any(|i| i.key_blob == blob) {
                                    identities.push(SshIdentityPayload {
                                        key_blob: blob,
                                        comment: comment.clone(),
                                    });
                                }
                            }

                        // 4. Custom fields
                        for cf in &full_entry.custom_fields {
                            if let Some(parsed) = yntra_crypto::ssh::parse_openssh_private_key(&cf.value)
                                && parsed.can_sign() {
                                    let blob = parsed.pubkey_blob().to_vec();
                                    if !identities.iter().any(|i| i.key_blob == blob) {
                                        identities.push(SshIdentityPayload {
                                            key_blob: blob,
                                            comment: comment.clone(),
                                        });
                                    }
                                }
                        }

                        // 5. File attachments (e.g. id_ed25519 attached)
                        for att in &full_entry.attachments {
                            let lower = att.name.to_ascii_lowercase();
                            if (lower.contains("id_") || lower.ends_with(".pem") || lower.ends_with(".key") || lower.contains("ssh"))
                                && let Ok(att_bytes) = mgr.get_attachment_data(full_entry.id, att.id)
                                    && let Ok(text) = std::str::from_utf8(&att_bytes)
                                        && let Some(parsed) = yntra_crypto::ssh::parse_openssh_private_key(text)
                                            && parsed.can_sign() {
                                                let blob = parsed.pubkey_blob().to_vec();
                                                if !identities.iter().any(|i| i.key_blob == blob) {
                                                    identities.push(SshIdentityPayload {
                                                        key_blob: blob,
                                                        comment: format!("{}: {}", full_entry.title, att.name),
                                                    });
                                                }
                                            }
                        }
                    }
                }
            }
            IpcResponse::SshIdentities(identities)
        }
        IpcRequest::SshSign { pubkey_blob, data, flags: _ } => {
            let entries = match mgr.list_entries() {
                Ok(e) => e,
                Err(e) => return IpcResponse::Error(e.to_string()),
            };

            for preview in entries {
                let full_entry = match mgr.get_entry(preview.id) {
                    Ok(e) => e,
                    Err(_) => continue,
                };

                // 1. Check passkey
                if preview.has_passkey
                    && let Some(ref pubkey_bytes) = full_entry.passkey_public_key {
                        let wire_blob = yntra_crypto::ssh::encode_p256_pubkey(pubkey_bytes);
                        if wire_blob == pubkey_blob
                            && let Ok(priv_bytes) = mgr.get_passkey_private_key(preview.id) {
                                let parsed = yntra_crypto::ssh::ParsedSshKey::P256 {
                                    pubkey_blob: wire_blob,
                                    private_key: Zeroizing::new(priv_bytes),
                                };
                                match parsed.sign(&data) {
                                    Ok(sig) => return IpcResponse::SshSignSuccess(sig),
                                    Err(e) => return IpcResponse::Error(e.to_string()),
                                }
                            }
                    }

                // 2. Check full entry password, notes, custom fields
                if let Some(parsed) = yntra_crypto::ssh::parse_openssh_private_key(&full_entry.password)
                    && parsed.pubkey_blob() == pubkey_blob.as_slice() && parsed.can_sign() {
                        match parsed.sign(&data) {
                            Ok(sig) => return IpcResponse::SshSignSuccess(sig),
                            Err(e) => return IpcResponse::Error(e.to_string()),
                        }
                    }
                if let Some(parsed) = yntra_crypto::ssh::parse_openssh_private_key(&full_entry.notes)
                    && parsed.pubkey_blob() == pubkey_blob.as_slice() && parsed.can_sign() {
                        match parsed.sign(&data) {
                            Ok(sig) => return IpcResponse::SshSignSuccess(sig),
                            Err(e) => return IpcResponse::Error(e.to_string()),
                        }
                    }
                for cf in &full_entry.custom_fields {
                    if let Some(parsed) = yntra_crypto::ssh::parse_openssh_private_key(&cf.value)
                        && parsed.pubkey_blob() == pubkey_blob.as_slice() && parsed.can_sign() {
                            match parsed.sign(&data) {
                                Ok(sig) => return IpcResponse::SshSignSuccess(sig),
                                Err(e) => return IpcResponse::Error(e.to_string()),
                            }
                        }
                }

                // 3. Check attachments
                for att in &full_entry.attachments {
                    let lower = att.name.to_ascii_lowercase();
                    if (lower.contains("id_") || lower.ends_with(".pem") || lower.ends_with(".key") || lower.contains("ssh"))
                        && let Ok(att_bytes) = mgr.get_attachment_data(full_entry.id, att.id)
                            && let Ok(text) = std::str::from_utf8(&att_bytes)
                                && let Some(parsed) = yntra_crypto::ssh::parse_openssh_private_key(text)
                                    && parsed.pubkey_blob() == pubkey_blob.as_slice() && parsed.can_sign() {
                                        match parsed.sign(&data) {
                                            Ok(sig) => return IpcResponse::SshSignSuccess(sig),
                                            Err(e) => return IpcResponse::Error(e.to_string()),
                                        }
                                    }
                }
            }
            IpcResponse::Error("Matching SSH signing key not found in vault".into())
        }
    }
}

fn resolve_id(mgr: &VaultManager, query: &str) -> Result<Uuid> {
    crate::resolve_entry_id(mgr, query)
}

#[cfg(test)]
mod vault_scope_tests {
    use super::*;

    #[tokio::test]
    async fn a_lost_mutation_reply_cannot_trigger_direct_file_fallback() {
        use tokio::io::AsyncReadExt;
        let (mut client, mut server) = tokio::io::duplex(4096);
        let applied = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let server_applied = applied.clone();
        let request = IpcRequest::DeleteEntry { id: Uuid::new_v4(), permanent: false };
        let envelope = IpcEnvelope { session_token: "synthetic-session".into(), request: request.clone() };
        let worker = tokio::spawn(async move {
            let mut length = [0; 4];
            server.read_exact(&mut length).await.unwrap();
            let mut payload = vec![0; u32::from_le_bytes(length) as usize];
            server.read_exact(&mut payload).await.unwrap();
            let received: IpcEnvelope = serde_json::from_slice(&payload).unwrap();
            assert!(matches!(received.request, IpcRequest::DeleteEntry { .. }));
            server_applied.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            // The daemon applied the request, then disconnected before its reply.
        });
        let response = exchange_request(&mut client, &envelope, Duration::from_secs(1)).await;
        worker.await.unwrap();
        assert_eq!(applied.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(matches!(&response, IpcResponse::Error(message) if message.contains("may have completed")));
        assert!(validate_mutation_response(&request, Some(response)).is_err());
        assert!(validate_mutation_response(&request, Some(IpcResponse::Pong)).is_err());
        assert!(matches!(validate_mutation_response(&request, Some(IpcResponse::DeleteEntrySuccess)), Ok(Some(IpcResponse::DeleteEntrySuccess))));
        assert!(validate_mutation_response(&request, None).unwrap().is_none());
    }

    #[test]
    fn scoped_mutations_cannot_use_a_different_unlocked_vault() {
        let directory = tempfile::tempdir().unwrap();
        let active = directory.path().join("active.vdb");
        let other = directory.path().join("other.vdb");
        let manager = VaultManager::create("Active", "synthetic fixture password", &active).unwrap();
        VaultManager::create("Other", "synthetic fixture password", &other).unwrap();
        let before = std::fs::read(&active).unwrap();
        let manager = Arc::new(Mutex::new(manager));
        let entry = NewEntry { title: "Synthetic Gmail".into(), password: "fixture only".into(), username: String::new(), email: String::new(), url: String::new(), notes: String::new(), tags: vec![], totp_secret: None, custom_fields: vec![], entry_type: None, generate_passkey: None, attachments: None };
        let request = IpcRequest::AddEntry { new_entry: entry.clone() };
        let wrong = handle_ipc_request(&manager, IpcRequest::ForVault { path: other, request: Box::new(request.clone()) });
        assert!(matches!(wrong, IpcResponse::Error(message) if message.contains("different vault")));
        assert!(manager.lock().unwrap().data.entries.is_empty());
        assert_eq!(std::fs::read(&active).unwrap(), before);
        let correct = handle_ipc_request(&manager, IpcRequest::ForVault { path: active.clone(), request: Box::new(request) });
        assert!(matches!(correct, IpcResponse::AddEntrySuccess(_)));
        assert_eq!(VaultManager::open(&active, "synthetic fixture password").unwrap().data.entries.len(), 1);
        let mut guard = manager.lock().unwrap();
        let sibling = guard.add_entry(NewEntry { title: "Synthetic Gmail Work".into(), ..entry }).unwrap();
        assert!(resolve_id(&guard, "Synthetic").is_err());
        assert_eq!(resolve_id(&guard, "Synthetic Gmail Work").unwrap(), sibling);
    }
}
