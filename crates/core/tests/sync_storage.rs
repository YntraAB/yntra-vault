use yntra_vault_core::{
    crypto::encrypt_vault_with_aad,
    services::sync::review,
    vault::{
        NewEntry, UpdateEntry, VaultManager,
        format::{FORMAT_VERSION, FileHeader, VaultFile},
    },
};

#[test]
fn downloaded_restore_preserves_backup_and_rejects_changed_or_protected_destinations() {
    use yntra_vault_core::services::sync::{restore_downloaded, restore_revision};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("local.vdb");
    let remote = directory.path().join("remote.vdb");
    let mut local = VaultManager::create("Local", "restore fixture password", &path).unwrap();
    VaultManager::create("Remote", "restore fixture password", &remote).unwrap();
    let downloaded = std::fs::read(&remote).unwrap();
    let initial = std::fs::read(&path).unwrap();
    let revision = restore_revision(&path).unwrap();
    local.add_tag("Changed during download", "blue", "tag").unwrap();
    let changed = std::fs::read(&path).unwrap();
    assert!(restore_downloaded(&downloaded, &path, revision).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), changed);
    let revision = restore_revision(&path).unwrap();
    assert!(restore_downloaded(b"invalid", &path, revision).is_err());
    restore_downloaded(&downloaded, &path, revision).unwrap();
    assert_eq!(std::fs::read(path.with_extension("vdb.bak")).unwrap(), changed);
    assert_eq!(std::fs::read(&path).unwrap(), downloaded);
    assert_ne!(initial, downloaded);
    assert!(restore_revision(&directory.path().join("unsafe.bat")).is_err());
    let mut protected = VaultManager::open(&path, "restore fixture password").unwrap();
    protected.generate_emergency_kit("restore fixture password").unwrap();
    assert!(restore_revision(&path).is_err());
    assert!(restore_downloaded(&downloaded, &path, None).is_err());
}

#[test]
fn legacy_migration_creates_an_exact_recovery_copy_and_persists_deletion_markers() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fixture.vdb");
    let password = "migration fixture password";
    let mut manager = VaultManager::create("Fixture", password, &path).unwrap();
    manager.add_tag("Legacy tag", "blue", "tag").unwrap();
    let id = manager
        .add_entry(NewEntry {
            title: "Gmail".into(),
            password: "original".into(),
            username: String::new(),
            email: String::new(),
            url: String::new(),
            notes: String::new(),
            tags: vec![],
            totp_secret: None,
            custom_fields: vec![],
            entry_type: None,
            generate_passkey: None,
            attachments: None,
        })
        .unwrap();
    let mut legacy = manager.data.clone();
    legacy.settings.sync = Default::default();
    let header = FileHeader {
        version: 4,
        flags: 0,
        salt: manager.salt(),
        kdf_params: Default::default(),
    };
    let plain = rmp_serde::to_vec(&legacy).unwrap();
    let blob = encrypt_vault_with_aad(
        &plain,
        &manager.get_subkeys().unwrap().vault_key,
        &header.aad_bytes().unwrap(),
    )
    .unwrap();
    let mut encrypted_payload = blob.nonce;
    encrypted_payload.extend(blob.ciphertext);
    let original = VaultFile {
        header,
        hmac: None,
        biometric: None,
        hardware2fa: None,
        encrypted_payload,
    }
    .to_bytes()
    .unwrap();
    std::fs::write(&path, &original).unwrap();
    manager.reload().unwrap();
    assert_eq!(manager.data.settings.sync.version, review::SYNC_METADATA_VERSION);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    manager.delete_entry(id).unwrap();
    manager.permanent_delete(id).unwrap();
    let backups: Vec<_> = std::fs::read_dir(directory.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|s| s == "bak"))
        .collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(std::fs::read(&backups[0]).unwrap(), original);
    assert_eq!(
        VaultFile::from_bytes(&std::fs::read(&path).unwrap())
            .unwrap()
            .header
            .version,
        FORMAT_VERSION
    );
    let reopened = VaultManager::open(&path, password).unwrap();
    assert_eq!(reopened.tags()[0].name, "Legacy tag");
    assert!(reopened.tags()[0].default_url.is_empty());
    assert!(reopened.data.entries.is_empty());
    assert!(
        reopened
            .data
            .settings
            .sync
            .fields
            .contains_key(&format!("{id}/state"))
    );
    let p = review::preview(&reopened.data, &legacy, reopened.get_subkeys().unwrap()).unwrap();
    assert_eq!(p.changes[0].suggested, Some(review::Choice::Local));
}

#[test]
fn history_restores_the_visible_index_and_failed_writes_restore_memory() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fixture.vdb");
    let password = "history fixture password";
    let mut manager = VaultManager::create("Fixture", password, &path).unwrap();
    let id = manager
        .add_entry(NewEntry {
            title: "Gmail".into(),
            password: "first".into(),
            username: String::new(),
            email: String::new(),
            url: String::new(),
            notes: String::new(),
            tags: vec![],
            totp_secret: None,
            custom_fields: vec![],
            entry_type: None,
            generate_passkey: None,
            attachments: None,
        })
        .unwrap();
    for value in ["second", "third"] {
        manager
            .update_entry(
                id,
                UpdateEntry {
                    password: Some(value.into()),
                    ..Default::default()
                },
            )
            .unwrap();
    }
    assert_eq!(
        manager.get_password_history(id).unwrap()[0].password,
        "second"
    );
    manager.restore_password_from_history(id, 0).unwrap();
    assert_eq!(manager.get_entry(id).unwrap().password, "second");
    let mut concurrent = VaultManager::open(&path, password).unwrap();
    concurrent.add_tag("Concurrent", "#123456", "").unwrap();
    let before = review::revision(&manager.data).unwrap();
    assert!(manager.restore_password_from_history(id, 0).is_err());
    assert_eq!(review::revision(&manager.data).unwrap(), before);
    assert!(manager.add_tag("Must roll back", "#123456", "").is_err());
    assert_eq!(review::revision(&manager.data).unwrap(), before);
    manager.reload().unwrap();
    manager.delete_entry(id).unwrap();
    concurrent.reload().unwrap();
    concurrent
        .add_tag("Second concurrent edit", "#654321", "")
        .unwrap();
    let before = review::revision(&manager.data).unwrap();
    assert!(manager.permanent_delete(id).is_err());
    assert_eq!(review::revision(&manager.data).unwrap(), before);
}

#[test]
fn interrupted_completion_is_durable_and_review_does_not_expire_trash() {
    use yntra_vault_core::services::sync::review_transport::{self, Snapshot};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fixture.vdb");
    let password = "receipt fixture password";
    let mut manager = VaultManager::create("Fixture", password, &path).unwrap();
    let peer = uuid::Uuid::new_v4();
    manager
        .data
        .settings
        .trusted_devices
        .push(yntra_vault_core::vault::TrustedDevice {
            id: peer,
            signing_public_key: vec![1; 32],
            ..Default::default()
        });
    let id = manager
        .add_entry(NewEntry {
            title: "Gmail".into(),
            password: "first".into(),
            username: String::new(),
            email: String::new(),
            url: String::new(),
            notes: String::new(),
            tags: vec![],
            totp_secret: None,
            custom_fields: vec![],
            entry_type: None,
            generate_passkey: None,
            attachments: None,
        })
        .unwrap();
    manager.delete_entry(id).unwrap();
    manager.data.trash[0].deleted_at = chrono::Utc::now() - chrono::Duration::days(60);
    manager.save_reviewed().unwrap();
    let snapshot = Snapshot::capture(&manager).unwrap();
    let operation = uuid::Uuid::new_v4();
    review_transport::apply(&mut manager, &snapshot.revision, snapshot.data, operation).unwrap();
    let mut reopened = VaultManager::open(&path, password).unwrap();
    assert_eq!(reopened.data.trash.len(), 1);
    assert!(
        reopened
            .data
            .settings
            .sync_pending_receipts
            .contains_key(&operation)
    );
    assert_eq!(
        Snapshot::capture(&reopened)
            .unwrap()
            .data
            .settings
            .sync_pending,
        None
    );
    assert_eq!(reopened.data.settings.trusted_devices[0].last_sync_at, None);
    review_transport::acknowledge_with_peer(&mut reopened, operation, Some(peer)).unwrap();
    let confirmed = VaultManager::open(&path, password).unwrap();
    assert_eq!(confirmed.data.settings.sync_pending, None);
    assert!(confirmed.data.settings.sync_pending_receipts.is_empty());
    assert_eq!(confirmed.data.trash.len(), 1);
    assert!(
        confirmed.data.settings.trusted_devices[0]
            .last_sync_at
            .is_some()
    );
}

#[test]
fn snapshots_reject_stale_storage_and_changed_trust() {
    use yntra_vault_core::{services::sync::review_transport::Snapshot, vault::TrustedDevice};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("trust.vdb");
    let password = "trust fixture password";
    let mut manager = VaultManager::create("Fixture", password, &path).unwrap();
    manager.data.settings.trusted_devices.push(TrustedDevice {
        id: uuid::Uuid::new_v4(),
        signing_public_key: vec![3; 32],
        ..Default::default()
    });
    manager.save().unwrap();
    let trusted = manager.data.settings.trusted_devices.clone();
    assert!(Snapshot::capture_with_trust(&manager, &trusted).is_ok());
    manager.revoke_trusted_device(trusted[0].id).unwrap();
    assert!(Snapshot::capture_with_trust(&manager, &trusted).is_err());
    let mut concurrent = VaultManager::open(&path, password).unwrap();
    concurrent.add_tag("External update", "", "").unwrap();
    assert!(Snapshot::capture(&manager).is_err());
    manager.reload().unwrap();
    assert!(Snapshot::capture(&manager).is_ok());
}

#[test]
fn pending_receipts_survive_other_peers_and_late_confirmations() {
    use yntra_vault_core::services::sync::review_transport::{self, Snapshot};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pending-peers.vdb");
    let password = "pending receipt fixture password";
    let mut manager = VaultManager::create("Fixture", password, &path).unwrap();
    let peer_a = uuid::Uuid::new_v4();
    let peer_b = uuid::Uuid::new_v4();
    let op_a = uuid::Uuid::new_v4();
    let op_b = uuid::Uuid::new_v4();
    let retry_a = uuid::Uuid::new_v4();
    let save = |manager: &mut VaultManager, operation, peer| {
        let snapshot = Snapshot::capture(manager).unwrap();
        assert!(snapshot.data.settings.sync_pending_receipts.is_empty());
        review_transport::apply_with_peer(
            manager,
            &snapshot.revision,
            snapshot.data,
            operation,
            Some(peer),
        )
        .unwrap();
    };
    save(&mut manager, op_a, peer_a);
    save(&mut manager, op_b, peer_b);
    manager = VaultManager::open(&path, password).unwrap();
    assert_eq!(manager.data.settings.sync_pending_receipts.len(), 2);
    let before = std::fs::read(&path).unwrap();
    assert!(review_transport::acknowledge_with_peer(&mut manager, op_a, Some(peer_b)).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    review_transport::acknowledge_with_peer(&mut manager, op_b, Some(peer_b)).unwrap();
    assert!(
        manager
            .data
            .settings
            .sync_pending_receipts
            .contains_key(&op_a)
    );
    assert!(
        !manager
            .data
            .settings
            .sync_pending_receipts
            .contains_key(&op_b)
    );
    save(&mut manager, retry_a, peer_a);
    review_transport::acknowledge_with_peer(&mut manager, op_a, Some(peer_a)).unwrap();
    assert!(
        manager
            .data
            .settings
            .sync_pending_receipts
            .contains_key(&retry_a)
    );
    manager = VaultManager::open(&path, password).unwrap();
    review_transport::acknowledge_with_peer(&mut manager, retry_a, Some(peer_a)).unwrap();
    assert!(manager.data.settings.sync_pending_receipts.is_empty());
    save(&mut manager, uuid::Uuid::new_v4(), peer_a);
    save(&mut manager, retry_a, peer_a);
    review_transport::acknowledge_with_peer(&mut manager, retry_a, Some(peer_a)).unwrap();
    assert!(
        VaultManager::open(&path, password)
            .unwrap()
            .data
            .settings
            .sync_pending_receipts
            .is_empty()
    );
}

#[test]
fn conflict_history_restoration_is_reviewed_atomic_and_supersedes_previous_values() {
    use yntra_vault_core::services::sync::review::{Choice, SyncDecision};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("history.vdb");
    let password = "history fixture password";
    let mut manager = VaultManager::create("Fixture", password, &path).unwrap();
    let id = manager.add_entry(NewEntry {
        title: "Discord".into(), password: "fixture password".into(), username: String::new(),
        email: String::new(), url: String::new(), notes: "Original".into(), tags: vec![],
        totp_secret: None, custom_fields: vec![], entry_type: None, generate_passkey: None, attachments: None,
    }).unwrap();
    let keys = manager.get_subkeys().unwrap().clone();
    let mut remote = manager.data.clone();
    remote.entries[0].notes = "SYNTHETIC OLD SECRET".into();
    review::track(&mut remote, &keys, uuid::Uuid::new_v4(), chrono::Utc::now()).unwrap();
    manager.update_entry(id, UpdateEntry { notes: Some("Current".into()), ..Default::default() }).unwrap();
    let preview = review::preview(&manager.data, &remote, &keys).unwrap();
    let decisions: Vec<_> = preview.changes.iter().map(|change| SyncDecision { key: change.key.clone(), choice: Choice::Local, text: None }).collect();
    manager.data = review::resolve(&manager.data, &remote, &preview, &decisions, &keys, manager.session_identity()).unwrap();
    manager.save_reviewed().unwrap();
    let versions = manager.list_conflict_history(id).unwrap();
    let serialized = serde_json::to_string(&versions).unwrap();
    assert!(!serialized.contains("SYNTHETIC OLD SECRET"));
    let version = versions.iter().find(|version| manager.inspect_conflict_history(id, version.id, "notes").unwrap() == "SYNTHETIC OLD SECRET").unwrap().id;
    let proposal = manager.preview_conflict_history(id, version).unwrap();
    assert!(!serde_json::to_string(&proposal).unwrap().contains("SYNTHETIC OLD SECRET"));
    manager.update_entry(id, UpdateEntry { username: Some("Newer edit".into()), ..Default::default() }).unwrap();
    assert!(manager.restore_conflict_history(id, version, &proposal.local_revision).is_err());
    let proposal = manager.preview_conflict_history(id, version).unwrap();
    let before = manager.data.clone();
    let mut concurrent = VaultManager::open(&path, password).unwrap();
    concurrent.add_tag("Concurrent", "", "").unwrap();
    assert!(manager.restore_conflict_history(id, version, &proposal.local_revision).is_err());
    assert_eq!(review::revision(&manager.data).unwrap(), review::revision(&before).unwrap());
    manager.reload().unwrap();
    let proposal = manager.preview_conflict_history(id, version).unwrap();
    let old = manager.data.clone();
    manager.restore_conflict_history(id, version, &proposal.local_revision).unwrap();
    assert_eq!(manager.get_entry(id).unwrap().notes, "SYNTHETIC OLD SECRET");
    let preview = review::preview(&manager.data, &old, &keys).unwrap();
    assert!(preview.changes.iter().all(|change| !change.conflict));
    let reopened = VaultManager::open(&path, password).unwrap();
    assert_eq!(reopened.get_entry(id).unwrap().notes, "SYNTHETIC OLD SECRET");
    assert!(reopened.list_conflict_history(id).unwrap().iter().any(|version| reopened.inspect_conflict_history(id, version.id, "notes").unwrap() == "Current"));
    manager.delete_entry(id).unwrap();
    manager.permanent_delete(id).unwrap();
    assert!(manager.preview_conflict_history(id, version).is_err());
    assert!(!manager.data.settings.sync.history.contains_key(&id));
}

#[test]
fn bulk_mutations_and_entry_flags_are_atomic_and_preserve_notes() {
    use yntra_vault_core::vault::manager::BulkEntryUpdate;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bulk.vdb");
    let password = "Synthetic bulk storage fixture";
    let mut manager = VaultManager::create("Bulk", password, &path).unwrap();
    let mut ids = Vec::new();
    for name in ["Gmail", "Discord"] {
        ids.push(manager.add_entry(NewEntry { title: name.into(), username: String::new(), password: "fixture".into(), url: String::new(), email: String::new(), notes: format!("{name} original"), tags: vec!["Existing".into()], totp_secret: None, custom_fields: vec![], entry_type: None, generate_passkey: None, attachments: None }).unwrap());
    }
    let patch = BulkEntryUpdate { notes: Some("appended".into()), append_notes: true, tags_to_add: vec!["Added".into()], favorite: Some(true), ..Default::default() };
    let revision = review::revision(&manager.data).unwrap();
    let disk = std::fs::read(&path).unwrap();
    let missing = uuid::Uuid::max();
    assert!(manager.bulk_update_entries(&[ids[0], missing], patch.clone()).is_err());
    assert!(manager.delete_entries(&[ids[0], missing]).is_err());
    assert_eq!(review::revision(&manager.data).unwrap(), revision);
    assert_eq!(std::fs::read(&path).unwrap(), disk);
    manager.bulk_update_entries(&[ids[0], ids[1], ids[0]], patch.clone()).unwrap();
    let mut reopened = VaultManager::open(&path, password).unwrap();
    for (id, name) in ids.iter().zip(["Gmail", "Discord"]) {
        let entry = reopened.get_entry(*id).unwrap();
        assert_eq!(entry.notes, format!("{name} original\nappended"));
        assert!(entry.favorite);
        assert_eq!(entry.tags, vec!["Existing", "Added"]);
    }
    reopened.add_tag("Concurrent", "#123456", "tag").unwrap();
    let revision = review::revision(&manager.data).unwrap();
    let disk = std::fs::read(&path).unwrap();
    assert!(manager.bulk_update_entries(&ids, patch).is_err());
    assert!(manager.delete_entry(ids[0]).is_err());
    assert!(manager.delete_entries(&ids).is_err());
    assert!(manager.toggle_favorite(ids[0]).is_err());
    assert!(manager.toggle_pin(ids[0]).is_err());
    assert_eq!(review::revision(&manager.data).unwrap(), revision);
    assert_eq!(manager.search_entries("Gmail").unwrap().len(), 1);
    assert_eq!(std::fs::read(&path).unwrap(), disk);
    manager.reload().unwrap();
    manager.delete_entries(&[ids[0], ids[1], ids[0]]).unwrap();
    let reopened = VaultManager::open(&path, password).unwrap();
    assert!(reopened.list_entries().unwrap().is_empty());
    assert_eq!(reopened.list_trash().unwrap().len(), 2);
}
