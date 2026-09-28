//! HTTPS Git credential helper. Credentials require a unique, exact saved origin/account match.
use std::io::{BufRead, stdin};
use std::process::Command;
use colored::*;
use yntra_vault_core::{Result, VaultError, vault::EntryPreview};
use crate::ipc::{try_ipc_request, IpcRequest, IpcResponse};

pub async fn handle_git_credential(action: &str) -> Result<()> {
    match action {
        "get" => handle_git_get().await,
        "store" | "erase" => Ok(()),
        "setup" => handle_git_setup(),
        other => Err(VaultError::InvalidFormat(format!("Unknown git-credential action '{}'", other))),
    }
}

#[derive(Debug)]
struct CredentialRequest { origin: url::Url, username: String }

fn https_origin(value: &str) -> Option<url::Url> {
    if value.chars().any(|c| c.is_control() || c == '\\') { return None; }
    let url = url::Url::parse(value).ok()?;
    (url.scheme() == "https" && url.host_str().is_some() && url.username().is_empty()
        && url.password().is_none() && !value.contains('\\')).then_some(url)
}

fn read_request(mut input: impl BufRead) -> Option<CredentialRequest> {
    let mut protocol = None;
    let mut host = None;
    let mut full_url = None;
    let mut username = String::new();
    let mut username_seen = false;
    let mut total = 0;
    loop {
        let mut line = String::new();
        let n = std::io::Read::take(&mut input, (16 * 1024 + 1 - total) as u64).read_line(&mut line).ok()?;
        total += n;
        if total > 16 * 1024 || line.contains('\0') { return None; }
        let line = line.trim_end_matches(['\r', '\n']);
        if n == 0 || line.is_empty() { break; }
        let (key, value) = line.split_once('=')?;
        match key {
            "protocol" if protocol.is_none() => protocol = Some(value.to_string()),
            "host" if host.is_none() => host = Some(value.to_string()),
            "url" if full_url.is_none() => full_url = Some(value.to_string()),
            "username" if !username_seen => { username = value.to_string(); username_seen = true; },
            "protocol" | "host" | "url" | "username" => return None,
            _ => {},
        }
    }
    let origin = match (protocol, host) {
        (Some(protocol), Some(host)) if protocol == "https"
            && !host.is_empty() && !host.chars().any(|c| c.is_whitespace() || matches!(c,'/'|'\\'|'@'|'?'|'#')) => {
                https_origin(&format!("https://{host}/"))?
            },
        (None, None) => https_origin(full_url.as_deref()?)?,
        _ => return None,
    };
    if let Some(full_url) = full_url {
        if https_origin(&full_url)?.origin() != origin.origin() { return None; }
    }
    Some(CredentialRequest { origin, username })
}

fn select_entry(entries: &[EntryPreview], request: &CredentialRequest) -> Option<uuid::Uuid> {
    let mut matches = entries.iter().filter(|entry| {
        https_origin(&entry.url).is_some_and(|saved| saved.origin() == request.origin.origin())
            && (request.username.is_empty() || entry.username == request.username
                || (entry.username.is_empty() && entry.email == request.username))
    });
    let selected = matches.next()?;
    if matches.next().is_some() { return None; }
    Some(selected.id)
}

fn credential_response(username: &str, password: &str) -> Option<String> {
    if username.chars().chain(password.chars()).any(|c| matches!(c,'\r'|'\n'|'\0')) { return None; }
    Some(format!("username={username}\npassword={password}\n\n"))
}

async fn handle_git_get() -> Result<()> {
    let Some(request) = read_request(stdin().lock()) else { return Ok(()); };
    let entries = match try_ipc_request(&IpcRequest::ListEntries).await {
        Some(IpcResponse::ListEntries(list)) => list,
        _ => return Ok(()),
    };
    let Some(id) = select_entry(&entries, &request) else { return Ok(()); };
    if let Some(IpcResponse::GetEntry(entry)) = try_ipc_request(&IpcRequest::GetEntry { query: id.to_string() }).await {
        // Recheck after the asynchronous fetch in case the entry was edited meanwhile.
        let user = if entry.username.is_empty() { &entry.email } else { &entry.username };
        if !https_origin(&entry.url).is_some_and(|saved| saved.origin() == request.origin.origin())
            || (!request.username.is_empty() && user != &request.username) { return Ok(()); }
        if let Some(response) = credential_response(user, &entry.password) {
            let response = zeroize::Zeroizing::new(response);
            use std::io::Write;
            let mut out = std::io::stdout().lock();
            out.write_all(response.as_bytes())?;
            out.flush()?;
        }
    }
    Ok(())
}
fn handle_git_setup() -> Result<()> {
    let exe = std::env::current_exe()
        .map_err(|e| VaultError::InvalidFormat(format!("Failed to locate yntra executable: {}", e)))?;
    let helper_cmd = format!("\"{}\" git-credential", exe.display().to_string().replace('\\', "/"));

    let status = Command::new("git")
        .args(["config", "--global", "credential.helper", &helper_cmd])
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
    use yntra_vault_core::vault::types::{EntryType, BreachStatus};
    fn entry(user: &str,url: &str)->EntryPreview {
        EntryPreview { id:uuid::Uuid::new_v4(),title:"GitHub".into(),username:user.into(),url:url.into(),email:String::new(),entry_type:EntryType::Login,tags:vec![],favorite:false,pinned:false,has_totp:false,updated_at:chrono::Utc::now(),breach_status:BreachStatus::Unknown,strength_score:None,password_age_days:0,has_passkey:false,attachment_count:0 }
    }
    fn resolve(input:&str, entries:&[EntryPreview])->Option<uuid::Uuid> {select_entry(entries,&read_request(input.as_bytes())?)}
    #[test]
    fn complete_request_path_never_falls_back_to_title_search() {
        let entries=[entry("alice","https://github.com")];
        for host in ["github.example","github.com.evil.example","gist.github.com","github.com:8443"] {
            assert_eq!(resolve(&format!("protocol=https\nhost={host}\nusername=alice\n\n"),&entries),None);
        }
        assert_eq!(resolve("protocol=https\nhost=github.com\nusername=alice\n\n",&entries),Some(entries[0].id));
        assert_eq!(resolve("protocol=https\nhost=github.com\nusername=bob\n\n",&entries),None);
    }
    #[test]
    fn request_parser_rejects_downgrade_userinfo_conflicts_and_ambiguity() {
        let entries=[entry("alice","https://github.com")];
        for input in ["protocol=http\nhost=github.com\n\n","host=github.com\n\n","protocol=https\nhost=github.com:pass@evil.example\n\n","protocol=https\nhost=github.com\nurl=https://evil.example\n\n","url=https://user@github.com\n\n","protocol=https\nhost=github.com\nhost=evil.example\n\n"] { assert_eq!(resolve(input,&entries),None); }
        assert_eq!(resolve("url=https://github.com/a.git\n\n",&entries),Some(entries[0].id));
        let duplicate=[entry("alice","https://github.com"),entry("bob","https://github.com")];
        assert_eq!(resolve("protocol=https\nhost=github.com\n\n",&duplicate),None);
        assert_eq!(resolve("protocol=https\nhost=github.com\nusername=bob\n\n",&duplicate),Some(duplicate[1].id));
        assert_eq!(resolve("protocol=https\nhost=github.com\n\n",&[entry("alice","")]),None);
    }
    #[test]
    fn response_does_not_inject_git_protocol_fields() {
        assert!(credential_response("alice\npassword=evil","secret").is_none());
        assert!(credential_response("alice","secret\nurl=https://evil.example").is_none());
        assert_eq!(credential_response("alice","secret").unwrap(),"username=alice\npassword=secret\n\n");
    }
    #[test]
    fn request_parser_is_bounded_and_rejects_empty_duplicate_fields() {
        assert!(read_request(format!("host={}\n\n", "x".repeat(17 * 1024)).as_bytes()).is_none());
        assert!(read_request(b"protocol=https\nhost=github.com\nusername=\nusername=alice\n\n".as_slice()).is_none());
        assert!(read_request(b"url=https://git\thub.com\n\n".as_slice()).is_none());
        let entries = [entry("alice", "https://github.com")];
        assert_eq!(resolve("protocol=https\nhost=GITHUB.COM:443\n\n", &entries), Some(entries[0].id));
    }
}

