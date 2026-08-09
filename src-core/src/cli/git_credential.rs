//! Yntra Vault — Git Credential Helper Engine (`yntra git-credential <get|store|erase|setup>`)
//!
//! Implements official Git Credential Helper framing protocol for seamless
//! automated git authentication over HTTPS.

use std::io::{BufRead, stdin};
use std::process::Command;
use colored::*;

use crate::{Result, VaultError};
use crate::cli::ipc::{try_ipc_request, IpcRequest, IpcResponse};

pub async fn handle_git_credential(action: &str) -> Result<()> {
    match action {
        "get" => handle_git_get().await,
        "store" | "erase" => Ok(()), // No-op for read-only query helper
        "setup" => handle_git_setup(),
        other => Err(VaultError::InvalidFormat(format!("Unknown git-credential action '{}'", other))),
    }
}

async fn handle_git_get() -> Result<()> {
    let mut host = String::new();
    let mut username = String::new();

    let stdin = stdin();
    for line in stdin.lock().lines().flatten() {
        let line_trim = line.trim();
        if line_trim.is_empty() {
            break;
        }
        if let Some((k, v)) = line_trim.split_once('=') {
            match k.trim() {
                "host" => host = v.trim().to_string(),
                "username" => username = v.trim().to_string(),
                _ => {}
            }
        }
    }

    if host.is_empty() {
        return Ok(());
    }

    // Query IPC daemon for matching host/URL
    let query = if !username.is_empty() {
        format!("{} {}", host, username)
    } else {
        host.clone()
    };

    if let Some(IpcResponse::GetEntry(entry)) = try_ipc_request(&IpcRequest::GetEntry { query }).await {
        println!("username={}", entry.username);
        println!("password={}", entry.password);
    } else if let Some(IpcResponse::SearchEntries(list)) = try_ipc_request(&IpcRequest::SearchEntries { query: host.clone() }).await {
        if let Some(first) = list.first() {
            if let Some(IpcResponse::GetEntry(entry)) = try_ipc_request(&IpcRequest::GetEntry { query: first.id.to_string() }).await {
                println!("username={}", entry.username);
                println!("password={}", entry.password);
            }
        }
    }

    Ok(())
}

fn handle_git_setup() -> Result<()> {
    let exe = std::env::current_exe()
        .map_err(|e| VaultError::InvalidFormat(format!("Failed to locate yntra executable: {}", e)))?;
    let helper_cmd = format!("\"{}\" git-credential", exe.display().to_string().replace('\\', "/"));

    let status = Command::new("git")
        .args(&["config", "--global", "credential.helper", &helper_cmd])
        .status()
        .map_err(|e| VaultError::InvalidFormat(format!("Failed to run git config: {}", e)))?;

    if status.success() {
        println!("{} Configured Git Credential Helper globally!", "✓".green().bold());
        println!("   Helper Command: {}", helper_cmd.cyan());
    } else {
        return Err(VaultError::InvalidFormat("git config command exited with error status".into()));
    }

    Ok(())
}
