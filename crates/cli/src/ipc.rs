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

#[derive(Serialize, Deserialize, Debug)]
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

pub fn get_ipc_pipe_name() -> String {
    let username = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "default".to_string());
    #[cfg(windows)]
    {
        format!(r"\\.\pipe\yntra-session-{}", username)
    }
    #[cfg(not(windows))]
    {
        if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
            if !runtime_dir.is_empty() {
                return format!("{}/yntra-session.sock", runtime_dir.trim_end_matches('/'));
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            let user_dir = format!("{}/.local/share/yntra/run", home.trim_end_matches('/'));
            let _ = std::fs::create_dir_all(&user_dir);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&user_dir, std::fs::Permissions::from_mode(0o700));
            }
            return format!("{}/yntra-session.sock", user_dir);
        }
        format!("/tmp/yntra-session-{}.sock", username)
    }
}

pub fn get_session_token_from_env() -> String {
    if let Ok(env_token) = std::env::var("YNTRA_SESSION") {
        if !env_token.is_empty() {
            return env_token;
        }
    }
    crate::keychain::load_session_token().unwrap_or_default()
}

/// Send request to active IPC daemon if available. Returns Ok(Some(response)) if connected.
pub async fn try_ipc_request(req: &IpcRequest) -> Option<IpcResponse> {
    let token = get_session_token_from_env();
    let envelope = IpcEnvelope {
        session_token: token,
        request: req.clone_req(),
    };

    let timeout_dur = Duration::from_secs(5);

    #[cfg(windows)]
    {
        use tokio::net::windows::named_pipe::ClientOptions;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let pipe_name = get_ipc_pipe_name();
        let mut client = ClientOptions::new().open(&pipe_name).ok()?;
        
        let payload = serde_json::to_vec(&envelope).ok()?;
        let len_bytes = (payload.len() as u32).to_le_bytes();
        
        tokio::time::timeout(timeout_dur, client.write_all(&len_bytes)).await.ok()?.ok()?;
        tokio::time::timeout(timeout_dur, client.write_all(&payload)).await.ok()?.ok()?;
        tokio::time::timeout(timeout_dur, client.flush()).await.ok()?.ok()?;

        let mut resp_len_bytes = [0u8; 4];
        tokio::time::timeout(timeout_dur, client.read_exact(&mut resp_len_bytes)).await.ok()?.ok()?;
        let resp_len = u32::from_le_bytes(resp_len_bytes) as usize;

        if resp_len > 16 * 1024 * 1024 {
            return None;
        }

        let mut resp_buf = vec![0u8; resp_len];
        tokio::time::timeout(timeout_dur, client.read_exact(&mut resp_buf)).await.ok()?.ok()?;

        serde_json::from_slice(&resp_buf).ok()
    }
    #[cfg(not(windows))]
    {
        use tokio::net::UnixStream;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let pipe_name = get_ipc_pipe_name();
        let mut stream = UnixStream::connect(&pipe_name).await.ok()?;

        let payload = serde_json::to_vec(&envelope).ok()?;
        let len_bytes = (payload.len() as u32).to_le_bytes();

        tokio::time::timeout(timeout_dur, stream.write_all(&len_bytes)).await.ok()?.ok()?;
        tokio::time::timeout(timeout_dur, stream.write_all(&payload)).await.ok()?.ok()?;
        tokio::time::timeout(timeout_dur, stream.flush()).await.ok()?.ok()?;

        let mut resp_len_bytes = [0u8; 4];
        tokio::time::timeout(timeout_dur, stream.read_exact(&mut resp_len_bytes)).await.ok()?.ok()?;
        let resp_len = u32::from_le_bytes(resp_len_bytes) as usize;

        if resp_len > 16 * 1024 * 1024 {
            return None;
        }

        let mut resp_buf = vec![0u8; resp_len];
        tokio::time::timeout(timeout_dur, stream.read_exact(&mut resp_buf)).await.ok()?.ok()?;

        serde_json::from_slice(&resp_buf).ok()
    }
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

    let pipe_name = get_ipc_pipe_name();

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
    let last_act_clone = last_activity.clone();
    let valid_token = Arc::new(session_token);

    // Auto-lock inactivity checker task
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            let elapsed = last_act_clone.lock().unwrap().elapsed();
            if elapsed > Duration::from_secs(900) { // 15 minutes
                println!("\n{} Inactivity timeout reached (15m). Locking vault session...", "⌛".yellow().bold());
                let _ = crate::keychain::clear_session_token();
                std::process::exit(0);
            }
        }
    });

    let timeout_dur = Duration::from_secs(5);

    #[cfg(windows)]
    {
        use tokio::net::windows::named_pipe::ServerOptions;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let mut server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&pipe_name)
            .map_err(|e| VaultError::SyncError(format!("Failed to create named pipe server: {}", e)))?;

        loop {
            if server.connect().await.is_ok() {
                *last_activity.lock().unwrap() = Instant::now();

                let mut len_bytes = [0u8; 4];
                if tokio::time::timeout(timeout_dur, server.read_exact(&mut len_bytes)).await.ok().and_then(|r| r.ok()).is_some() {
                    let req_len = u32::from_le_bytes(len_bytes) as usize;
                    if req_len <= 16 * 1024 * 1024 { // 16MB OOM guard
                        let mut req_buf = vec![0u8; req_len];
                        if tokio::time::timeout(timeout_dur, server.read_exact(&mut req_buf)).await.ok().and_then(|r| r.ok()).is_some() {
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

                                let _ = server.write_all(&resp_len_bytes).await;
                                let _ = server.write_all(&resp_bytes).await;
                                let _ = server.flush().await;

                                if should_exit {
                                    println!("{} Received lock command. Session terminated.", "🔒".yellow().bold());
                                    let _ = crate::keychain::clear_session_token();
                                    std::process::exit(0);
                                }
                            }
                        }
                    }
                }
            }

            // Recreate pipe instance for next client
            server = ServerOptions::new()
                .create(&pipe_name)
                .map_err(|e| VaultError::SyncError(format!("Pipe recreate failed: {}", e)))?;
        }
    }

    #[cfg(not(windows))]
    {
        use tokio::net::UnixListener;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let _ = std::fs::remove_file(&pipe_name);
        let listener = UnixListener::bind(&pipe_name)
            .map_err(|e| VaultError::SyncError(format!("Failed to bind unix socket: {}", e)))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&pipe_name, std::fs::Permissions::from_mode(0o600));
        }

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
    let mut mgr = manager.lock().unwrap();
    match req {
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
                        if let Some(parsed) = yntra_crypto::ssh::parse_openssh_private_key(&full_entry.password) {
                            if parsed.can_sign() {
                                let blob = parsed.pubkey_blob().to_vec();
                                if !identities.iter().any(|i| i.key_blob == blob) {
                                    identities.push(SshIdentityPayload {
                                        key_blob: blob,
                                        comment: comment.clone(),
                                    });
                                }
                            }
                        }

                        // 3. OpenSSH private key in notes
                        if let Some(parsed) = yntra_crypto::ssh::parse_openssh_private_key(&full_entry.notes) {
                            if parsed.can_sign() {
                                let blob = parsed.pubkey_blob().to_vec();
                                if !identities.iter().any(|i| i.key_blob == blob) {
                                    identities.push(SshIdentityPayload {
                                        key_blob: blob,
                                        comment: comment.clone(),
                                    });
                                }
                            }
                        }

                        // 4. Custom fields
                        for cf in &full_entry.custom_fields {
                            if let Some(parsed) = yntra_crypto::ssh::parse_openssh_private_key(&cf.value) {
                                if parsed.can_sign() {
                                    let blob = parsed.pubkey_blob().to_vec();
                                    if !identities.iter().any(|i| i.key_blob == blob) {
                                        identities.push(SshIdentityPayload {
                                            key_blob: blob,
                                            comment: comment.clone(),
                                        });
                                    }
                                }
                            }
                        }

                        // 5. File attachments (e.g. id_ed25519 attached)
                        for att in &full_entry.attachments {
                            let lower = att.name.to_ascii_lowercase();
                            if lower.contains("id_") || lower.ends_with(".pem") || lower.ends_with(".key") || lower.contains("ssh") {
                                if let Ok(att_bytes) = mgr.get_attachment_data(full_entry.id, att.id) {
                                    if let Ok(text) = std::str::from_utf8(&att_bytes) {
                                        if let Some(parsed) = yntra_crypto::ssh::parse_openssh_private_key(text) {
                                            if parsed.can_sign() {
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
                if preview.has_passkey {
                    if let Some(ref pubkey_bytes) = full_entry.passkey_public_key {
                        let wire_blob = yntra_crypto::ssh::encode_p256_pubkey(pubkey_bytes);
                        if wire_blob == pubkey_blob {
                            if let Ok(priv_bytes) = mgr.get_passkey_private_key(preview.id) {
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
                    }
                }

                // 2. Check full entry password, notes, custom fields
                if let Some(parsed) = yntra_crypto::ssh::parse_openssh_private_key(&full_entry.password) {
                    if parsed.pubkey_blob() == pubkey_blob.as_slice() && parsed.can_sign() {
                        match parsed.sign(&data) {
                            Ok(sig) => return IpcResponse::SshSignSuccess(sig),
                            Err(e) => return IpcResponse::Error(e.to_string()),
                        }
                    }
                }
                if let Some(parsed) = yntra_crypto::ssh::parse_openssh_private_key(&full_entry.notes) {
                    if parsed.pubkey_blob() == pubkey_blob.as_slice() && parsed.can_sign() {
                        match parsed.sign(&data) {
                            Ok(sig) => return IpcResponse::SshSignSuccess(sig),
                            Err(e) => return IpcResponse::Error(e.to_string()),
                        }
                    }
                }
                for cf in &full_entry.custom_fields {
                    if let Some(parsed) = yntra_crypto::ssh::parse_openssh_private_key(&cf.value) {
                        if parsed.pubkey_blob() == pubkey_blob.as_slice() && parsed.can_sign() {
                            match parsed.sign(&data) {
                                Ok(sig) => return IpcResponse::SshSignSuccess(sig),
                                Err(e) => return IpcResponse::Error(e.to_string()),
                            }
                        }
                    }
                }

                // 3. Check attachments
                for att in &full_entry.attachments {
                    let lower = att.name.to_ascii_lowercase();
                    if lower.contains("id_") || lower.ends_with(".pem") || lower.ends_with(".key") || lower.contains("ssh") {
                        if let Ok(att_bytes) = mgr.get_attachment_data(full_entry.id, att.id) {
                            if let Ok(text) = std::str::from_utf8(&att_bytes) {
                                if let Some(parsed) = yntra_crypto::ssh::parse_openssh_private_key(text) {
                                    if parsed.pubkey_blob() == pubkey_blob.as_slice() && parsed.can_sign() {
                                        match parsed.sign(&data) {
                                            Ok(sig) => return IpcResponse::SshSignSuccess(sig),
                                            Err(e) => return IpcResponse::Error(e.to_string()),
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            IpcResponse::Error("Matching SSH signing key not found in vault".into())
        }
    }
}

fn resolve_id(mgr: &VaultManager, query: &str) -> Result<Uuid> {
    if let Ok(id) = Uuid::parse_str(query) {
        return Ok(id);
    }
    let entries = mgr.list_entries()?;
    let matches: Vec<_> = entries.iter().filter(|e| e.title.to_lowercase().contains(&query.to_lowercase())).collect();
    if matches.is_empty() {
        return Err(VaultError::EntryNotFound(format!("No entry matching title/UUID '{}'", query)));
    }
    let exact_matches: Vec<_> = matches.iter().filter(|e| e.title.eq_ignore_ascii_case(query)).collect();
    if exact_matches.len() == 1 {
        return Ok(exact_matches[0].id);
    }
    if exact_matches.len() > 1 {
        return Err(VaultError::InvalidFormat(format!(
            "Ambiguous match: Found {} entries titled '{}'. Please specify exact UUID.",
            exact_matches.len(), query
        )));
    }
    Ok(matches[0].id)
}
