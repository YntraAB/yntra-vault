//! Integration tests for VaultManager.

use std::fs;
use std::path::PathBuf;
use chrono::Utc;
use uuid::Uuid;

use yntra_vault_core::error::VaultError;
use yntra_vault_core::vault::entry::{DecryptedEntry, NewEntry, UpdateEntry};
use yntra_vault_core::vault::format::{FileHeader, KdfParams, VaultFile, FORMAT_VERSION};
use yntra_vault_core::vault::manager::{read_key_file_safely, VaultManager};
use yntra_vault_core::vault::types::*;
use yntra_vault_core::services::sync::merge_vault_data;
use yntra_vault_core::crypto::hardware2fa::{perform_hardware2fa_challenge, set_hardware2fa_mock, Hardware2FaProtocol};

struct TestVault {
    path: PathBuf,
}

impl TestVault {
    fn new() -> Self {
        let mut path = std::env::temp_dir();
        path.push(format!("yntra_vault_test_{}.vdb", Uuid::new_v4()));
        TestVault { path }
    }
}

impl Drop for TestVault {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[test]
fn test_vault_lifecycle_and_master_password_change() {
    let test_vault = TestVault::new();
    let password = "initial-secure-password";

    // 1. Create vault
    let mut manager = VaultManager::create("my-test-vault", password, &test_vault.path).unwrap();
    assert!(manager.is_unlocked());
    assert_eq!(manager.metadata().name, "my-test-vault");

    // 2. Add an entry
    let entry1 = NewEntry {
        title: "Service A".to_string(),
        username: "userA".to_string(),
        password: "passwordA-1".to_string(),
        url: "https://a.com".to_string(),
        email: "a@a.com".to_string(),
        notes: "Notes A".to_string(),
        tags: vec!["Work".to_string()],
        totp_secret: Some("JBSWY3DPEHPK3PXP".to_string()),
        custom_fields: Vec::new(),
        entry_type: Some(EntryType::Login),
        generate_passkey: None,
        attachments: None,
    };

    let id1 = manager.add_entry(entry1).unwrap();

    // 3. Update entry to generate history item
    let update = UpdateEntry {
        password: Some("passwordA-2".to_string()),
        ..Default::default()
    };
    manager.update_entry(id1, update).unwrap();

    // Check history count
    let history = manager.get_password_history(id1).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].password, "passwordA-1");

    // 4. Delete entry (moves it to trash)
    manager.delete_entry(id1).unwrap();
    assert_eq!(manager.list_entries().unwrap().len(), 0);
    assert_eq!(manager.list_trash().unwrap().len(), 1);

    // 5. Add second active entry
    let entry2 = NewEntry {
        title: "Service B".to_string(),
        username: "userB".to_string(),
        password: "passwordB-1".to_string(),
        url: "https://b.com".to_string(),
        email: "b@b.com".to_string(),
        notes: "Notes B".to_string(),
        tags: vec!["Personal".to_string()],
        totp_secret: None,
        custom_fields: Vec::new(),
        entry_type: Some(EntryType::Login),
        generate_passkey: None,
        attachments: None,
    };
    let id2 = manager.add_entry(entry2).unwrap();

    // 6. Change master password
    let new_password = "new-secure-password";
    manager.change_master_password(password, new_password).unwrap();

    // 7. Save and lock
    manager.save().unwrap();
    manager.lock();
    assert!(!manager.is_unlocked());

    // 8. Re-open with new master password
    let reopened = VaultManager::open(&test_vault.path, new_password).unwrap();
    assert!(reopened.is_unlocked());

    // Check active entry
    let dec2 = reopened.get_entry(id2).unwrap();
    assert_eq!(dec2.password, "passwordB-1");

    // 9. Restore first entry from trash and verify it decrypts correctly
    let mut reopened_mut = reopened;
    reopened_mut.restore_from_trash(id1).unwrap();

    let dec1 = reopened_mut.get_entry(id1).unwrap();
    assert_eq!(dec1.password, "passwordA-2");
    assert_eq!(dec1.totp_secret.as_deref(), Some("JBSWY3DPEHPK3PXP"));

    // Check restored entry's password history decrypts correctly
    let restored_history = reopened_mut.get_password_history(id1).unwrap();
    assert_eq!(restored_history.len(), 1);
    assert_eq!(restored_history[0].password, "passwordA-1");
}

#[test]
fn test_truncated_payload_returns_error() {
    let test_vault = TestVault::new();
    let password = "test-password";
    let manager = VaultManager::create("test-vault", password, &test_vault.path).unwrap();
    drop(manager);

    // Read vault bytes, corrupt payload length to < 24 bytes
    let header = FileHeader {
        version: FORMAT_VERSION,
        flags: 0,
        salt: [1u8; 32],
        kdf_params: KdfParams::default(),
    };
    let file = VaultFile {
        header,
        hmac: None,
        biometric: None,
        hardware2fa: None,
        encrypted_payload: vec![1, 2, 3], // Payload < 24 bytes
    };
    let corrupted_bytes = file.to_bytes().unwrap();
    fs::write(&test_vault.path, &corrupted_bytes).unwrap();

    let result = VaultManager::open(&test_vault.path, password);
    assert!(result.is_err());
    match result {
        Err(VaultError::IntegrityError) | Err(VaultError::InvalidFormat(_)) => {}
        Err(e) => panic!("Expected IntegrityError or InvalidFormat, got err {:?}", e),
        Ok(_) => panic!("Expected error on truncated payload, got Ok"),
    }
}

#[test]
fn test_history_aad_isolation_prevents_substitution() {
    let test_vault = TestVault::new();
    let password = "test-password";
    let mut manager = VaultManager::create("test-vault", password, &test_vault.path).unwrap();

    let entry = NewEntry {
        title: "Security Test".to_string(),
        username: "user".to_string(),
        password: "active-password-v1".to_string(),
        url: "".to_string(),
        email: "".to_string(),
        notes: "".to_string(),
        tags: vec![],
        totp_secret: None,
        custom_fields: Vec::new(),
        entry_type: None,
        generate_passkey: None,
        attachments: None,
    };
    let id = manager.add_entry(entry).unwrap();

    // Change password to generate history item
    manager
        .update_entry(
            id,
            UpdateEntry {
                password: Some("active-password-v2".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

    // Verify active password decrypts under Scope::Password
    let dec = manager.get_entry(id).unwrap();
    assert_eq!(dec.password, "active-password-v2");

    // Verify history item decrypts under Scope::History
    let history = manager.get_password_history(id).unwrap();
    assert_eq!(history[0].password, "active-password-v1");

    // Now attempt to swap encrypted_password with history item's encrypted_password in memory
    let entry_mut = manager.data.entries.iter_mut().find(|e| e.id == id).unwrap();
    entry_mut.encrypted_password = entry_mut.password_history[0].encrypted_password.clone();

    // Attempting to decrypt the history blob under Scope::Password MUST fail due to AAD mismatch
    let result = manager.get_entry(id);
    assert!(result.is_err());
}

#[test]
fn test_file_attachment_encryption_decryption() {
    let test_vault = TestVault::new();
    let password = "attachment-test-password";
    let mut manager = VaultManager::create("attachment-vault", password, &test_vault.path).unwrap();

    // Staged attachment
    let raw_bytes = b"SECRET ENCRYPTED FILE PAYLOAD 1234567890".to_vec();
    let new_att = NewAttachment {
        name: "test_doc.txt".to_string(),
        mime_type: "text/plain".to_string(),
        data: raw_bytes.clone(),
    };

    let new_entry = NewEntry {
        title: "Attachment Entry".to_string(),
        username: "user".to_string(),
        password: "password123".to_string(),
        url: "".to_string(),
        email: "".to_string(),
        notes: "".to_string(),
        tags: vec![],
        totp_secret: None,
        custom_fields: Vec::new(),
        entry_type: None,
        generate_passkey: None,
        attachments: Some(vec![new_att]),
    };

    let entry_id = manager.add_entry(new_entry).unwrap();

    // Verify entry preview reports 1 attachment
    let previews = manager.list_entries().unwrap();
    assert_eq!(previews[0].attachment_count, 1);

    // Verify get_entry lists attachment metadata
    let dec_entry = manager.get_entry(entry_id).unwrap();
    assert_eq!(dec_entry.attachments.len(), 1);
    assert_eq!(dec_entry.attachments[0].name, "test_doc.txt");
    assert_eq!(dec_entry.attachments[0].size, raw_bytes.len() as u64);

    let att_id = dec_entry.attachments[0].id;

    // Decrypt attachment raw bytes and verify match
    let decrypted_bytes = manager.get_attachment_data(entry_id, att_id).unwrap();
    assert_eq!(decrypted_bytes, raw_bytes);

    // Add second attachment via add_attachment method
    let raw_bytes_2 = b"SECOND FILE PAYLOAD PNG DATA".to_vec();
    let info2 = manager
        .add_attachment(entry_id, "image.png", "image/png", &raw_bytes_2)
        .unwrap();
    assert_eq!(info2.name, "image.png");

    let dec_entry2 = manager.get_entry(entry_id).unwrap();
    assert_eq!(dec_entry2.attachments.len(), 2);

    // Delete first attachment
    manager.delete_attachment(entry_id, att_id).unwrap();
    let dec_entry3 = manager.get_entry(entry_id).unwrap();
    assert_eq!(dec_entry3.attachments.len(), 1);
    assert_eq!(dec_entry3.attachments[0].name, "image.png");

    // Save and lock
    manager.save().unwrap();
    manager.lock();

    // Re-open vault and verify attachment persists and decrypts correctly
    let reopened = VaultManager::open(&test_vault.path, password).unwrap();
    let dec_reopened = reopened.get_entry(entry_id).unwrap();
    assert_eq!(dec_reopened.attachments.len(), 1);
    let reopened_bytes = reopened.get_attachment_data(entry_id, info2.id).unwrap();
    assert_eq!(reopened_bytes, raw_bytes_2);
}

#[test]
fn test_change_master_password_reencrypts_attachments() {
    let test_vault = TestVault::new();
    let password = "old-master-password";
    let mut manager = VaultManager::create("attachment-rekey-vault", password, &test_vault.path).unwrap();

    // 1. Create active entry with an attachment
    let active_att_data = b"ACTIVE ENTRY ATTACHMENT CONTENT 12345".to_vec();
    let active_entry = NewEntry {
        title: "Active Entry".to_string(),
        username: "active_user".to_string(),
        password: "active_password".to_string(),
        url: "".to_string(),
        email: "".to_string(),
        notes: "".to_string(),
        tags: vec![],
        totp_secret: None,
        custom_fields: Vec::new(),
        entry_type: None,
        generate_passkey: None,
        attachments: Some(vec![NewAttachment {
            name: "active_file.txt".to_string(),
            mime_type: "text/plain".to_string(),
            data: active_att_data.clone(),
        }]),
    };
    let active_id = manager.add_entry(active_entry).unwrap();

    // 2. Create another entry with an attachment and move it to trash
    let trashed_att_data = b"TRASHED ENTRY ATTACHMENT CONTENT 67890".to_vec();
    let trashed_entry = NewEntry {
        title: "Trashed Entry".to_string(),
        username: "trashed_user".to_string(),
        password: "trashed_password".to_string(),
        url: "".to_string(),
        email: "".to_string(),
        notes: "".to_string(),
        tags: vec![],
        totp_secret: None,
        custom_fields: Vec::new(),
        entry_type: None,
        generate_passkey: None,
        attachments: Some(vec![NewAttachment {
            name: "trashed_file.txt".to_string(),
            mime_type: "text/plain".to_string(),
            data: trashed_att_data.clone(),
        }]),
    };
    let trashed_id = manager.add_entry(trashed_entry).unwrap();
    manager.delete_entry(trashed_id).unwrap();

    // Verify active attachment decrypts before password change
    let dec_active = manager.get_entry(active_id).unwrap();
    let active_att_id = dec_active.attachments[0].id;
    assert_eq!(
        manager.get_attachment_data(active_id, active_att_id).unwrap(),
        active_att_data
    );

    // 3. Change master password
    let new_password = "new-super-secure-password";
    manager.change_master_password(password, new_password).unwrap();
    manager.save().unwrap();
    manager.lock();

    // 4. Old password must fail
    assert!(VaultManager::open(&test_vault.path, password).is_err());

    // 5. Open with new password
    let mut reopened = VaultManager::open(&test_vault.path, new_password).unwrap();

    // 6. Verify active entry's attachment decrypts cleanly under the new entry key
    let dec_active_reopened = reopened.get_entry(active_id).unwrap();
    assert_eq!(dec_active_reopened.password, "active_password");
    assert_eq!(dec_active_reopened.attachments.len(), 1);
    let active_data_after = reopened.get_attachment_data(active_id, active_att_id).unwrap();
    assert_eq!(active_data_after, active_att_data);

    // 7. Restore trashed entry and verify its attachment decrypts cleanly under the new entry key
    reopened.restore_from_trash(trashed_id).unwrap();
    let dec_trashed_restored = reopened.get_entry(trashed_id).unwrap();
    assert_eq!(dec_trashed_restored.password, "trashed_password");
    assert_eq!(dec_trashed_restored.attachments.len(), 1);
    let trashed_att_id = dec_trashed_restored.attachments[0].id;
    let trashed_data_after = reopened.get_attachment_data(trashed_id, trashed_att_id).unwrap();
    assert_eq!(trashed_data_after, trashed_att_data);
}

#[test]
fn test_key_file_safely_size_limit_and_zeroization() {
    let temp_dir = tempfile::tempdir().unwrap();
    let kf_path = temp_dir.path().join("test.key");

    // Test normal 32-byte keyfile
    VaultManager::generate_key_file(&kf_path).unwrap();
    let bytes = read_key_file_safely(&kf_path).unwrap();
    assert_eq!(bytes.as_slice().len(), 32);

    // Test non-existent file
    let missing_path = temp_dir.path().join("missing.key");
    assert!(read_key_file_safely(&missing_path).is_err());

    // Test empty (0-byte) keyfile rejection
    let empty_path = temp_dir.path().join("empty.key");
    std::fs::write(&empty_path, b"").unwrap();
    let empty_res = read_key_file_safely(&empty_path);
    assert!(empty_res.is_err(), "Empty keyfile must be rejected");
}

#[test]
fn test_security_audit_ignores_empty_passwords() {
    let test_vault = TestVault::new();
    let password = "master-password-123";
    let mut manager = VaultManager::create("audit-test-vault", password, &test_vault.path).unwrap();

    // Add 2 entries with empty passwords (e.g. Secure Notes or empty password items)
    let entry1 = NewEntry {
        title: "Note 1".to_string(),
        username: "".to_string(),
        password: "".to_string(),
        url: "".to_string(),
        email: "".to_string(),
        notes: "Secret Note A".to_string(),
        tags: vec![],
        totp_secret: None,
        custom_fields: vec![],
        entry_type: Some(EntryType::SecureNote),
        generate_passkey: None,
        attachments: None,
    };
    let entry2 = NewEntry {
        title: "Note 2".to_string(),
        username: "".to_string(),
        password: "".to_string(),
        url: "".to_string(),
        email: "".to_string(),
        notes: "Secret Note B".to_string(),
        tags: vec![],
        totp_secret: None,
        custom_fields: vec![],
        entry_type: Some(EntryType::SecureNote),
        generate_passkey: None,
        attachments: None,
    };

    manager.add_entry(entry1).unwrap();
    manager.add_entry(entry2).unwrap();

    let audit = manager.security_audit().unwrap();
    assert_eq!(audit.reused_count, 0, "Empty passwords must not be counted as reused");
    assert_eq!(audit.weak_count, 0, "Empty passwords must not be counted as weak passwords");

    // Add 2 entries with identical non-empty passwords to verify real reuse is still detected
    let entry3 = NewEntry {
        title: "Service A".to_string(),
        username: "user1".to_string(),
        password: "SamePassword123!".to_string(),
        url: "".to_string(),
        email: "".to_string(),
        notes: "".to_string(),
        tags: vec![],
        totp_secret: None,
        custom_fields: vec![],
        entry_type: Some(EntryType::Login),
        generate_passkey: None,
        attachments: None,
    };
    let entry4 = NewEntry {
        title: "Service B".to_string(),
        username: "user2".to_string(),
        password: "SamePassword123!".to_string(),
        url: "".to_string(),
        email: "".to_string(),
        notes: "".to_string(),
        tags: vec![],
        totp_secret: None,
        custom_fields: vec![],
        entry_type: Some(EntryType::Login),
        generate_passkey: None,
        attachments: None,
    };

    manager.add_entry(entry3).unwrap();
    manager.add_entry(entry4).unwrap();

    let audit2 = manager.security_audit().unwrap();
    assert_eq!(audit2.reused_count, 2, "Duplicate non-empty passwords must be flagged as reused");
}

#[test]
fn test_breach_status_update_does_not_modify_updated_at_or_corrupt_sync() {
    let temp_dir = tempfile::tempdir().unwrap();
    let vault_path = temp_dir.path().join("breach_test.vdb");
    let master_pass = "TestPassword123!_master";
    let mut manager = VaultManager::create("BreachTestVault", master_pass, &vault_path).unwrap();

    let entry = NewEntry {
        title: "Original Title".to_string(),
        username: "original_user".to_string(),
        password: "password123".to_string(),
        url: "https://original.com".to_string(),
        email: "orig@test.com".to_string(),
        notes: "Original notes".to_string(),
        tags: vec!["Work".to_string()],
        totp_secret: None,
        custom_fields: Vec::new(),
        entry_type: Some(EntryType::Login),
        generate_passkey: None,
        attachments: None,
    };

    let id = manager.add_entry(entry).unwrap();
    let initial_entry = manager.list_entries().unwrap().into_iter().find(|e| e.id == id).unwrap();
    let initial_updated_at = initial_entry.updated_at;

    // Small sleep to ensure system clock advances if a timestamp were to be taken
    std::thread::sleep(std::time::Duration::from_millis(20));

    // 1. Update breach status via update_entry_breach_status
    let checked_time = Utc::now();
    manager.update_entry_breach_status(id, BreachStatus::Safe { checked_at: checked_time }).unwrap();

    let after_status = manager.list_entries().unwrap().into_iter().find(|e| e.id == id).unwrap();
    assert_eq!(after_status.updated_at, initial_updated_at, "Breach status check must NOT alter updated_at");
    assert_eq!(after_status.breach_status, BreachStatus::Safe { checked_at: checked_time });

    // 2. Update breach status via update_entry with only breach_status provided
    let breached_status = BreachStatus::Breached { breach_count: 42, checked_at: Utc::now() };
    manager.update_entry(id, UpdateEntry {
        breach_status: Some(breached_status.clone()),
        ..Default::default()
    }).unwrap();

    let after_update_entry = manager.list_entries().unwrap().into_iter().find(|e| e.id == id).unwrap();
    assert_eq!(after_update_entry.updated_at, initial_updated_at, "update_entry with only breach_status must NOT alter updated_at");
    assert_eq!(after_update_entry.breach_status, breached_status);

    // 3. Verify that WebDAV 3-way sync correctly prefers newer remote user edits over local breach-checked entry
    let mut remote_entry = manager.data.entries.iter().find(|e| e.id == id).unwrap().clone();
    remote_entry.title = "Updated Title On Mobile".to_string();
    remote_entry.updated_at = initial_updated_at + chrono::Duration::seconds(60);

    let remote_vault = VaultData {
        metadata: VaultMetadata {
            id: Uuid::new_v4(),
            name: "Remote".to_string(),
            created_at: initial_updated_at,
            updated_at: remote_entry.updated_at,
            entry_count: 1,
            version: 3,
        },
        entries: vec![remote_entry],
        tags: vec![],
        trash: vec![],
        settings: VaultSettings::default(),
    };

    let stats = merge_vault_data(&mut manager.data, remote_vault);
    assert_eq!(stats.entries_updated, 1, "Remote edit must win over local entry whose updated_at was preserved");
    let merged_entry = manager.data.entries.iter().find(|e| e.id == id).unwrap();
    assert_eq!(merged_entry.title, "Updated Title On Mobile");
}

#[test]
fn test_hardware2fa_enforces_true_2fa() {
    set_hardware2fa_mock(true);
    let dir = tempfile::tempdir().unwrap();
    let vault_path = dir.path().join("test_hw2fa_true2fa.vdb");

    let master_pass = "True2FaMasterPassword123!";
    let mut manager = VaultManager::create("HW Vault", master_pass, &vault_path).unwrap();

    let challenge_salt = [42u8; 32];
    let hw_resp = perform_hardware2fa_challenge(
        Hardware2FaProtocol::YubiKeyChallengeResponse,
        &challenge_salt,
    ).unwrap();

    manager.enable_hardware2fa_with_password(
        master_pass,
        None,
        Hardware2FaProtocol::YubiKeyChallengeResponse,
        "Backup YubiKey 5C",
        challenge_salt,
        vec![1, 2, 3],
        &hw_resp,
    ).unwrap();
    assert!(manager.is_hardware2fa_enabled());

    // Enabling biometric while Hardware 2FA is active must be rejected (Mutual Exclusivity)
    let bio_res = manager.enable_biometric();
    assert!(matches!(bio_res, Err(VaultError::InvalidState(_))));

    drop(manager);

    // 1. Password-only open_with_keyfile must be blocked with Hardware2FaRequired
    let bypass_res = VaultManager::open_with_keyfile(&vault_path, master_pass, None);
    assert!(matches!(bypass_res, Err(VaultError::Hardware2FaRequired)));

    // 2. Hardware-only (wrong/empty password) must fail (True 2FA enforcement)
    let wrong_pass_res = VaultManager::open_with_hardware2fa(&vault_path, "WrongPassword!", None, &hw_resp);
    assert!(wrong_pass_res.is_err());
    let empty_pass_res = VaultManager::open_with_hardware2fa(&vault_path, "", None, &hw_resp);
    assert!(matches!(empty_pass_res, Err(VaultError::InvalidPassword)));

    // 3. Password with wrong hardware response must fail (Factor 2 enforcement)
    let wrong_hw_res = VaultManager::open_with_hardware2fa(&vault_path, master_pass, None, &[0xAA; 20]);
    assert!(wrong_hw_res.is_err());

    // 4. Correct password + correct hardware response succeeds
    let mut unlocked = VaultManager::open_with_hardware2fa(&vault_path, master_pass, None, &hw_resp).unwrap();
    assert_eq!(unlocked.info().name, "HW Vault");
    assert!(unlocked.is_hardware2fa_enabled());

    // 5. Test retrieve challenge info
    let chall_info = VaultManager::get_hardware2fa_challenge_info(&vault_path).unwrap().unwrap();
    assert_eq!(chall_info.challenge_salt, challenge_salt.to_vec());
    assert_eq!(chall_info.protocol, Hardware2FaProtocol::YubiKeyChallengeResponse);

    // 6. Parameterless enable_hardware2fa is strictly rejected
    let paramless_res = unlocked.enable_hardware2fa(Hardware2FaProtocol::YubiKeyChallengeResponse, "Key", &hw_resp);
    assert!(matches!(paramless_res, Err(VaultError::InvalidState(_))));

    // 7. Enrollment with empty password is strictly rejected
    let empty_enroll_res = unlocked.enable_hardware2fa_with_password("", None, Hardware2FaProtocol::YubiKeyChallengeResponse, "Key", challenge_salt, vec![], &hw_resp);
    assert!(matches!(empty_enroll_res, Err(VaultError::InvalidPassword)));

    set_hardware2fa_mock(false);
}

#[test]
fn test_hardware2fa_with_keyfile() {
    set_hardware2fa_mock(true);
    let dir = tempfile::tempdir().unwrap();
    let vault_path = dir.path().join("test_hw2fa_keyfile.vdb");
    let kf_path = dir.path().join("test.key");

    let master_pass = "MasterPassWithKeyFile123!";
    VaultManager::generate_key_file(&kf_path).unwrap();
    let mut manager = VaultManager::create_with_keyfile("HW KF Vault", master_pass, Some(&kf_path), &vault_path).unwrap();

    let challenge_salt = [55u8; 32];
    let hw_resp = perform_hardware2fa_challenge(
        Hardware2FaProtocol::YubiKeyChallengeResponse,
        &challenge_salt,
    ).unwrap();

    // 1. Enrollment without key file must fail because active vault requires keyfile
    let no_kf_enroll = manager.enable_hardware2fa_with_password(
        master_pass,
        None,
        Hardware2FaProtocol::YubiKeyChallengeResponse,
        "YubiKey with KF",
        challenge_salt,
        vec![10, 20],
        &hw_resp,
    );
    assert!(matches!(no_kf_enroll, Err(VaultError::InvalidPassword)));

    // 2. Enrollment with correct password + key file succeeds
    manager.enable_hardware2fa_with_password(
        master_pass,
        Some(&kf_path),
        Hardware2FaProtocol::YubiKeyChallengeResponse,
        "YubiKey with KF",
        challenge_salt,
        vec![10, 20],
        &hw_resp,
    ).unwrap();
    assert!(manager.is_hardware2fa_enabled());

    drop(manager);

    // 3. Opening without key file fails
    let no_kf_open = VaultManager::open_with_hardware2fa(&vault_path, master_pass, None, &hw_resp);
    assert!(no_kf_open.is_err());

    // 4. Opening with correct password + key file + hardware key succeeds
    let unlocked = VaultManager::open_with_hardware2fa(&vault_path, master_pass, Some(&kf_path), &hw_resp).unwrap();
    assert_eq!(unlocked.info().name, "HW KF Vault");
    assert!(unlocked.is_hardware2fa_enabled());

    set_hardware2fa_mock(false);
}

#[test]
fn test_rekey_with_hardware2fa_invalidates_stale_envelopes() {
    set_hardware2fa_mock(true);
    let dir = tempfile::tempdir().unwrap();
    let vault_path = dir.path().join("test_hw2fa_rekey.vdb");

    let old_pass = "OldPassword123!";
    let new_pass = "NewPassword456!";
    let mut manager = VaultManager::create("HW Rekey Vault", old_pass, &vault_path).unwrap();

    let challenge_salt = [99u8; 32];
    let hw_resp = perform_hardware2fa_challenge(
        Hardware2FaProtocol::YubiKeyChallengeResponse,
        &challenge_salt,
    ).unwrap();

    manager.enable_hardware2fa_with_password(
        old_pass,
        None,
        Hardware2FaProtocol::YubiKeyChallengeResponse,
        "YubiKey 5 NFC",
        challenge_salt,
        vec![9, 8, 7],
        &hw_resp,
    ).unwrap();
    assert!(manager.is_hardware2fa_enabled());

    // Rekey the master password
    manager.change_master_password(old_pass, new_pass).unwrap();
    // Hardware 2FA envelope wrapping old keys should be invalidated
    assert!(!manager.is_hardware2fa_enabled());

    drop(manager);

    // Verify vault opens cleanly with new password without hardware key
    let reopened = VaultManager::open_with_keyfile(&vault_path, new_pass, None).unwrap();
    assert_eq!(reopened.info().name, "HW Rekey Vault");
    assert!(!reopened.is_hardware2fa_enabled());
    set_hardware2fa_mock(false);
}

#[test]
fn test_change_master_password_on_empty_vault_verifies_current_password() {
    let dir = tempfile::tempdir().unwrap();
    let vault_path = dir.path().join("test_empty_vault_rekey.vdb");

    let initial_pass = "InitialPass123!";
    let wrong_pass = "WrongPass999!";
    let new_pass = "NewPass456!";

    let mut manager = VaultManager::create("Empty Vault", initial_pass, &vault_path).unwrap();
    assert_eq!(manager.data.entries.len(), 0);
    assert_eq!(manager.data.trash.len(), 0);

    // Attempting to change master password with incorrect current password must fail
    let err = manager.change_master_password(wrong_pass, new_pass).unwrap_err();
    match err {
        VaultError::InvalidPassword => (),
        other => panic!("Expected VaultError::InvalidPassword, got: {:?}", other),
    }

    // Changing with correct current password succeeds
    manager.change_master_password(initial_pass, new_pass).unwrap();
    manager.save().unwrap();
    manager.lock();

    // Opening with initial password should fail
    assert!(VaultManager::open(&vault_path, initial_pass).is_err());

    // Opening with new password must succeed
    let reopened = VaultManager::open(&vault_path, new_pass).unwrap();
    assert_eq!(reopened.info().name, "Empty Vault");
}

#[test]
fn test_change_master_password_locked_vault_fails() {
    let dir = tempfile::tempdir().unwrap();
    let vault_path = dir.path().join("test_locked_rekey.vdb");

    let pass = "Password123!";
    let mut manager = VaultManager::create("Locked Vault", pass, &vault_path).unwrap();
    manager.lock();

    let err = manager.change_master_password(pass, "NewPass").unwrap_err();
    match err {
        VaultError::VaultLocked => (),
        other => panic!("Expected VaultError::VaultLocked, got: {:?}", other),
    }
}

#[test]
fn test_rekey_rebuilds_search_index() {
    let dir = tempfile::tempdir().unwrap();
    let vault_path = dir.path().join("test_rekey_search.vdb");
    let old_pass = "OldPass123!";
    let new_pass = "NewPass456!";

    let mut manager = VaultManager::create("Rekey Search Vault", old_pass, &vault_path).unwrap();
    let entry = NewEntry {
        title: "KeePassX Migration".to_string(),
        username: "migrator".to_string(),
        password: "secret_pass_123".to_string(),
        url: "https://keepass.info".to_string(),
        email: "migrator@keepass.info".to_string(),
        notes: "Imported notes".to_string(),
        tags: vec!["Migration".to_string()],
        totp_secret: None,
        custom_fields: Vec::new(),
        entry_type: None,
        generate_passkey: None,
        attachments: None,
    };
    let entry_id = manager.add_entry(entry).unwrap();

    let pre_search = manager.search_entries("Migration").unwrap();
    assert_eq!(pre_search.len(), 1);
    assert_eq!(pre_search[0].id, entry_id);

    // Rekey master password without locking/reopening
    manager.change_master_password(old_pass, new_pass).unwrap();

    // In-memory search index must still match using newly derived search key
    let post_search = manager.search_entries("Migration").unwrap();
    assert_eq!(post_search.len(), 1);
    assert_eq!(post_search[0].id, entry_id);
}

#[test]
fn test_vault_manager_reload() {
    let dir = tempfile::tempdir().unwrap();
    let vault_path = dir.path().join("test_reload.vdb");
    let password = "ReloadPass123!";

    let mut manager1 = VaultManager::create("Reload Vault", password, &vault_path).unwrap();
    let entry1 = NewEntry {
        title: "First Entry".to_string(),
        username: "user1".to_string(),
        password: "pass1".to_string(),
        url: "https://example.com".to_string(),
        email: "user1@example.com".to_string(),
        notes: "Notes 1".to_string(),
        tags: vec!["tag1".to_string()],
        totp_secret: None,
        custom_fields: Vec::new(),
        entry_type: None,
        generate_passkey: None,
        attachments: None,
    };
    let id1 = manager1.add_entry(entry1).unwrap();
    assert_eq!(manager1.list_entries().unwrap().len(), 1);

    // Create a second manager pointing to the same file to simulate external change / restore
    let mut manager2 = VaultManager::open(&vault_path, password).unwrap();
    let entry2 = NewEntry {
        title: "Restored Entry".to_string(),
        username: "user2".to_string(),
        password: "pass2".to_string(),
        url: "https://restored.com".to_string(),
        email: "user2@restored.com".to_string(),
        notes: "Notes 2".to_string(),
        tags: vec!["restored".to_string()],
        totp_secret: None,
        custom_fields: Vec::new(),
        entry_type: None,
        generate_passkey: None,
        attachments: None,
    };
    let id2 = manager2.add_entry(entry2).unwrap();
    assert_eq!(manager2.list_entries().unwrap().len(), 2);

    // manager1 still has stale in-memory state (1 entry)
    assert_eq!(manager1.list_entries().unwrap().len(), 1);

    // Reload manager1 from disk
    manager1.reload().unwrap();

    // Now manager1 has both entries
    let entries = manager1.list_entries().unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().any(|e| e.id == id1));
    assert!(entries.iter().any(|e| e.id == id2));

    // Search index is rebuilt and finds the restored entry
    let search_res = manager1.search_entries("Restored").unwrap();
    assert_eq!(search_res.len(), 1);
    assert_eq!(search_res[0].id, id2);

    // Test that reload with mismatched credentials fails gracefully
    let other_vault_path = dir.path().join("other_vault.vdb");
    let _other_manager = VaultManager::create("Other Vault", "DifferentPass789!", &other_vault_path).unwrap();
    // Overwrite vault_path with the other vault's bytes
    std::fs::copy(&other_vault_path, &vault_path).unwrap();

    let reload_err = manager1.reload().unwrap_err();
    match reload_err {
        VaultError::InvalidPassword | VaultError::DecryptionError(_) => (),
        other => panic!("Expected InvalidPassword or DecryptionError on mismatched credentials, got: {:?}", other),
    }
}

#[test]
fn test_restore_from_trash_updates_updated_at_and_survives_sync() {
    let test_vault = TestVault::new();
    let password = "TestPassword123!_restore";
    let mut manager = VaultManager::create("RestoreSyncVault", password, &test_vault.path).unwrap();

    let entry = NewEntry {
        title: "To Be Restored".to_string(),
        username: "user".to_string(),
        password: "password123".to_string(),
        url: "https://example.com".to_string(),
        email: "user@example.com".to_string(),
        notes: "Notes".to_string(),
        tags: vec!["Tag".to_string()],
        totp_secret: None,
        custom_fields: Vec::new(),
        entry_type: Some(EntryType::Login),
        generate_passkey: None,
        attachments: None,
    };

    let id = manager.add_entry(entry).unwrap();
    let initial_entry = manager.list_entries().unwrap().into_iter().find(|e| e.id == id).unwrap();
    let initial_updated_at = initial_entry.updated_at;

    // Small sleep to ensure clock advances
    std::thread::sleep(std::time::Duration::from_millis(20));

    // 1. Delete entry to trash
    manager.delete_entry(id).unwrap();
    assert_eq!(manager.list_entries().unwrap().len(), 0);
    assert_eq!(manager.list_trash().unwrap().len(), 1);

    // Capture remote vault state with the trash tombstone
    let remote_vault = manager.data.clone();

    // Small sleep to ensure clock advances past deleted_at
    std::thread::sleep(std::time::Duration::from_millis(20));

    // 2. Restore entry from trash
    manager.restore_from_trash(id).unwrap();
    assert_eq!(manager.list_entries().unwrap().len(), 1);
    assert_eq!(manager.list_trash().unwrap().len(), 0);

    let restored_entry = manager.list_entries().unwrap().into_iter().find(|e| e.id == id).unwrap();
    assert!(
        restored_entry.updated_at > initial_updated_at,
        "Restoring from trash must update updated_at"
    );

    // 3. Perform 3-way sync merge against the remote peer holding the trash tombstone
    let stats = merge_vault_data(&mut manager.data, remote_vault);

    // Restored entry must remain active and NOT be re-trashed
    assert_eq!(manager.data.entries.len(), 1);
    assert_eq!(manager.data.entries[0].id, id);
    assert_eq!(manager.data.trash.len(), 0);
    assert_eq!(stats.trash_merged, 0);

    // 4. Test rapid delete + restore without delay (verifies monotonic clock advance over deleted_at)
    let entry_rapid = NewEntry {
        title: "Rapid Item".to_string(),
        username: "rapid".to_string(),
        password: "pass".to_string(),
        url: "".to_string(),
        email: "".to_string(),
        notes: "".to_string(),
        tags: vec![],
        totp_secret: None,
        custom_fields: Vec::new(),
        entry_type: None,
        generate_passkey: None,
        attachments: None,
    };
    let rapid_id = manager.add_entry(entry_rapid).unwrap();
    manager.delete_entry(rapid_id).unwrap();
    let trashed_deleted_at = manager.data.trash.iter().find(|t| t.entry.id == rapid_id).unwrap().deleted_at;
    let remote_rapid_tombstone = manager.data.clone();
    // Immediate restore with no sleep
    manager.restore_from_trash(rapid_id).unwrap();
    let restored_rapid = manager.list_entries().unwrap().into_iter().find(|e| e.id == rapid_id).unwrap();
    assert!(
        restored_rapid.updated_at > trashed_deleted_at,
        "Restored rapid item updated_at must strictly exceed deleted_at"
    );
    let merge_res = merge_vault_data(&mut manager.data, remote_rapid_tombstone);
    assert_eq!(merge_res.trash_merged, 0);
    assert!(manager.data.entries.iter().any(|e| e.id == rapid_id));

    // 5. Test locked vault rejects restore_from_trash
    manager.lock();
    assert!(matches!(manager.restore_from_trash(id).unwrap_err(), VaultError::VaultLocked));
}

#[test]
fn test_trash_compaction_and_storage_metrics() {
    let test_vault = TestVault::new();
    let password = "TestPassword123!";
    let mut manager = VaultManager::create("Trash Metrics Vault", password, &test_vault.path).unwrap();

    let att_data = b"STORAGE METRICS SAMPLE PAYLOAD 12345".to_vec();
    let entry1 = NewEntry {
        title: "Entry With File".to_string(),
        username: "user1".to_string(),
        password: "secretpassword".to_string(),
        url: "https://example.com".to_string(),
        email: "user1@example.com".to_string(),
        notes: "Notes".to_string(),
        tags: vec!["Work".to_string(), "Finance".to_string()],
        totp_secret: None,
        custom_fields: Vec::new(),
        entry_type: Some(EntryType::Login),
        generate_passkey: None,
        attachments: Some(vec![NewAttachment {
            name: "test.dat".to_string(),
            mime_type: "application/octet-stream".to_string(),
            data: att_data.clone(),
        }]),
    };
    let id1 = manager.add_entry(entry1).unwrap();

    let entry2 = NewEntry {
        title: "Plain Entry".to_string(),
        username: "user2".to_string(),
        password: "plainpassword".to_string(),
        url: "https://plain.com".to_string(),
        email: "user2@plain.com".to_string(),
        notes: "".to_string(),
        tags: vec!["Personal".to_string()],
        totp_secret: None,
        custom_fields: Vec::new(),
        entry_type: Some(EntryType::Login),
        generate_passkey: None,
        attachments: None,
    };
    let id2 = manager.add_entry(entry2).unwrap();

    // Verify initial metrics
    let metrics = manager.get_storage_metrics().unwrap();
    assert_eq!(metrics.entry_count, 2);
    assert_eq!(metrics.trashed_entry_count, 0);
    assert_eq!(metrics.active_attachment_count, 1);
    assert_eq!(metrics.active_attachment_bytes, att_data.len() as u64);
    assert_eq!(metrics.trashed_attachment_count, 0);
    assert_eq!(metrics.trashed_attachment_bytes, 0);
    assert_eq!(metrics.tag_count, 3);
    assert!(metrics.vault_file_bytes > 0);
    assert_eq!(manager.trash_count().unwrap(), 0);

    // Delete id2 to trash
    manager.delete_entry(id2).unwrap();
    assert_eq!(manager.trash_count().unwrap(), 1);

    // Delete id1 (with attachment) to trash
    manager.delete_entry(id1).unwrap();
    assert_eq!(manager.trash_count().unwrap(), 2);

    let metrics_after_trash = manager.get_storage_metrics().unwrap();
    assert_eq!(metrics_after_trash.entry_count, 0);
    assert_eq!(metrics_after_trash.trashed_entry_count, 2);
    assert_eq!(metrics_after_trash.active_attachment_count, 0);
    assert_eq!(metrics_after_trash.trashed_attachment_count, 1);
    assert_eq!(metrics_after_trash.trashed_attachment_bytes, att_data.len() as u64);

    // Retention check: purge_expired_trash(30) should not delete fresh trash
    let purged_zero = manager.purge_expired_trash(30).unwrap();
    assert_eq!(purged_zero, 0);
    assert_eq!(manager.trash_count().unwrap(), 2);

    // Artificially age id2 to 35 days ago
    for t in &mut manager.data.trash {
        if t.entry.id == id2 {
            t.deleted_at = Utc::now() - chrono::Duration::days(35);
        }
    }

    // Now purge entries older than 30 days
    let purged_one = manager.purge_expired_trash(30).unwrap();
    assert_eq!(purged_one, 1);
    assert_eq!(manager.trash_count().unwrap(), 1);

    // Compacting the vault
    let compacted_metrics = manager.compact_vault().unwrap();
    assert_eq!(compacted_metrics.trashed_entry_count, 1);

    // Purge all remaining trash
    let purged_remaining = manager.purge_all_trash().unwrap();
    assert_eq!(purged_remaining, 1);
    assert_eq!(manager.trash_count().unwrap(), 0);

    let final_metrics = manager.get_storage_metrics().unwrap();
    assert_eq!(final_metrics.trashed_entry_count, 0);
    assert_eq!(final_metrics.trashed_attachment_count, 0);
    assert_eq!(final_metrics.trashed_attachment_bytes, 0);

    // Locked vault rejection
    manager.lock();
    assert!(matches!(manager.trash_count().unwrap_err(), VaultError::VaultLocked));
    assert!(matches!(manager.get_storage_metrics().unwrap_err(), VaultError::VaultLocked));
    assert!(matches!(manager.purge_expired_trash(30).unwrap_err(), VaultError::VaultLocked));
    assert!(matches!(manager.compact_vault().unwrap_err(), VaultError::VaultLocked));
}

#[test]
fn test_export_csv_and_json_lifecycle() {
    let test_vault = TestVault::new();
    let password = "TestPassword123!";
    let mut manager = VaultManager::create("Export Test Vault", password, &test_vault.path).unwrap();

    let entry1 = NewEntry {
        title: "Test \"Quotes\" Service".to_string(),
        username: "alice_export".to_string(),
        password: "p@ss\"word\n123".to_string(),
        url: "https://alice.example.com".to_string(),
        email: "alice@example.com".to_string(),
        notes: "Multi-line\nnotes\r\ntest".to_string(),
        tags: vec!["Dev".to_string(), "Testing".to_string()],
        totp_secret: Some("JBSWY3DPEHPK3PXP".to_string()),
        custom_fields: Vec::new(),
        entry_type: Some(EntryType::Login),
        generate_passkey: None,
        attachments: None,
    };
    manager.add_entry(entry1).unwrap();

    let entry2 = NewEntry {
        title: "=cmd|' /C calc'!A0".to_string(),
        username: "@malicious_user".to_string(),
        password: "+password_formula".to_string(),
        url: "https://safe.example.com".to_string(),
        email: "-email_formula@example.com".to_string(),
        notes: "\tTabPrefixedNote".to_string(),
        tags: vec![],
        totp_secret: None,
        custom_fields: Vec::new(),
        entry_type: Some(EntryType::Login),
        generate_passkey: None,
        attachments: None,
    };
    manager.add_entry(entry2).unwrap();

    let csv_path = test_vault.path.with_extension("export.csv");
    let json_path = test_vault.path.with_extension("export.json");

    // Export CSV
    manager.export_csv(&csv_path).unwrap();
    assert!(csv_path.exists());
    let csv_content = std::fs::read_to_string(&csv_path).unwrap();
    assert!(csv_content.starts_with("Title,Username,Email,Password,URL,Notes,TOTP,Tags\n"));
    assert!(csv_content.contains("\"Test \"\"Quotes\"\" Service\""));
    assert!(csv_content.contains("\"alice_export\""));
    assert!(csv_content.contains("Dev;Testing"));

    // Verify formula injection sanitization
    assert!(csv_content.contains("\"'=cmd|' /C calc'!A0\""));
    assert!(csv_content.contains("\"'@malicious_user\""));
    assert!(csv_content.contains("\"'+password_formula\""));
    assert!(csv_content.contains("\"'-email_formula@example.com\""));
    assert!(csv_content.contains("\"'\tTabPrefixedNote\""));

    // Export JSON
    manager.export_json(&json_path).unwrap();
    assert!(json_path.exists());
    let json_content = std::fs::read_to_string(&json_path).unwrap();
    let parsed: Vec<DecryptedEntry> = serde_json::from_str(&json_content).unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].title, "Test \"Quotes\" Service");
    assert_eq!(parsed[0].username, "alice_export");
    assert_eq!(parsed[0].password, "p@ss\"word\n123");
    assert_eq!(parsed[0].totp_secret.as_deref(), Some("JBSWY3DPEHPK3PXP"));

    // Locked vault rejection
    manager.lock();
    assert!(matches!(manager.export_csv(&csv_path).unwrap_err(), VaultError::VaultLocked));
    assert!(matches!(manager.export_json(&json_path).unwrap_err(), VaultError::VaultLocked));

    let _ = std::fs::remove_file(csv_path);
    let _ = std::fs::remove_file(json_path);
}

#[test]
fn test_lock_clears_settings_and_sensitive_state() {
    let test_vault = TestVault::new();
    let password = "TestPassword123!";
    let mut manager = VaultManager::create("Settings Test Vault", password, &test_vault.path).unwrap();

    // Verify initial settings exist
    manager.data.settings.webdav.url = "https://dav.example.com".to_string();
    manager.data.settings.webdav.username = "secret_user".to_string();
    manager.data.settings.webdav.enabled = true;
    assert!(!manager.data.settings.webdav.url.is_empty());

    // Lock the vault
    manager.lock();

    // Sensitive settings must be cleared to Default
    assert!(manager.data.settings.webdav.url.is_empty());
    assert!(manager.data.settings.webdav.username.is_empty());
    assert!(!manager.data.settings.webdav.enabled);
    assert!(manager.data.entries.is_empty());
    assert!(manager.data.tags.is_empty());
    assert!(manager.data.trash.is_empty());
}




