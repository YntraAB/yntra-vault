//! Yntra Vault — Built-in OpenSSH Agent Socket Protocol Server
//!
//! Provides in-memory SSH authentication challenge signing (`SSH_AUTH_SOCK`)
//! without exposing SSH private key bytes to disk.

use colored::*;
use yntra_vault_core::{Result, VaultError};
use crate::ipc::{try_ipc_request, IpcRequest, IpcResponse};

pub async fn run_ssh_agent_pipe() -> Result<()> {
    let pipe_name = get_ssh_agent_pipe_name();
    println!("{} Starting Yntra SSH Agent pipe server on {}", "🔑".bold(), pipe_name.cyan());
    println!("   Set SSH_AUTH_SOCK to use with git and ssh:");
    println!("   {}", format!("$env:SSH_AUTH_SOCK='{}'", pipe_name).green());

    #[cfg(windows)]
    {
        use tokio::net::windows::named_pipe::ServerOptions;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let mut server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&pipe_name)
            .map_err(|e| VaultError::SyncError(format!("Failed to create SSH agent pipe: {}", e)))?;

        loop {
            if server.connect().await.is_ok() {
                let mut client = server;
                server = match ServerOptions::new().create(&pipe_name) {
                    Ok(next) => next,
                    Err(e) => {
                        eprintln!("Pipe recreate error: {}", e);
                        break;
                    }
                };

                tokio::spawn(async move {
                    loop {
                        let mut len_bytes = [0u8; 4];
                        if client.read_exact(&mut len_bytes).await.is_err() {
                            break;
                        }
                        let packet_len = u32::from_be_bytes(len_bytes) as usize;
                        if packet_len == 0 || packet_len > 10 * 1024 * 1024 {
                            break;
                        }
                        let mut req_buf = vec![0u8; packet_len];
                        if client.read_exact(&mut req_buf).await.is_err() {
                            break;
                        }
                        let resp = handle_ssh_agent_packet(&req_buf).await;
                        let resp_len_bytes = (resp.len() as u32).to_be_bytes();
                        if client.write_all(&resp_len_bytes).await.is_err() {
                            break;
                        }
                        if client.write_all(&resp).await.is_err() {
                            break;
                        }
                        if client.flush().await.is_err() {
                            break;
                        }
                    }
                });
            } else {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                server = ServerOptions::new()
                    .create(&pipe_name)
                    .map_err(|e| VaultError::SyncError(format!("Pipe recreate failed: {}", e)))?;
            }
        }
        Ok(())
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
                tokio::spawn(async move {
                    loop {
                        let mut len_bytes = [0u8; 4];
                        if socket.read_exact(&mut len_bytes).await.is_err() {
                            break;
                        }
                        let packet_len = u32::from_be_bytes(len_bytes) as usize;
                        if packet_len == 0 || packet_len > 10 * 1024 * 1024 {
                            break;
                        }
                        let mut req_buf = vec![0u8; packet_len];
                        if socket.read_exact(&mut req_buf).await.is_err() {
                            break;
                        }
                        let resp = handle_ssh_agent_packet(&req_buf).await;
                        let resp_len_bytes = (resp.len() as u32).to_be_bytes();
                        if socket.write_all(&resp_len_bytes).await.is_err() {
                            break;
                        }
                        if socket.write_all(&resp).await.is_err() {
                            break;
                        }
                        if socket.flush().await.is_err() {
                            break;
                        }
                    }
                });
            }
        }
        #[allow(unreachable_code)]
        Ok(())
    }
}

pub fn get_ssh_agent_pipe_name() -> String {
    let username = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "default".to_string());
    #[cfg(windows)]
    {
        format!(r"\\.\pipe\yntra-ssh-agent-{}", username)
    }
    #[cfg(not(windows))]
    {
        if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
            if !runtime_dir.is_empty() {
                return format!("{}/yntra-ssh-agent.sock", runtime_dir.trim_end_matches('/'));
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
            return format!("{}/yntra-ssh-agent.sock", user_dir);
        }
        format!("/tmp/yntra-ssh-agent-{}.sock", username)
    }
}

async fn handle_ssh_agent_packet(packet: &[u8]) -> Vec<u8> {
    if packet.is_empty() {
        return vec![5]; // SSH_AGENT_FAILURE
    }

    let msg_type = packet[0];
    match msg_type {
        11 => { // SSH2_AGENTC_REQUEST_IDENTITIES
            let identities = match try_ipc_request(&IpcRequest::SshListIdentities).await {
                Some(IpcResponse::SshIdentities(ids)) => ids,
                _ => Vec::new(),
            };

            let mut resp = vec![12]; // SSH2_AGENT_IDENTITIES_ANSWER
            let count_bytes = (identities.len() as u32).to_be_bytes();
            resp.extend_from_slice(&count_bytes);

            for id in identities {
                let blob_len = (id.key_blob.len() as u32).to_be_bytes();
                resp.extend_from_slice(&blob_len);
                resp.extend_from_slice(&id.key_blob);

                let comment_bytes = id.comment.as_bytes();
                let comment_len = (comment_bytes.len() as u32).to_be_bytes();
                resp.extend_from_slice(&comment_len);
                resp.extend_from_slice(comment_bytes);
            }
            resp
        }
        13 => { // SSH2_AGENTC_SIGN_REQUEST
            // Wire format: [13, string key_blob, string data, uint32 flags]
            let mut cursor = 1;
            let key_blob = match read_ssh_string(packet, &mut cursor) {
                Some(k) => k.to_vec(),
                None => return vec![5],
            };
            let data = match read_ssh_string(packet, &mut cursor) {
                Some(d) => d.to_vec(),
                None => return vec![5],
            };
            let flags = match read_u32(packet, &mut cursor) {
                Some(f) => f,
                None => 0,
            };

            match try_ipc_request(&IpcRequest::SshSign { pubkey_blob: key_blob, data, flags }).await {
                Some(IpcResponse::SshSignSuccess(sig_blob)) => {
                    let mut resp = vec![14]; // SSH2_AGENT_SIGN_RESPONSE
                    let sig_len = (sig_blob.len() as u32).to_be_bytes();
                    resp.extend_from_slice(&sig_len);
                    resp.extend_from_slice(&sig_blob);
                    resp
                }
                _ => vec![5], // SSH_AGENT_FAILURE
            }
        }
        _ => vec![5], // SSH_AGENT_FAILURE
    }
}

fn read_u32(buf: &[u8], cursor: &mut usize) -> Option<u32> {
    let end = cursor.checked_add(4)?;
    if end > buf.len() {
        return None;
    }
    let val = u32::from_be_bytes(buf[*cursor..end].try_into().ok()?);
    *cursor = end;
    Some(val)
}

fn read_ssh_string<'a>(buf: &'a [u8], cursor: &mut usize) -> Option<&'a [u8]> {
    let len = read_u32(buf, cursor)? as usize;
    let end = cursor.checked_add(len)?;
    if end > buf.len() {
        return None;
    }
    let slice = &buf[*cursor..end];
    *cursor = end;
    Some(slice)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ssh_agent_empty_packet() {
        let resp = handle_ssh_agent_packet(&[]).await;
        assert_eq!(resp, vec![5]);
    }

    #[tokio::test]
    async fn test_ssh_agent_unknown_packet() {
        let resp = handle_ssh_agent_packet(&[99]).await;
        assert_eq!(resp, vec![5]);
    }

    #[tokio::test]
    async fn test_ssh_agent_identities_answer_framing() {
        let packet = [11];
        let resp = handle_ssh_agent_packet(&packet).await;
        assert_eq!(resp[0], 12);
        let count = u32::from_be_bytes(resp[1..5].try_into().unwrap());
        assert_eq!(count, 0);
    }
}
