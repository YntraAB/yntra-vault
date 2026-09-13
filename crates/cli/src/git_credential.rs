//! Yntra Vault — Git Credential Helper Engine (`yntra git-credential <get|store|erase|setup>`)
//!
//! Implements official Git Credential Helper framing protocol for seamless
//! automated git authentication over HTTPS.

use std::io::{BufRead, stdin};
use std::process::Command;
use colored::*;

use yntra_vault_core::{Result, VaultError};
use crate::ipc::{try_ipc_request, IpcRequest, IpcResponse};

use yntra_vault_core::vault::EntryPreview;

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
                "url" => {
                    if host.is_empty() {
                        host = normalize_host(v.trim());
                    }
                }
                "username" => username = v.trim().to_string(),
                _ => {}
            }
        }
    }

    if host.is_empty() {
        return Ok(());
    }

    // 1. Fetch all entries from the IPC daemon
    let entries = match try_ipc_request(&IpcRequest::ListEntries).await {
        Some(IpcResponse::ListEntries(list)) => list,
        _ => Vec::new(),
    };

    // 2. Score entries against host & requested username
    let mut scored: Vec<(&EntryPreview, u32)> = entries
        .iter()
        .map(|e| (e, score_entry_match(e, &host, &username)))
        .filter(|(_, score)| *score > 0)
        .collect();

    scored.sort_by(|a, b| b.1.cmp(&a.1));

    let best_id = scored.first().map(|(e, _)| e.id);

    // 3. Fallback: if no scored match from ListEntries, try search_entries
    let target_id = if let Some(id) = best_id {
        Some(id)
    } else {
        let stem = extract_domain_stem(&host);
        if let Some(IpcResponse::SearchEntries(list)) = try_ipc_request(&IpcRequest::SearchEntries { query: stem.to_string() }).await {
            list.first().map(|e| e.id)
        } else {
            None
        }
    };

    // 4. Retrieve decrypted credentials and write to stdout in git-credential format
    if let Some(id) = target_id {
        if let Some(IpcResponse::GetEntry(entry)) = try_ipc_request(&IpcRequest::GetEntry { query: id.to_string() }).await {
            use std::io::Write;
            let user = if !entry.username.is_empty() {
                &entry.username
            } else {
                &entry.email
            };
            println!("username={}", user);
            println!("password={}", entry.password);
            let _ = std::io::stdout().flush();
        }
    }

    Ok(())
}

pub fn normalize_host(host: &str) -> String {
    let no_proto = if let Some((_, rest)) = host.split_once("://") {
        rest
    } else {
        host
    };
    let no_port = no_proto.split(':').next().unwrap_or(no_proto);
    let domain = if let Some((d, _)) = no_port.split_once('/') {
        d
    } else {
        no_port
    };
    domain.trim_matches('/').to_lowercase()
}

pub fn extract_domain_stem(host: &str) -> &str {
    let clean = host.trim();
    let no_proto = if let Some((_, rest)) = clean.split_once("://") {
        rest
    } else {
        clean
    };
    let no_port = no_proto.split(':').next().unwrap_or(no_proto);
    let trimmed = no_port.trim_matches('/');

    let parts: Vec<&str> = trimmed.split('.').collect();
    if parts.len() >= 2 {
        parts[0]
    } else {
        trimmed
    }
}

pub fn score_entry_match(entry: &EntryPreview, host: &str, requested_user: &str) -> u32 {
    let clean_host = normalize_host(host);
    let clean_stem = extract_domain_stem(&clean_host).to_lowercase();

    let entry_title = entry.title.to_lowercase();
    let entry_domain = normalize_host(&entry.url);
    let entry_user = entry.username.to_lowercase();
    let entry_email = entry.email.to_lowercase();
    let req_user = requested_user.trim().to_lowercase();

    let mut host_score: u32 = 0;
    if !entry_domain.is_empty() && (entry_domain.contains(&clean_host) || clean_host.contains(&entry_domain)) {
        host_score = 150;
    } else if entry_title == clean_host {
        host_score = 140;
    } else if entry_title.contains(&clean_host) {
        host_score = 120;
    } else if !clean_stem.is_empty() && entry_title.contains(&clean_stem) {
        host_score = 80;
    } else if !clean_stem.is_empty() && clean_host.contains(&entry_title) && !entry_title.is_empty() {
        host_score = 70;
    } else if !clean_stem.is_empty() && entry_domain.contains(&clean_stem) {
        host_score = 60;
    }

    if host_score == 0 {
        return 0;
    }

    if req_user.is_empty() {
        host_score
    } else if entry_user == req_user || (!entry_email.is_empty() && entry_email == req_user) {
        host_score + 150
    } else if entry_user.is_empty() && entry_email.is_empty() {
        host_score + 10
    } else {
        // Different username explicitly requested: don't match the wrong account
        0
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    use yntra_vault_core::vault::types::{EntryType, BreachStatus};

    fn make_preview(title: &str, username: &str, url: &str) -> EntryPreview {
        EntryPreview {
            id: Uuid::new_v4(),
            title: title.to_string(),
            username: username.to_string(),
            url: url.to_string(),
            email: String::new(),
            entry_type: EntryType::Login,
            tags: vec![],
            favorite: false,
            pinned: false,
            has_totp: false,
            updated_at: chrono::Utc::now(),
            breach_status: BreachStatus::Unknown,
            strength_score: None,
            password_age_days: 0,
            has_passkey: false,
            attachment_count: 0,
        }
    }

    #[test]
    fn test_score_entry_match_github_title_and_username() {
        let entry = make_preview("GitHub", "octocat", "");
        let score = score_entry_match(&entry, "github.com", "octocat");
        assert!(score > 0);
        assert_eq!(score, 230);
    }

    #[test]
    fn test_score_entry_match_different_username_penalized() {
        let entry = make_preview("GitHub", "other_user", "https://github.com");
        let score = score_entry_match(&entry, "github.com", "octocat");
        assert_eq!(score, 0); // Must not match the wrong user!
    }

    #[test]
    fn test_score_entry_match_url_matching() {
        let entry = make_preview("Enterprise Git", "dev_user", "https://git.mycompany.org");
        let score = score_entry_match(&entry, "git.mycompany.org", "dev_user");
        assert!(score >= 300);
    }

    #[test]
    fn test_score_entry_match_no_username_requested() {
        let entry = make_preview("GitLab", "gitlab_user", "");
        let score = score_entry_match(&entry, "gitlab.com", "");
        assert!(score > 0);
    }

    #[test]
    fn test_extract_domain_stem() {
        assert_eq!(extract_domain_stem("github.com"), "github");
        assert_eq!(extract_domain_stem("https://gitlab.com:8443"), "gitlab");
        assert_eq!(extract_domain_stem("bitbucket.org"), "bitbucket");
    }

    #[test]
    fn test_normalize_host_and_custom_ports() {
        assert_eq!(normalize_host("https://gitlab.example.com:8443/repo.git"), "gitlab.example.com");
        assert_eq!(normalize_host("gitlab.example.com:8443"), "gitlab.example.com");

        let entry = make_preview("Self-Hosted GitLab", "corp_user", "https://gitlab.example.com");
        // Git sends host with port 8443:
        let score = score_entry_match(&entry, "gitlab.example.com:8443", "corp_user");
        assert!(score >= 300); // Matches despite port in Git host request!
    }

    #[test]
    fn test_score_entry_match_email_matching() {
        let mut entry = make_preview("GitHub", "", "https://github.com");
        entry.email = "octocat@github.com".to_string();

        let score = score_entry_match(&entry, "github.com", "octocat@github.com");
        assert!(score >= 300);
    }
}
