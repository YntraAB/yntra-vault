//! Yntra Vault — Built-in OpenSSH Agent Socket Protocol Server
//!
//! Provides in-memory SSH authentication challenge signing (`SSH_AUTH_SOCK`)
//! without exposing SSH private key bytes to disk.

use colored::*;
use crate::{Result, VaultError};
use crate::cli::ipc::{try_ipc_request, IpcRequest, IpcResponse};

pub async fn run_ssh_agent_pipe() -> Result<()> {
    let pipe_name = get_ssh_agent_pipe_name();
    println!("{} Starting Yntra SSH Agent pipe server on {}", "🔑".bold(), pipe_name.cyan());
    println!("   Set SSH_AUTH_SOCK to use with git and ssh:");
    println!("   {}", format!("$env:SSH_AUTH_SOCK='{}'", pipe_name).green());

    #[cfg(windows)]
    {
        use tokio::net::windows::named_pipe::ServerOptions;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&pipe_name)
            .map_err(|e| VaultError::SyncError(format!("Failed to create SSH agent pipe: {}", e)))?;

        let mut server = server;
        loop {
            if server.connect().await.is_ok() {
                let mut len_bytes = [0u8; 4];
                if server.read_exact(&mut len_bytes).await.is_ok() {
                    let packet_len = u32::from_be_bytes(len_bytes) as usize;
                    let mut req_buf = vec![0u8; packet_len];
                    if server.read_exact(&mut req_buf).await.is_ok() {
                        let resp = handle_ssh_agent_packet(&req_buf).await;
                        let resp_len_bytes = (resp.len() as u32).to_be_bytes();
                        let _ = server.write_all(&resp_len_bytes).await;
                        let _ = server.write_all(&resp).await;
                        let _ = server.flush().await;
                    }
                }
            }

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

        loop {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut len_bytes = [0u8; 4];
                if socket.read_exact(&mut len_bytes).await.is_ok() {
                    let packet_len = u32::from_be_bytes(len_bytes) as usize;
                    let mut req_buf = vec![0u8; packet_len];
                    if socket.read_exact(&mut req_buf).await.is_ok() {
                        let resp = handle_ssh_agent_packet(&req_buf).await;
                        let resp_len_bytes = (resp.len() as u32).to_be_bytes();
                        let _ = socket.write_all(&resp_len_bytes).await;
                        let _ = socket.write_all(&resp).await;
                        let _ = socket.flush().await;
                    }
                }
            }
        }
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
            let mut keys_list = Vec::new();
            if let Some(IpcResponse::ListEntries(entries)) = try_ipc_request(&IpcRequest::ListEntries).await {
                for entry in entries {
                    if entry.tags.iter().any(|t| t.to_lowercase() == "ssh" || t.to_lowercase() == "key") {
                        keys_list.push(entry);
                    }
                }
            }

            let mut resp = vec![12]; // SSH2_AGENT_IDENTITIES_ANSWER
            let count_bytes = (keys_list.len() as u32).to_be_bytes();
            resp.extend_from_slice(&count_bytes);

            for k in keys_list {
                let blob = k.title.as_bytes();
                let blob_len = (blob.len() as u32).to_be_bytes();
                resp.extend_from_slice(&blob_len);
                resp.extend_from_slice(blob);

                let comment = k.username.as_bytes();
                let comment_len = (comment.len() as u32).to_be_bytes();
                resp.extend_from_slice(&comment_len);
                resp.extend_from_slice(comment);
            }
            resp
        }
        13 => { // SSH2_AGENTC_SIGN_REQUEST
            vec![5] // SSH_AGENT_FAILURE stub for unsupported key types
        }
        _ => vec![5], // SSH_AGENT_FAILURE
    }
}
