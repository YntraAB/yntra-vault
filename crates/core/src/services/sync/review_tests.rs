use super::review::*;
use crate::{crypto::*, vault::*};
use chrono::Utc;
use uuid::Uuid;

pub(super) fn fixture() -> (VaultData, SubKeys) {
    let keys = SubKeys::from_bytes(&[7; 160]).unwrap();
    let id = Uuid::new_v4();
    let now = Utc::now();
    let item = Entry {
        id,
        title: "Gmail".into(),
        username: "user".into(),
        email: "user@example.test".into(),
        url: "https://example.test".into(),
        notes: String::new(),
        tags: vec![],
        favorite: false,
        pinned: false,
        encrypted_password: VaultManager::encrypt_entry_field(
            b"original",
            &keys.entry_key,
            &id,
            FieldScope::Password,
        )
        .unwrap(),
        encrypted_totp_secret: None,
        custom_fields: vec![],
        entry_type: EntryType::Login,
        created_at: now,
        updated_at: now,
        password_history: vec![],
        breach_status: Default::default(),
        strength_score: None,
        password_changed_at: now,
        encrypted_passkey: None,
        passkey_public_key: None,
        attachments: vec![],
    };
    let mut data = VaultData {
        metadata: VaultMetadata {
            id: Uuid::new_v4(),
            name: "Fixture".into(),
            created_at: now,
            updated_at: now,
            entry_count: 1,
            version: 6,
        },
        entries: vec![item],
        tags: vec![],
        trash: vec![],
        settings: Default::default(),
    };
    track(&mut data, &keys, Uuid::new_v4(), now).unwrap();
    (data, keys)
}

fn save(data: &mut VaultData, keys: &SubKeys) {
    track(data, keys, Uuid::new_v4(), Utc::now()).unwrap();
}

#[test]
fn tag_default_and_ordered_credentials_merge_and_custom_passwords_keep_coherent_accounts() {
    let (mut local, keys) = fixture();
    let password_id = Uuid::new_v4();
    let layout_id = Uuid::new_v4();
    local.tags.push(Tag { id: Uuid::new_v4(), name: "Mail".into(), color: "blue".into(), icon: "tag".into(), default_url: String::new() });
    local.entries[0].custom_fields = vec![
        CustomField { id: password_id, name: "Other password".into(), field_type: FieldType::Password, value: "initial".into(), sensitive: true },
        CustomField { id: layout_id, name: "_field_order".into(), field_type: FieldType::Text, value: "username,password".into(), sensitive: false },
    ];
    save(&mut local, &keys);
    let mut remote = local.clone();
    local.tags[0].default_url = "https://gmail.com/".into();
    local.entries[0].custom_fields[1].value = format!("username,{password_id},password");
    remote.entries[0].custom_fields[0].value = "remote edit".into();
    save(&mut local, &keys); save(&mut remote, &keys);
    let proposal = preview(&local, &remote, &keys).unwrap();
    assert!(proposal.changes.iter().all(|change| !change.conflict));
    let merged = resolve(&local, &remote, &proposal, &[], &keys, Uuid::new_v4()).unwrap();
    assert_eq!(merged.tags[0].default_url, "https://gmail.com/");
    assert_eq!(merged.entries[0].custom_fields.iter().find(|field| field.id == password_id).unwrap().value, "remote edit");
    assert_eq!(merged.entries[0].custom_fields.iter().find(|field| field.id == layout_id).unwrap().value, format!("username,{password_id},password"));
    local = merged.clone(); remote = merged;
    local.entries[0].custom_fields.iter_mut().find(|field| field.id == password_id).unwrap().value = "left".into();
    remote.entries[0].custom_fields.iter_mut().find(|field| field.id == password_id).unwrap().value = "right".into();
    local.entries[0].username = "left-user".into(); remote.entries[0].username = "right-user".into();
    save(&mut local, &keys); save(&mut remote, &keys);
    let proposal = preview(&local, &remote, &keys).unwrap();
    let password = proposal.changes.iter().find(|change| change.field == format!("custom/{password_id}")).unwrap();
    assert!(password.conflict && password.can_keep_both && password.keep_both_entries.is_some());
    let both = resolve(&local, &remote, &proposal, &[SyncDecision { key: password.key.clone(), choice: Choice::Both, text: None }], &keys, Uuid::new_v4()).unwrap();
    assert_eq!(both.entries.len(), 2);
    for (username, value) in [("left-user", "left"), ("right-user", "right")] {
        let item = both.entries.iter().find(|entry| entry.username == username).unwrap();
        assert!(item.custom_fields.iter().any(|field| field.value == value));
        assert_eq!(inspect(&both, item.id, "encrypted_password", &keys).unwrap(), "original");
    }
    assert!(preview(&both, &both, &keys).unwrap().changes.is_empty());
}

#[test]
fn independent_password_and_notes_survive_in_both_directions() {
    let (mut desktop, keys) = fixture();
    let mut phone = desktop.clone();
    desktop.entries[0].notes = "Discord backup instructions".into();
    save(&mut desktop, &keys);
    let id = phone.entries[0].id;
    phone.entries[0].encrypted_password = VaultManager::encrypt_entry_field(
        b"new password",
        &keys.entry_key,
        &id,
        FieldScope::Password,
    )
    .unwrap();
    save(&mut phone, &keys);
    let proposal = preview(&desktop, &phone, &keys).unwrap();
    assert_eq!(proposal.changes.len(), 2);
    assert!(proposal.changes.iter().all(|c| !c.conflict));
    let merged = resolve(&desktop, &phone, &proposal, &[], &keys, Uuid::new_v4()).unwrap();
    assert_eq!(merged.entries[0].notes, "Discord backup instructions");
    assert_eq!(
        inspect(&merged, id, "encrypted_password", &keys).unwrap(),
        "new password"
    );
    assert!(preview(&merged, &merged, &keys).unwrap().changes.is_empty());
}

#[test]
fn concurrent_notes_require_a_choice_and_keep_both_without_concatenation() {
    let (mut a, keys) = fixture();
    let mut b = a.clone();
    a.entries[0].notes = "desktop".into();
    b.entries[0].notes = "phone".into();
    save(&mut a, &keys);
    save(&mut b, &keys);
    let proposal = preview(&a, &b, &keys).unwrap();
    assert!(proposal.changes[0].conflict);
    assert!(resolve(&a, &b, &proposal, &[], &keys, Uuid::new_v4()).is_err());
    let choice = SyncDecision {
        key: proposal.changes[0].key.clone(),
        choice: Choice::Both,
        text: None,
    };
    let merged = resolve(&a, &b, &proposal, &[choice], &keys, Uuid::new_v4()).unwrap();
    assert_eq!(merged.entries[0].notes, "desktop");
    assert_eq!(merged.entries[0].custom_fields[0].value, "phone");
    assert!(
        preview(&merged, &b, &keys)
            .unwrap()
            .changes
            .iter()
            .all(|c| !c.conflict)
    );
}

#[test]
fn password_keep_both_reencrypts_the_new_identity() {
    let (mut a, keys) = fixture();
    let mut b = a.clone();
    let id = a.entries[0].id;
    a.entries[0].encrypted_password =
        VaultManager::encrypt_entry_field(b"left", &keys.entry_key, &id, FieldScope::Password)
            .unwrap();
    b.entries[0].encrypted_password =
        VaultManager::encrypt_entry_field(b"right", &keys.entry_key, &id, FieldScope::Password)
            .unwrap();
    save(&mut a, &keys);
    save(&mut b, &keys);
    let proposal = preview(&a, &b, &keys).unwrap();
    let decision = SyncDecision {
        key: proposal.changes[0].key.clone(),
        choice: Choice::Both,
        text: None,
    };
    let merged = resolve(&a, &b, &proposal, &[decision], &keys, Uuid::new_v4()).unwrap();
    assert_eq!(merged.entries.len(), 2);
    assert_ne!(merged.entries[0].id, merged.entries[1].id);
    assert_eq!(
        inspect(&merged, merged.entries[1].id, "encrypted_password", &keys).unwrap(),
        "right"
    );
    assert!(
        VaultManager::decrypt_entry_field(
            &merged.entries[1].encrypted_password,
            &keys.entry_key,
            &id,
            FieldScope::Password
        )
        .is_err()
    );
}

#[test]
fn keep_both_preserves_accounts_and_the_exact_preview_identities() {
    let (mut a, keys) = fixture();
    let mut b = a.clone();
    let id = a.entries[0].id;
    a.entries[0].username = "desktop-account".into();
    a.entries[0].notes = "desktop note".into();
    b.entries[0].username = "phone-account".into();
    b.entries[0].email = "phone@example.test".into();
    a.entries[0].encrypted_password =
        VaultManager::encrypt_entry_field(b"left", &keys.entry_key, &id, FieldScope::Password)
            .unwrap();
    b.entries[0].encrypted_password =
        VaultManager::encrypt_entry_field(b"right", &keys.entry_key, &id, FieldScope::Password)
            .unwrap();
    save(&mut a, &keys);
    save(&mut b, &keys);
    let p = preview(&a, &b, &keys).unwrap();
    let password = p
        .changes
        .iter()
        .find(|c| c.field == "encrypted_password")
        .unwrap();
    let decisions = [SyncDecision {
        key: password.key.clone(),
        choice: Choice::Both,
        text: None,
    }];
    let expected = password.keep_both_entries.as_ref().unwrap();
    for _ in 0..2 {
        let result = resolve(&a, &b, &p, &decisions, &keys, Uuid::new_v4()).unwrap();
        assert_eq!(result.entries.len(), 2);
        for (actual, expected) in result.entries.iter().zip(expected) {
            assert_eq!(actual.id, expected.id);
            assert_eq!(actual.title, expected.title);
        }
        assert_eq!(result.entries[0].username, "desktop-account");
        assert_eq!(result.entries[0].notes, "desktop note");
        assert_eq!(result.entries[0].email, a.entries[0].email);
        assert_eq!(result.entries[1].username, "phone-account");
        assert_eq!(result.entries[1].email, "phone@example.test");
        assert_eq!(result.entries[1].notes, b.entries[0].notes);
        assert_eq!(
            inspect(&result, expected[0].id, "encrypted_password", &keys).unwrap(),
            "left"
        );
        assert_eq!(
            inspect(&result, expected[1].id, "encrypted_password", &keys).unwrap(),
            "right"
        );
        let retry = preview(&result, &b, &keys).unwrap();
        assert!(retry.changes.iter().all(|c| !c.conflict));
        let retried = resolve(&result, &b, &retry, &[], &keys, Uuid::new_v4()).unwrap();
        assert_eq!(retried.entries.len(), 2);
    }
}

#[test]
fn unsupported_metadata_and_oversized_edited_titles_are_rejected() {
    let (mut a, keys) = fixture();
    let mut b = a.clone();
    b.entries[0].title = "New title".into();
    save(&mut b, &keys);
    let p = preview(&a, &b, &keys).unwrap();
    assert!(
        resolve(
            &a,
            &b,
            &p,
            &[SyncDecision {
                key: p.changes[0].key.clone(),
                choice: Choice::Edited,
                text: Some("x".repeat(129))
            }],
            &keys,
            Uuid::new_v4()
        )
        .is_err()
    );
    a.settings.sync.version = u8::MAX;
    assert!(track(&mut a, &keys, Uuid::new_v4(), Utc::now()).is_err());
    assert!(preview(&a, &b, &keys).is_err());
    assert_eq!(a.settings.sync.version, u8::MAX);
}

#[test]
fn ciphertext_randomness_does_not_create_a_change() {
    let (a, keys) = fixture();
    let mut b = a.clone();
    let id = b.entries[0].id;
    b.entries[0].encrypted_password =
        VaultManager::encrypt_entry_field(b"original", &keys.entry_key, &id, FieldScope::Password)
            .unwrap();
    save(&mut b, &keys);
    assert!(preview(&a, &b, &keys).unwrap().changes.is_empty());
}

#[test]
fn permanent_deletion_survives_old_offline_replica() {
    let (mut a, keys) = fixture();
    let old = a.clone();
    let id = a.entries[0].id;
    a.entries.clear();
    save(&mut a, &keys);
    let proposal = preview(&a, &old, &keys).unwrap();
    assert_eq!(proposal.changes.len(), 1);
    assert_eq!(proposal.changes[0].suggested, Some(Choice::Local));
    let merged = resolve(&a, &old, &proposal, &[], &keys, Uuid::new_v4()).unwrap();
    assert!(merged.entries.is_empty());
    assert!(
        merged
            .settings
            .sync
            .fields
            .contains_key(&format!("{id}/state"))
    );
    assert!(!merged.settings.sync.history.contains_key(&id));
}

#[test]
fn concurrent_delete_and_edit_requires_explicit_resolution() {
    let (mut a, keys) = fixture();
    let mut b = a.clone();
    a.entries.clear();
    b.entries[0].notes = "offline edit".into();
    save(&mut a, &keys);
    save(&mut b, &keys);
    let proposal = preview(&a, &b, &keys).unwrap();
    assert_eq!(proposal.changes.len(), 1);
    assert!(proposal.changes[0].conflict);
}

#[test]
fn intentional_empty_field_does_not_restore_old_notes() {
    let (mut a, keys) = fixture();
    a.entries[0].notes = "old".into();
    save(&mut a, &keys);
    let old = a.clone();
    a.entries[0].notes.clear();
    save(&mut a, &keys);
    let p = preview(&old, &a, &keys).unwrap();
    let merged = resolve(&old, &a, &p, &[], &keys, Uuid::new_v4()).unwrap();
    assert!(merged.entries[0].notes.is_empty());
}

#[test]
fn conflict_history_is_bounded_rekeyed_and_removed_with_deleted_content() {
    let (mut a, keys) = fixture();
    let id = a.entries[0].id;
    for i in 0..7 {
        let mut b = a.clone();
        a.entries[0].notes = format!("desktop {i}");
        b.entries[0].notes = format!("phone {i}");
        save(&mut a, &keys);
        save(&mut b, &keys);
        let p = preview(&a, &b, &keys).unwrap();
        let decision = SyncDecision {
            key: p.changes[0].key.clone(),
            choice: Choice::Local,
            text: None,
        };
        a = resolve(&a, &b, &p, &[decision], &keys, Uuid::new_v4()).unwrap();
    }
    assert_eq!(a.settings.sync.history[&id].len(), 10);
    let new_keys = SubKeys::from_bytes(&[8; 160]).unwrap();
    VaultManager::reencrypt_sync_history(&mut a, &keys.entry_key, &new_keys.entry_key).unwrap();
    for version in &a.settings.sync.history[&id] {
        assert!(
            VaultManager::decrypt_entry_field(
                &version.encrypted_entry,
                &keys.entry_key,
                &id,
                "sync-history"
            )
            .is_err()
        );
        let bytes = VaultManager::decrypt_entry_field(
            &version.encrypted_entry,
            &new_keys.entry_key,
            &id,
            "sync-history",
        )
        .unwrap();
        let entry: Entry = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(
            &*VaultManager::decrypt_entry_field(
                &entry.encrypted_password,
                &new_keys.entry_key,
                &id,
                "password"
            )
            .unwrap(),
            b"original"
        );
    }
    a.entries.clear();
    save(&mut a, &keys);
    let mut b = a.clone();
    b.settings.sync.history.clear();
    let p = preview(&a, &b, &keys).unwrap();
    assert!(p.changes.is_empty());
    let merged = resolve(
        &b,
        &a,
        &preview(&b, &a, &keys).unwrap(),
        &[],
        &keys,
        Uuid::new_v4(),
    )
    .unwrap();
    assert!(merged.settings.sync.history.is_empty());
}

#[test]
fn causal_history_retention_survives_a_clock_far_in_the_future() {
    let (mut current, keys) = fixture();
    let id = current.entries[0].id;
    for index in 0..8 {
        let mut other = current.clone();
        current.entries[0].notes = format!("desktop {index}");
        other.entries[0].notes = format!("phone {index}");
        save(&mut current, &keys);
        save(&mut other, &keys);
        let preview = preview(&current, &other, &keys).unwrap();
        let decisions = preview.changes.iter().map(|change| SyncDecision {
            key: change.key.clone(), choice: Choice::Local, text: None,
        }).collect::<Vec<_>>();
        current = resolve(&current, &other, &preview, &decisions, &keys, Uuid::new_v4()).unwrap();
        if index == 4 {
            for version in current.settings.sync.history.get_mut(&id).unwrap() {
                version.saved_at = Utc::now() + chrono::Duration::days(3650);
            }
        }
    }
    let versions = &current.settings.sync.history[&id];
    assert_eq!(versions.len(), 10);
    let notes = versions.iter().map(|version| {
        let bytes = VaultManager::decrypt_entry_field(&version.encrypted_entry, &keys.entry_key, &id, "sync-history").unwrap();
        rmp_serde::from_slice::<Entry>(&bytes).unwrap().notes.clone()
    }).collect::<Vec<_>>();
    assert!(notes.contains(&"desktop 7".to_string()));
    assert!(notes.contains(&"phone 7".to_string()));
    assert!(!notes.contains(&"desktop 0".to_string()));
}

#[test]
fn stale_preview_and_unknown_decisions_are_rejected() {
    let (mut a, keys) = fixture();
    let b = a.clone();
    let p = preview(&a, &b, &keys).unwrap();
    a.entries[0].notes = "changed during review".into();
    assert!(resolve(&a, &b, &p, &[], &keys, Uuid::new_v4()).is_err());
    let p = preview(&a, &b, &keys).unwrap();
    assert!(
        resolve(
            &a,
            &b,
            &p,
            &[SyncDecision {
                key: "unknown".into(),
                choice: Choice::Remote,
                text: None
            }],
            &keys,
            Uuid::new_v4()
        )
        .is_err()
    );
}

#[test]
fn equal_timestamps_and_missing_ancestry_remain_conflicts() {
    let (mut a, keys) = fixture();
    let mut b = a.clone();
    a.settings.sync = Default::default();
    b.settings.sync = Default::default();
    a.entries[0].notes = "a".into();
    b.entries[0].notes = "b".into();
    let proposal = preview(&a, &b, &keys).unwrap();
    assert!(proposal.changes[0].conflict);
}

#[test]
fn tracking_noop_does_not_advance_revisions() {
    let (mut a, keys) = fixture();
    let before = a.settings.sync.clone();
    save(&mut a, &keys);
    assert_eq!(before, a.settings.sync);
}

#[test]
fn vault_name_and_tag_order_participate_in_review() {
    let (mut a, keys) = fixture();
    a.tags = vec![
        Tag {
            id: Uuid::new_v4(),
            name: "A".into(),
            color: String::new(),
            icon: String::new(), default_url: String::new(),
        },
        Tag {
            id: Uuid::new_v4(),
            name: "B".into(),
            color: String::new(),
            icon: String::new(), default_url: String::new(),
        },
    ];
    save(&mut a, &keys);
    let mut b = a.clone();
    b.tags.reverse();
    b.metadata.name = "Renamed vault".into();
    save(&mut b, &keys);
    let p = preview(&a, &b, &keys).unwrap();
    assert_eq!(p.changes.len(), 2);
    assert!(p.changes.iter().all(|c| !c.conflict));
    let result = resolve(&a, &b, &p, &[], &keys, Uuid::new_v4()).unwrap();
    assert_eq!(result.metadata.name, b.metadata.name);
    assert_eq!(result.tags[0].id, b.tags[0].id);
}

#[tokio::test]
async fn webdav_rejects_a_server_change_after_review_without_overwriting_it() {
    use super::review_transport;
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
    };
    fn request(stream: &mut TcpStream) -> String {
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut data = Vec::new();
        let mut buffer = [0; 4096];
        loop {
            let count = stream.read(&mut buffer).unwrap();
            assert!(count > 0);
            data.extend_from_slice(&buffer[..count]);
            if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                let header = String::from_utf8(data[..end].to_vec()).unwrap();
                let size: usize = header
                    .lines()
                    .find_map(|line| {
                        line.to_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse().unwrap())
                    })
                    .unwrap_or(0);
                if data.len() >= end + 4 + size {
                    return header;
                }
            }
        }
    }
    let (local, keys) = fixture();
    let salt = [9; 32];
    let old = review_transport::encode_vault(&local, &keys, salt).unwrap();
    let mut changed = local.clone();
    changed.entries[0].notes = "concurrent server update".into();
    save(&mut changed, &keys);
    let new = review_transport::encode_vault(&changed, &keys, salt).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/vault.vdb", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut get, _) = listener.accept().unwrap();
        assert!(request(&mut get).starts_with("GET "));
        write!(
            get,
            "HTTP/1.1 200 OK\r\nETag: \"v1\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            old.len()
        )
        .unwrap();
        get.write_all(&old).unwrap();
        drop(get);
        let (mut put, _) = listener.accept().unwrap();
        let header = request(&mut put);
        assert!(header.starts_with("PUT "));
        assert!(header.to_lowercase().contains("if-match: \"v1\""));
        put.write_all(
            b"HTTP/1.1 412 Precondition Failed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        )
        .unwrap();
        new
    });
    let (bytes, etag) = super::webdav_download_snapshot(&url, "", None)
        .await
        .unwrap()
        .unwrap();
    let remote = super::decrypt_remote_vault_bytes_checked(&bytes, &keys, Some(&salt)).unwrap();
    let p = preview(&local, &remote, &keys).unwrap();
    let result = resolve(&local, &remote, &p, &[], &keys, Uuid::new_v4()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("proposal.vdb");
    std::fs::write(
        &path,
        review_transport::encode_vault(&result, &keys, salt).unwrap(),
    )
    .unwrap();
    let error = super::webdav_upload_conditional(&url, "", None, &path, Some(&etag), false)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("412"));
    let preserved =
        super::decrypt_remote_vault_bytes_checked(&server.join().unwrap(), &keys, Some(&salt))
            .unwrap();
    assert_eq!(preserved.entries[0].notes, "concurrent server update");
    assert!(local.entries[0].notes.is_empty());
}

#[test]
fn deletion_marker_propagates_to_a_replica_that_never_had_the_entry() {
    let (mut deleted, keys) = fixture();
    let old = deleted.clone();
    let mut empty = deleted.clone();
    empty.entries.clear();
    empty.settings.sync = Default::default();
    save(&mut empty, &keys);
    deleted.entries.clear();
    save(&mut deleted, &keys);
    let p = preview(&empty, &deleted, &keys).unwrap();
    let learned = resolve(&empty, &deleted, &p, &[], &keys, Uuid::new_v4()).unwrap();
    let p = preview(&learned, &old, &keys).unwrap();
    assert_eq!(p.changes[0].suggested, Some(Choice::Local));
    assert!(
        resolve(&learned, &old, &p, &[], &keys, Uuid::new_v4())
            .unwrap()
            .entries
            .is_empty()
    );
}

#[test]
fn normal_transfer_preserves_the_field_edit_time() {
    let (a, keys) = fixture();
    let mut b = a.clone();
    b.entries[0].notes = "changed".into();
    save(&mut b, &keys);
    let field = format!("{}/notes", a.entries[0].id);
    let original = b.settings.sync.fields[&field].clone();
    let p = preview(&a, &b, &keys).unwrap();
    let result = resolve(&a, &b, &p, &[], &keys, Uuid::new_v4()).unwrap();
    assert_eq!(result.settings.sync.fields[&field], original);
}

#[test]
fn backwards_clock_is_not_eligible_for_automatic_latest_policy() {
    let (mut a, keys) = fixture();
    let mut b = a.clone();
    a.entries[0].notes = "a".into();
    b.entries[0].notes = "b".into();
    save(&mut a, &keys);
    track(
        &mut b,
        &keys,
        Uuid::new_v4(),
        Utc::now() - chrono::Duration::days(1),
    )
    .unwrap();
    let p = preview(&a, &b, &keys).unwrap();
    assert!(p.changes[0].conflict);
    assert!(!p.changes[0].time_order_reliable);
}

#[test]
fn edited_result_supersedes_both_and_retains_encrypted_conflict_history() {
    let (mut a, keys) = fixture();
    let mut b = a.clone();
    a.entries[0].notes = "a".into();
    b.entries[0].notes = "b".into();
    save(&mut a, &keys);
    save(&mut b, &keys);
    let p = preview(&a, &b, &keys).unwrap();
    let result = resolve(
        &a,
        &b,
        &p,
        &[SyncDecision {
            key: p.changes[0].key.clone(),
            choice: Choice::Edited,
            text: Some("combined by user".into()),
        }],
        &keys,
        Uuid::new_v4(),
    )
    .unwrap();
    let id = result.entries[0].id;
    assert_eq!(result.entries[0].notes, "combined by user");
    assert_eq!(result.settings.sync.history[&id].len(), 2);
    for source in [&a, &b] {
        let p = preview(&result, source, &keys).unwrap();
        assert!(p.changes.iter().all(|c| c.suggested == Some(Choice::Local)));
    }
    let notes: Vec<_> = result.settings.sync.history[&id]
        .iter()
        .map(|version| {
            let bytes = VaultManager::decrypt_entry_field(
                &version.encrypted_entry,
                &keys.entry_key,
                &id,
                "sync-history",
            )
            .unwrap();
            rmp_serde::from_slice::<Entry>(&bytes)
                .unwrap()
                .notes
                .clone()
        })
        .collect();
    assert!(notes.contains(&"a".to_owned()));
    assert!(notes.contains(&"b".to_owned()));
}

#[test]
fn removed_custom_fields_and_attachments_do_not_return_from_old_copies() {
    let (mut a, keys) = fixture();
    let id = a.entries[0].id;
    let attachment = Uuid::new_v4();
    a.entries[0].custom_fields.push(CustomField {
        id: Uuid::new_v4(),
        name: "Note".into(),
        value: "old".into(),
        field_type: FieldType::Notes,
        sensitive: true,
    });
    a.entries[0].attachments.push(FileAttachment {
        id: attachment,
        name: "fixture.txt".into(),
        size: 3,
        mime_type: "text/plain".into(),
        created_at: Utc::now(),
        encrypted_blob: VaultManager::encrypt_entry_field(
            b"old",
            &keys.entry_key,
            &id,
            FieldScope::Attachment {
                attachment_id: &attachment,
            },
        )
        .unwrap(),
    });
    save(&mut a, &keys);
    let old = a.clone();
    a.entries[0].custom_fields.clear();
    a.entries[0].attachments.clear();
    save(&mut a, &keys);
    let p = preview(&a, &old, &keys).unwrap();
    assert_eq!(p.changes.len(), 2);
    assert!(p.changes.iter().all(|c| c.suggested == Some(Choice::Local)));
    let result = resolve(&a, &old, &p, &[], &keys, Uuid::new_v4()).unwrap();
    assert!(result.entries[0].custom_fields.is_empty());
    assert!(result.entries[0].attachments.is_empty());
}

#[test]
fn identical_names_do_not_merge_different_entries() {
    let (a, keys) = fixture();
    let (other, _) = fixture();
    let mut b = a.clone();
    b.entries.push(other.entries[0].clone());
    save(&mut b, &keys);
    let p = preview(&a, &b, &keys).unwrap();
    let result = resolve(&a, &b, &p, &[], &keys, Uuid::new_v4()).unwrap();
    assert_eq!(result.entries.len(), 2);
    assert_ne!(result.entries[0].id, result.entries[1].id);
}

#[test]
fn interrupted_p2p_retries_preserve_the_approved_duplicate() {
    use super::{identity, resolve_local_device_info, review_transport as transport};
    use std::sync::{Arc, Mutex, atomic::AtomicBool, mpsc};
    let (mut a, keys) = fixture();
    let mut b = a.clone();
    let id = a.entries[0].id;
    a.entries[0].encrypted_password =
        VaultManager::encrypt_entry_field(b"server", &keys.entry_key, &id, FieldScope::Password)
            .unwrap();
    save(&mut a, &keys);
    b.entries[0].encrypted_password =
        VaultManager::encrypt_entry_field(b"client", &keys.entry_key, &id, FieldScope::Password)
            .unwrap();
    save(&mut b, &keys);
    let trusted = vec![TrustedDevice {
        id: resolve_local_device_info(None).id,
        signing_public_key: identity::public_key().unwrap(),
        ..Default::default()
    }];
    let server = Arc::new(Mutex::new(a));
    let client = Arc::new(Mutex::new(b));
    for attempt in 0..2 {
        let (ready_tx, ready_rx) = mpsc::channel();
        let (host_data, host_keys, host_trust) = (server.clone(), keys.clone(), trusted.clone());
        let host = std::thread::spawn(move || {
            transport::listen(
                "127.0.0.1:0",
                &host_keys,
                &[8; 32],
                &host_trust,
                Arc::new(AtomicBool::new(false)),
                |port| ready_tx.send(port).unwrap(),
                || {
                    let data = host_data.lock().unwrap().clone();
                    Ok(transport::Snapshot {
                        revision: revision(&data)?,
                        data,
                        observed_at: Utc::now(),
                    })
                },
                |_, data, _, _| {
                    *host_data.lock().unwrap() = data;
                    Ok(())
                },
            )
        });
        let port = ready_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let data = client.lock().unwrap().clone();
        let local = transport::Snapshot {
            revision: revision(&data).unwrap(),
            data,
            observed_at: Utc::now(),
        };
        let result = transport::connect(
            &format!("127.0.0.1:{port}"),
            &keys,
            &[8; 32],
            &trusted,
            Arc::new(AtomicBool::new(false)),
            None,
            local,
            |a, b| {
                let p = preview(&a.data, &b.data, &keys)?;
                let decisions = if attempt == 0 {
                    p.changes
                        .iter()
                        .filter(|c| c.conflict)
                        .map(|c| SyncDecision {
                            key: c.key.clone(),
                            choice: Choice::Both,
                            text: None,
                        })
                        .collect()
                } else {
                    assert!(p.changes.iter().all(|c| !c.conflict));
                    vec![]
                };
                Ok((
                    p.id,
                    resolve(&a.data, &b.data, &p, &decisions, &keys, Uuid::new_v4())?,
                ))
            },
            |_| Ok(()),
            |_, data, _, _| {
                if attempt == 0 {
                    return Err(crate::VaultError::SyncError("injected disk full".into()));
                }
                *client.lock().unwrap() = data;
                Ok(())
            },
        );
        let received = host.join().unwrap();
        if attempt == 0 {
            assert!(result.is_err());
            assert!(received.is_err());
            assert_eq!(server.lock().unwrap().entries.len(), 2);
            assert_eq!(client.lock().unwrap().entries.len(), 1);
        } else {
            result.unwrap();
            received.unwrap();
        }
    }
    assert_eq!(server.lock().unwrap().entries.len(), 2);
    assert_eq!(client.lock().unwrap().entries.len(), 2);
    assert_eq!(
        revision(&server.lock().unwrap()).unwrap(),
        revision(&client.lock().unwrap()).unwrap()
    );
}

#[test]
fn interrupted_protocol_stages_retry_without_repeating_committed_conflicts() {
    use super::{identity, resolve_local_device_info, review_transport as transport};
    use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}, mpsc};
    for fault in 0..7 {
        let (mut desktop, keys) = fixture();
        let mut phone = desktop.clone();
        let id = desktop.entries[0].id;
        for (data, password) in [(&mut desktop, b"desktop".as_slice()), (&mut phone, b"phone".as_slice())] {
            data.entries[0].encrypted_password = VaultManager::encrypt_entry_field(password, &keys.entry_key, &id, FieldScope::Password).unwrap();
            save(data, &keys);
        }
        let server = Arc::new(Mutex::new(desktop));
        let client = Arc::new(Mutex::new(phone));
        let trusted = vec![TrustedDevice { id: resolve_local_device_info(None).id, signing_public_key: identity::public_key().unwrap(), ..Default::default() }];
        for attempt in 0..2 {
            let (ready_tx, ready_rx) = mpsc::channel();
            let host_cancel = Arc::new(AtomicBool::new(false));
            let client_cancel = Arc::new(AtomicBool::new(false));
            let (state, host_keys, host_trust, cancel) = (server.clone(), keys.clone(), trusted.clone(), host_cancel.clone());
            let host = std::thread::spawn(move || transport::listen(
                "127.0.0.1:0", &host_keys, &[9;32], &host_trust, cancel.clone(),
                |port| ready_tx.send(port).unwrap(),
                || { let data = state.lock().unwrap().clone(); Ok(transport::Snapshot { revision: revision(&data)?, data, observed_at: Utc::now() }) },
                |expected, data, _, _| {
                    let mut current = state.lock().unwrap();
                    assert_eq!(revision(&current).unwrap(), expected);
                    if attempt == 0 && fault == 3 { return Err(crate::VaultError::SyncError("injected host storage failure".into())); }
                    *current = data;
                    if attempt == 0 && fault == 4 { cancel.store(true, Ordering::Release); }
                    Ok(())
                },
            ));
            let port = ready_rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
            let data = client.lock().unwrap().clone();
            let snapshot = transport::Snapshot { revision: revision(&data).unwrap(), data, observed_at: Utc::now() };
            let validations = std::cell::Cell::new(0);
            let outcome = transport::connect(
                &format!("127.0.0.1:{port}"), &keys, &[9;32], &trusted, client_cancel.clone(), None, snapshot,
                |local, remote| {
                    if attempt == 0 && fault == 0 { return Err(crate::VaultError::SyncError("injected review cancellation".into())); }
                    let proposal = preview(&local.data, &remote.data, &keys)?;
                    if attempt == 1 && fault >= 4 { assert!(proposal.changes.iter().all(|change| !change.conflict), "fault stage {fault}"); }
                    let decisions = proposal.changes.iter().filter(|change| change.conflict).map(|change| SyncDecision { key: change.key.clone(), choice: Choice::Both, text: None }).collect::<Vec<_>>();
                    Ok((proposal.id, resolve(&local.data, &remote.data, &proposal, &decisions, &keys, Uuid::new_v4())?))
                },
                |expected| {
                    validations.set(validations.get() + 1);
                    assert_eq!(revision(&client.lock().unwrap()).unwrap(), expected);
                    if attempt == 0 && fault == validations.get() && fault <= 2 { return Err(crate::VaultError::SyncError("injected local revision rejection".into())); }
                    Ok(())
                },
                |_, data, _, _| {
                    if attempt == 0 && fault == 5 { return Err(crate::VaultError::SyncError("injected client storage failure".into())); }
                    *client.lock().unwrap() = data;
                    if attempt == 0 && fault == 6 { client_cancel.store(true, Ordering::Release); }
                    Ok(())
                },
            );
            let host_outcome = host.join().unwrap();
            if attempt == 0 {
                assert!(outcome.is_err(), "fault stage {fault}");
                assert!(host_outcome.is_err(), "fault stage {fault}");
                assert_eq!(server.lock().unwrap().entries.len(), if fault >= 4 { 2 } else { 1 });
                assert_eq!(client.lock().unwrap().entries.len(), if fault == 6 { 2 } else { 1 });
                // Retry from serialized state, without the prior review or decisions in memory.
                for state in [&server, &client] {
                    let bytes = zeroize::Zeroizing::new(rmp_serde::to_vec(&*state.lock().unwrap()).unwrap());
                    *state.lock().unwrap() = rmp_serde::from_slice(&bytes).unwrap();
                }
            } else {
                outcome.unwrap(); host_outcome.unwrap();
                assert_eq!(server.lock().unwrap().entries.len(), 2);
                assert_eq!(client.lock().unwrap().entries.len(), 2);
                assert_eq!(revision(&server.lock().unwrap()).unwrap(), revision(&client.lock().unwrap()).unwrap());
            }
        }
    }
}

#[test]
fn local_trust_and_credentials_never_come_from_remote() {
    let (a, keys) = fixture();
    let mut b = a.clone();
    b.settings.webdav.username = "remote".into();
    let proposal = preview(&a, &b, &keys).unwrap();
    let merged = resolve(&a, &b, &proposal, &[], &keys, Uuid::new_v4()).unwrap();
    assert_eq!(merged.settings.webdav.username, a.settings.webdav.username);
}

#[test]
fn three_devices_keep_independent_changes_without_false_conflicts() {
    let (mut a, keys) = fixture();
    let mut b = a.clone();
    let mut c = a.clone();
    a.entries[0].notes = "A".into();
    save(&mut a, &keys);
    b.entries[0].email = "B@example.test".into();
    save(&mut b, &keys);
    c.entries[0].url = "https://c.example.test".into();
    save(&mut c, &keys);
    let p = preview(&a, &b, &keys).unwrap();
    let ab = resolve(&a, &b, &p, &[], &keys, Uuid::new_v4()).unwrap();
    let p = preview(&ab, &c, &keys).unwrap();
    assert!(p.changes.iter().all(|c| !c.conflict));
    let abc = resolve(&ab, &c, &p, &[], &keys, Uuid::new_v4()).unwrap();
    assert_eq!(abc.entries[0].notes, "A");
    assert_eq!(abc.entries[0].email, "B@example.test");
    assert_eq!(abc.entries[0].url, "https://c.example.test");
}

#[test]
fn reviewed_p2p_waits_for_approval_and_both_storage_receipts() {
    use super::{identity, resolve_local_device_info, review_transport as transport};
    use std::sync::{Arc, Mutex, atomic::AtomicBool, mpsc};
    let (a, keys) = fixture();
    let mut b = a.clone();
    b.entries[0].notes = "from phone".into();
    save(&mut b, &keys);
    let trusted = vec![TrustedDevice {
        id: resolve_local_device_info(None).id,
        signing_public_key: identity::public_key().unwrap(),
        ..Default::default()
    }];
    let server = Arc::new(Mutex::new(a));
    let client = Arc::new(Mutex::new(b));
    let (ready_tx, ready_rx) = mpsc::channel();
    let (server_state, server_keys, server_trust) = (server.clone(), keys.clone(), trusted.clone());
    let host = std::thread::spawn(move || {
        transport::listen(
            "127.0.0.1:0",
            &server_keys,
            &[4; 32],
            &server_trust,
            Arc::new(AtomicBool::new(false)),
            |port| ready_tx.send(port).unwrap(),
            || {
                let data = server_state.lock().unwrap().clone();
                Ok(transport::Snapshot {
                    revision: revision(&data)?,
                    data,
                    observed_at: Utc::now(),
                })
            },
            |expected, data, _, _| {
                let mut current = server_state.lock().unwrap();
                assert_eq!(revision(&current).unwrap(), expected);
                *current = data;
                Ok(())
            },
        )
    });
    let port = ready_rx.recv().unwrap();
    let local = {
        let data = client.lock().unwrap().clone();
        transport::Snapshot {
            revision: revision(&data).unwrap(),
            data,
            observed_at: Utc::now(),
        }
    };
    let result = transport::connect(
        &format!("127.0.0.1:{port}"),
        &keys,
        &[4; 32],
        &trusted,
        Arc::new(AtomicBool::new(false)),
        None,
        local,
        |local, remote| {
            assert!(server.lock().unwrap().entries[0].notes.is_empty());
            let p = preview(&local.data, &remote.data, &keys)?;
            Ok((
                p.id,
                resolve(&local.data, &remote.data, &p, &[], &keys, Uuid::new_v4())?,
            ))
        },
        |expected| {
            assert_eq!(revision(&client.lock().unwrap()).unwrap(), expected);
            Ok(())
        },
        |expected, data, _, _| {
            let mut current = client.lock().unwrap();
            assert_eq!(revision(&current).unwrap(), expected);
            *current = data;
            Ok(())
        },
    );
    result.unwrap();
    host.join().unwrap().unwrap();
    assert_eq!(server.lock().unwrap().entries[0].notes, "from phone");
    assert_eq!(
        revision(&server.lock().unwrap()).unwrap(),
        revision(&client.lock().unwrap()).unwrap()
    );
}

#[test]
fn reviewed_p2p_reports_a_stale_peer_without_committing_locally() {
    use super::{identity, resolve_local_device_info, review_transport as transport};
    use std::sync::{Arc, Mutex, atomic::AtomicBool, mpsc};
    let (a, keys) = fixture();
    let trusted = vec![TrustedDevice {
        id: resolve_local_device_info(None).id,
        signing_public_key: identity::public_key().unwrap(),
        ..Default::default()
    }];
    let server = Arc::new(Mutex::new(a.clone()));
    let (ready_tx, ready_rx) = mpsc::channel();
    let (state, host_keys, host_trust) = (server.clone(), keys.clone(), trusted.clone());
    let host = std::thread::spawn(move || transport::listen(
        "127.0.0.1:0", &host_keys, &[4; 32], &host_trust,
        Arc::new(AtomicBool::new(false)), |port| ready_tx.send(port).unwrap(),
        || {
            let data = state.lock().unwrap().clone();
            Ok(transport::Snapshot { revision: revision(&data)?, data, observed_at: Utc::now() })
        },
        |expected, _, _, _| {
            assert_ne!(revision(&state.lock().unwrap()).unwrap(), expected);
            Err(crate::VaultError::InvalidState("Vault changed in another session; reload before saving".into()))
        },
    ));
    let port = ready_rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
    let local = transport::Snapshot { revision: revision(&a).unwrap(), data: a, observed_at: Utc::now() };
    let result = transport::connect(
        &format!("127.0.0.1:{port}"), &keys, &[4; 32], &trusted,
        Arc::new(AtomicBool::new(false)), None, local,
        |local, remote| {
            {
                let mut changed = server.lock().unwrap();
                changed.entries[0].notes = "changed while review was open".into();
                save(&mut changed, &keys);
            }
            let p = preview(&local.data, &remote.data, &keys)?;
            Ok((p.id, resolve(&local.data, &remote.data, &p, &[], &keys, Uuid::new_v4())?))
        },
        |_| Ok(()),
        |_, _, _, _| panic!("A rejected review must not write the client vault"),
    );
    assert!(result.unwrap_err().to_string().contains("Other device changed during review"));
    assert!(host.join().unwrap().unwrap_err().to_string().contains("Vault changed"));
    assert_eq!(server.lock().unwrap().entries[0].notes, "changed while review was open");
}

#[test]
fn automatic_latest_requires_both_devices_and_never_resolves_deletion_conflicts() {
    let (mut base, keys) = fixture();
    let now = Utc::now();
    for revision in base.settings.sync.fields.values_mut() { revision.changed_at = now - chrono::Duration::minutes(3); }
    let mut local = base.clone();
    let mut remote = base.clone();
    local.entries[0].notes = "Desktop".into();
    remote.entries[0].notes = "Phone".into();
    track(&mut local, &keys, Uuid::new_v4(), now - chrono::Duration::minutes(2)).unwrap();
    track(&mut remote, &keys, Uuid::new_v4(), now - chrono::Duration::minutes(1)).unwrap();
    let original = preview(&local, &remote, &keys).unwrap();
    assert_eq!(original.changes.len(), 1);
    assert!(original.changes[0].conflict && original.changes[0].time_order_reliable);
    for bits in 0..16 {
        let local = SyncPolicy { review_conflicts_only: bits & 1 != 0, prefer_latest: bits & 2 != 0 };
        let remote = SyncPolicy { review_conflicts_only: bits & 4 != 0, prefer_latest: bits & 8 != 0 };
        let mut proposed = original.clone();
        let (decisions, needs_review) = apply_policy(&mut proposed, &local, &remote);
        assert_eq!(!needs_review, bits == 15);
        assert_eq!(decisions.len(), usize::from(local.prefer_latest));
        if let Some(decision) = decisions.first() { assert_eq!(decision.choice, Choice::Remote); }
    }
    let automatic = SyncPolicy { review_conflicts_only: true, prefer_latest: true };
    for field in ["state", "tag", "custom/synthetic", "attachment/synthetic", "encrypted_totp_secret"] {
        let mut proposed = original.clone();
        proposed.changes[0].field = field.into();
        proposed.changes[0].local_present = false;
        let (decisions, needs_review) = apply_policy(&mut proposed, &automatic, &automatic);
        assert!(decisions.is_empty() && needs_review, "{field}");
    }
    for uncertain in [false, true] {
        let mut proposed = original.clone();
        if uncertain { proposed.changes[0].time_order_reliable = false; }
        else { proposed.changes[0].remote_changed_at = proposed.changes[0].local_changed_at; }
        let (decisions, needs_review) = apply_policy(&mut proposed, &automatic, &automatic);
        assert!(decisions.is_empty() && needs_review);
    }
}

#[test]
fn independently_added_tags_with_the_same_name_are_resolved_in_review() {
    let (base, keys) = fixture();
    let mut local = base.clone();
    let mut remote = base.clone();
    let left = Uuid::new_v4();
    let right = Uuid::new_v4();
    for (data, id) in [(&mut local, left), (&mut remote, right)] {
        data.tags.push(Tag { id, name: "Work".into(), color: "blue".into(), icon: "tag".into() , default_url: String::new(),});
        data.entries[0].tags.push("Work".into());
        save(data, &keys);
    }
    let mut proposal = preview(&local, &remote, &keys).unwrap();
    let tags = proposal.changes.iter().filter(|change| change.field == "tag").collect::<Vec<_>>();
    assert_eq!(tags.len(), 2);
    assert!(tags.iter().all(|change| change.conflict && change.requires_explicit_choice && change.can_edit));
    let automatic = SyncPolicy { review_conflicts_only: true, prefer_latest: true };
    assert!(apply_policy(&mut proposal, &automatic, &automatic).1);
    let mut decisions: Vec<_> = proposal.changes.iter().filter(|change| change.conflict).map(|change| SyncDecision {
        key: change.key.clone(), choice: if change.field == "tag" && change.entry_id == right { Choice::Remote } else { Choice::Local }, text: None,
    }).collect();
    assert!(resolve(&local, &remote, &proposal, &decisions, &keys, Uuid::new_v4()).unwrap_err().to_string().contains("Tag names overlap"));
    let decision = decisions.iter_mut().find(|decision| decision.key == format!("{right}/tag")).unwrap();
    decision.choice = Choice::Edited; decision.text = Some("Phone work".into());
    let merged = resolve(&local, &remote, &proposal, &decisions, &keys, Uuid::new_v4()).unwrap();
    assert_eq!(merged.tags.len(), 2);
    assert_eq!(merged.tags.iter().find(|tag| tag.id == right).unwrap().name, "Phone work");
    assert!(merged.entries[0].tags.contains(&"Work".into()));
    assert!(merged.entries[0].tags.contains(&"Phone work".into()));
    for previous in [&local, &remote, &base] {
        let proposed = preview(&merged, previous, &keys).unwrap();
        assert!(proposed.changes.iter().all(|change| !change.conflict));
        let repeated = resolve(&merged, previous, &proposed, &[], &keys, Uuid::new_v4()).unwrap();
        assert!(preview(&merged, &repeated, &keys).unwrap().changes.is_empty());
    }
}

#[test]
fn tag_rename_and_independent_memberships_follow_stable_identity() {
    let (mut base, keys) = fixture();
    let work = Uuid::new_v4();
    let social = Uuid::new_v4();
    base.tags = vec![
        Tag { id: work, name: "Work".into(), color: "".into(), icon: "".into() , default_url: String::new(),},
        Tag { id: social, name: "Social".into(), color: "".into(), icon: "".into() , default_url: String::new(),},
    ];
    save(&mut base, &keys);
    let mut desktop = base.clone();
    let mut phone = base.clone();
    desktop.tags[0].name = "Office".into();
    desktop.entries[0].tags = vec!["Social".into()];
    save(&mut desktop, &keys);
    phone.entries[0].tags = vec!["Work".into()];
    save(&mut phone, &keys);
    for (a, b) in [(&desktop, &phone), (&phone, &desktop)] {
        let proposal = preview(a, b, &keys).unwrap();
        assert!(proposal.changes.iter().all(|change| !change.conflict));
        let merged = resolve(a, b, &proposal, &[], &keys, Uuid::new_v4()).unwrap();
        assert!(merged.entries[0].tags.contains(&"Office".into()));
        assert!(merged.entries[0].tags.contains(&"Social".into()));
        assert_eq!(merged.entries[0].tags.len(), 2);
        let proposal = preview(&merged, &base, &keys).unwrap();
        assert!(proposal.changes.iter().all(|change| !change.conflict));
        let third = resolve(&merged, &base, &proposal, &[], &keys, Uuid::new_v4()).unwrap();
        assert!(preview(&third, &merged, &keys).unwrap().changes.is_empty());
    }
}

#[test]
fn deleting_a_tag_conflicts_with_a_concurrent_new_assignment() {
    let (mut local, keys) = fixture();
    let tag = Uuid::new_v4();
    local.tags.push(Tag { id: tag, name: "Work".into(), color: "".into(), icon: "".into() , default_url: String::new(),});
    save(&mut local, &keys);
    let mut remote = local.clone();
    local.tags.clear();
    save(&mut local, &keys);
    remote.entries[0].tags.push("Work".into());
    save(&mut remote, &keys);
    let proposal = preview(&local, &remote, &keys).unwrap();
    assert!(proposal.changes.iter().any(|change| change.field == "tag" && change.conflict));
    let tag_change = proposal.changes.iter().find(|change| change.field == "tag").unwrap();
    assert!(!tag_change.local_present && tag_change.remote_present);
    assert!(resolve(&local, &remote, &proposal, &[], &keys, Uuid::new_v4()).is_err());
    let decisions: Vec<_> = proposal.changes.iter().filter(|change| change.conflict).map(|change| SyncDecision {
        key: change.key.clone(), choice: Choice::Remote, text: None,
    }).collect();
    let merged = resolve(&local, &remote, &proposal, &decisions, &keys, Uuid::new_v4()).unwrap();
    assert_eq!(merged.tags[0].id, tag);
    assert_eq!(merged.entries[0].tags, vec!["Work".to_string()]);
    assert!(preview(&merged, &merged, &keys).unwrap().changes.is_empty());
}
