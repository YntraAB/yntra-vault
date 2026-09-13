//! Yntra Vault — Secret Injection Engine (`yntra run -- <command>`)
//!
//! Substituted environment references formatted as `yntra://<entry_query>/<field>`
//! in process environment variables or `.env` template files before executing child process.

use std::process::Command;
use std::path::{Path, PathBuf};
use colored::*;

use yntra_vault_core::{
    Result, VaultError,
    vault::VaultManager,
    totp::{generate_totp, parse_otpauth_uri, TotpConfig},
};
use crate::ipc::{try_ipc_request, IpcRequest, IpcResponse};

pub async fn execute_secret_injection(
    vault_path: &Path,
    password: Option<String>,
    keyfile: Option<&Path>,
    env_file: Option<PathBuf>,
    cmd: String,
    args: Vec<String>,
) -> Result<()> {
    let mut env_vars = std::env::vars().collect::<Vec<(String, String)>>();
    let mut replacements_count = 0;

    // Load .env or .env.tpl file if specified
    if let Some(ref env_path) = env_file {
        if !env_path.exists() {
            return Err(VaultError::VaultNotFound(format!("Env template file not found at: {}", env_path.display())));
        }
        let content = std::fs::read_to_string(env_path)
            .map_err(|e| VaultError::InvalidFormat(format!("Failed to read env file: {}", e)))?;
        for line in content.lines() {
            let line_trim = line.trim();
            if line_trim.is_empty() || line_trim.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line_trim.split_once('=') {
                let key = k.trim().to_string();
                let mut val = v.trim().trim_matches('"').trim_matches('\'').to_string();
                if val.contains("yntra://") {
                    if let Some(resolved) = resolve_yntra_secret_uri(vault_path, password.clone(), keyfile, &val).await? {
                        val = resolved;
                        replacements_count += 1;
                    }
                }
                env_vars.push((key, val));
            }
        }
    }

    // Process system environment variables
    for (_key, val) in env_vars.iter_mut() {
        if val.contains("yntra://") {
            if let Some(resolved) = resolve_yntra_secret_uri(vault_path, password.clone(), keyfile, val).await? {
                *val = resolved;
                replacements_count += 1;
            }
        }
    }

    if replacements_count > 0 {
        println!("{} Injected {} Yntra Vault secrets into child environment for '{}'", "🔑".bold(), replacements_count, cmd);
    }

    let mut child = Command::new(&cmd)
        .args(&args)
        .envs(env_vars)
        .spawn()
        .map_err(|e| VaultError::InvalidFormat(format!("Failed to execute command '{}': {}", cmd, e)))?;

    let status = child.wait()
        .map_err(|e| VaultError::InvalidFormat(format!("Child command execution error: {}", e)))?;

    if !status.success() {
        let code = status.code().unwrap_or(1);
        std::process::exit(code);
    }

    Ok(())
}

async fn resolve_yntra_secret_uri(
    vault_path: &Path,
    password: Option<String>,
    keyfile: Option<&Path>,
    uri: &str,
) -> Result<Option<String>> {
    // Format: yntra://<entry_query>/<field>
    let rest = match uri.strip_prefix("yntra://") {
        Some(r) => r,
        None => return Ok(None),
    };

    let parts: Vec<&str> = rest.splitn(2, '/').collect();
    if parts.len() < 2 {
        return Err(VaultError::InvalidFormat(format!("Invalid secret URI format '{}'. Expected yntra://<entry>/<field>", uri)));
    }

    let entry_query = parts[0];
    let field = parts[1].to_lowercase();

    // Try fast IPC first
    let entry = if let Some(IpcResponse::GetEntry(e)) = try_ipc_request(&IpcRequest::GetEntry { query: entry_query.to_string() }).await {
        e
    } else {
        let pass = password.ok_or_else(|| VaultError::VaultLocked)?;
        let manager = VaultManager::open_with_keyfile(vault_path, &pass, keyfile)?;
        let id = resolve_entry_id_from_manager(&manager, entry_query)?;
        manager.get_entry(id)?
    };

    let resolved = match field.as_str() {
        "password" | "pass" | "secret" => entry.password,
        "username" | "user" => entry.username,
        "url" => entry.url,
        "email" => entry.email,
        "notes" => entry.notes,
        "totp" => {
            let secret = entry.totp_secret.ok_or_else(|| VaultError::InvalidFormat(format!("Entry '{}' has no TOTP secret", entry.title)))?;
            let cfg = if secret.starts_with("otpauth://") {
                parse_otpauth_uri(&secret)?
            } else {
                TotpConfig { secret, ..Default::default() }
            };
            generate_totp(&cfg)?.code
        }
        custom => {
            let field_val = entry.custom_fields.iter().find(|f| f.name.eq_ignore_ascii_case(custom)).map(|f| f.value.clone());
            field_val.ok_or_else(|| VaultError::InvalidFormat(format!("Custom field '{}' not found on entry '{}'", custom, entry.title)))?
        }
    };

    Ok(Some(resolved))
}

fn resolve_entry_id_from_manager(manager: &VaultManager, query: &str) -> Result<uuid::Uuid> {
    if let Ok(id) = uuid::Uuid::parse_str(query) {
        return Ok(id);
    }
    let entries = manager.list_entries()?;
    let matches: Vec<_> = entries.iter().filter(|e| e.title.to_lowercase().contains(&query.to_lowercase())).collect();
    if matches.is_empty() {
        return Err(VaultError::EntryNotFound(format!("No entry matching title/UUID '{}'", query)));
    }
    let exact_matches: Vec<_> = matches.iter().filter(|e| e.title.eq_ignore_ascii_case(query)).collect();
    if exact_matches.len() == 1 {
        return Ok(exact_matches[0].id);
    }
    Ok(matches[0].id)
}
