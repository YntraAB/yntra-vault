use std::{
    cell::RefCell,
    io::{self, IsTerminal, Write},
    sync::{Arc, atomic::AtomicBool},
};
use yntra_vault_core::{
    Result, VaultError,
    services::sync::{
        review::{self, Choice, SyncDecision},
        review_transport::{self, Snapshot},
    },
    vault::VaultManager,
};

fn cancelled() -> VaultError {
    VaultError::SyncError("Synchronization cancelled".into())
}

fn ask(prompt: &str) -> Result<String> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut value = String::new();
    if io::stdin().read_line(&mut value)? == 0 {
        return Err(cancelled());
    }
    Ok(value.trim().to_owned())
}

pub fn run(manager: VaultManager, address: &str, listener: bool) -> Result<()> {
    let keys = manager.get_subkeys()?.clone();
    let salt = manager.salt();
    let trusted = manager.data.settings.trusted_devices.clone();
    let actor = manager.session_identity();
    let manager = RefCell::new(manager);
    let cancel = Arc::new(AtomicBool::new(false));
    let stored = std::cell::Cell::new(None);
    let commit = |expected: &str, data, operation, peer| {
        review_transport::apply_with_peer(&mut manager.borrow_mut(), expected, data, operation, Some(peer))?;
        stored.set(Some(operation));
        Ok(())
    };
    let stats = if listener {
        review_transport::listen(
            address,
            &keys,
            &salt,
            &trusted,
            cancel,
            |port| {
                println!("Listening on port {port}. Review the changes on the initiating device.")
            },
            || Snapshot::capture_with_trust(&manager.borrow(), &trusted),
            commit,
        )?
    } else {
        if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
            return Err(VaultError::SyncError(
                "An interactive terminal is required to review synchronization".into(),
            ));
        }
        let local = Snapshot::capture(&manager.borrow())?;
        review_transport::connect(
            address,
            &keys,
            &salt,
            &trusted,
            cancel,
            None,
            local,
            |local, remote| {
                review_changes(&local.data, &remote.data, &keys, actor)
            },
            |expected| {
                if Snapshot::capture(&manager.borrow())?.revision == expected {
                    Ok(())
                } else {
                    Err(VaultError::SyncError("Vault changed during review".into()))
                }
            },
            commit,
        )?
    };
    if let Some(operation) = stored.get() {
        review_transport::acknowledge_with_peer(&mut manager.borrow_mut(), operation, stats.peer_device_id)?;
    }
    println!(
        "Both devices confirmed storage. Added: {}, updated: {}.",
        stats.entries_added, stats.entries_updated
    );
    Ok(())
}


pub fn review_changes(local: &yntra_vault_core::vault::VaultData, remote: &yntra_vault_core::vault::VaultData, keys: &yntra_vault_core::crypto::SubKeys, actor: uuid::Uuid) -> Result<(uuid::Uuid, yntra_vault_core::vault::VaultData)> {
    require_terminal()?;
    let mut preview = review::preview(local, remote, keys)?;
    preview
        .changes
        .sort_by_key(|c| (c.entry_id, c.field != "encrypted_password"));
    let mut decisions = Vec::new();
    let mut duplicated = std::collections::BTreeSet::new();
    for change in &preview.changes {
        if duplicated.contains(&change.entry_id) {
            continue;
        }
        println!(
            "{} · {}{}",
            change.title,
            change.field,
            if change.conflict { " (conflict)" } else { "" }
        );
        println!(
            "Local: {:?}; remote: {:?}; proposed: {:?}",
            change.local_changed_at, change.remote_changed_at, change.suggested
        );
        loop {
            let answer = ask(
                "Local [l], remote [r], keep both [b], edit [e], reveal [v], accept proposal [Enter], cancel [q]: ",
            )?;
            let choice = match answer.as_str() {
                "l" => Choice::Local,
                "r" => Choice::Remote,
                "b" if change.can_keep_both => Choice::Both,
                "e" if change.can_edit => Choice::Edited,
                "v" => {
                    let a = zeroize::Zeroizing::new(review::inspect(
                        local,
                        change.entry_id,
                        &change.field,
                        keys,
                    )?);
                    let b = zeroize::Zeroizing::new(review::inspect(
                        remote,
                        change.entry_id,
                        &change.field,
                        keys,
                    )?);
                    println!("Local: {}\nRemote: {}", a.as_str(), b.as_str());
                    continue;
                }
                "" if change.suggested.is_some() => break,
                "q" => return Err(cancelled()),
                _ => continue,
            };
            let text = if choice == Choice::Edited {
                Some(ask("Result: ")?)
            } else {
                None
            };
            if choice == Choice::Both && change.field == "encrypted_password" {
                duplicated.insert(change.entry_id);
                if let Some(entries) = &change.keep_both_entries {
                    for entry in entries {
                        println!("{} ({})", entry.title, entry.id);
                    }
                }
            }
            decisions.push(SyncDecision {
                key: change.key.clone(),
                choice,
                text,
            });
            break;
        }
    }
    if !preview.changes.is_empty()
        && ask("Save this result on both devices? [yes/no]: ")? != "yes"
    {
        return Err(cancelled());
    }
    Ok((
        preview.id,
        review::resolve(
            local,
            remote,
            &preview,
            &decisions,
            keys,
            actor,
        )?,
    ))
}

pub fn require_terminal() -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(VaultError::SyncError("An interactive terminal is required to review synchronization".into()));
    }
    Ok(())
}
