//! Integration tests for yntra-vault-app (Tauri app lib and IPC state).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use uuid::Uuid;

use yntra_vault_app_lib::{commands, AppState};
use yntra_vault_core::generator::GeneratorOptions;
use yntra_vault_core::vault::types::StrengthLevel;
use yntra_vault_core::vault::VaultManager;

#[tokio::test]
async fn test_app_state_and_atomic_settings() {
    let state = AppState {
        vault: Mutex::new(None),
        minimize_to_tray: AtomicBool::new(true),
        lock_on_focus_loss: AtomicBool::new(false),
        lock_on_system_lock: AtomicBool::new(true),
        smart_login_cancel: std::sync::Arc::new(AtomicBool::new(false)),
    };

    // Verify initial settings defaults
    assert!(state.minimize_to_tray.load(Ordering::SeqCst));
    assert!(!state.lock_on_focus_loss.load(Ordering::SeqCst));
    assert!(state.lock_on_system_lock.load(Ordering::SeqCst));
    assert!(state.vault.lock().unwrap().is_none());

    // Update settings
    state.minimize_to_tray.store(false, Ordering::SeqCst);
    state.lock_on_focus_loss.store(true, Ordering::SeqCst);
    state.lock_on_system_lock.store(false, Ordering::SeqCst);

    assert!(!state.minimize_to_tray.load(Ordering::SeqCst));
    assert!(state.lock_on_focus_loss.load(Ordering::SeqCst));
    assert!(!state.lock_on_system_lock.load(Ordering::SeqCst));
}

#[tokio::test]
async fn test_password_generation_and_strength_commands() {
    // Test default password generation
    let pwd = commands::generate_password_default().await.unwrap();
    assert!(!pwd.is_empty());
    assert!(pwd.len() >= 16);

    // Test custom password generation options
    let custom_opts = GeneratorOptions {
        length: 24,
        uppercase: true,
        lowercase: true,
        digits: true,
        symbols: true,
        ..Default::default()
    };
    let custom_pwd = commands::generate_password(custom_opts).await.unwrap();
    assert_eq!(custom_pwd.len(), 24);

    // Test password strength analysis
    let score = commands::analyze_password_strength("Correct-Horse-Battery-Staple-2026!".to_string())
        .await
        .unwrap();
    assert!(matches!(score.level, StrengthLevel::Strong | StrengthLevel::Excellent));
}

#[tokio::test]
async fn test_totp_commands_roundtrip() {
    let uri = "otpauth://totp/YntraTest:alice@example.com?secret=JBSWY3DPEHPK3PXP&issuer=YntraTest";
    let parsed = commands::parse_otpauth_uri(uri.to_string()).await.unwrap();
    assert_eq!(parsed.secret, "JBSWY3DPEHPK3PXP");
    assert_eq!(parsed.issuer.as_deref(), Some("YntraTest"));
    assert_eq!(parsed.label.as_deref(), Some("YntraTest:alice@example.com"));

    let totp_code = commands::generate_totp("JBSWY3DPEHPK3PXP".to_string()).await.unwrap();
    assert_eq!(totp_code.code.len(), 6);
    assert!(totp_code.code.chars().all(|c| c.is_ascii_digit()));
    assert!(totp_code.seconds_remaining <= 30);
}

#[tokio::test]
async fn test_app_state_vault_lifecycle() {
    let temp_dir = tempfile::tempdir().unwrap();
    let vault_path = temp_dir.path().join(format!("tauri_test_{}.vdb", Uuid::new_v4()));
    let master_password = "TauriAppTestPassword123!";

    let state = AppState {
        vault: Mutex::new(None),
        minimize_to_tray: AtomicBool::new(true),
        lock_on_focus_loss: AtomicBool::new(false),
        lock_on_system_lock: AtomicBool::new(true),
        smart_login_cancel: std::sync::Arc::new(AtomicBool::new(false)),
    };

    // 1. Create and mount vault into AppState
    let manager = VaultManager::create("Tauri Vault", master_password, &vault_path).unwrap();
    assert!(manager.is_unlocked());

    {
        let mut vault_guard = state.vault.lock().unwrap();
        *vault_guard = Some(manager);
    }

    // 2. Verify vault is present and unlocked in AppState
    {
        let vault_guard = state.vault.lock().unwrap();
        let mgr = vault_guard.as_ref().unwrap();
        assert!(mgr.is_unlocked());
        assert_eq!(mgr.metadata().name, "Tauri Vault");
    }

    // 3. Lock vault and unmount from AppState
    {
        let mut vault_guard = state.vault.lock().unwrap();
        if let Some(ref mut mgr) = *vault_guard {
            mgr.lock();
        }
        *vault_guard = None;
    }

    // 4. Verify state is locked
    {
        let vault_guard = state.vault.lock().unwrap();
        assert!(vault_guard.is_none());
    }

    // 5. Reopen and remount
    let reopened = VaultManager::open(&vault_path, master_password).unwrap();
    {
        let mut vault_guard = state.vault.lock().unwrap();
        *vault_guard = Some(reopened);
    }

    {
        let vault_guard = state.vault.lock().unwrap();
        assert!(vault_guard.as_ref().unwrap().is_unlocked());
    }
}

#[tokio::test]
async fn test_app_state_vault_reload() {
    let temp_dir = tempfile::tempdir().unwrap();
    let vault_path = temp_dir.path().join(format!("tauri_reload_{}.vdb", Uuid::new_v4()));
    let master_password = "TauriAppTestPassword123!";

    let state = AppState {
        vault: Mutex::new(None),
        minimize_to_tray: AtomicBool::new(true),
        lock_on_focus_loss: AtomicBool::new(false),
        lock_on_system_lock: AtomicBool::new(true),
        smart_login_cancel: std::sync::Arc::new(AtomicBool::new(false)),
    };

    // 1. Create vault with 1 entry
    let mut manager1 = VaultManager::create("Tauri Reload Vault", master_password, &vault_path).unwrap();
    let entry1 = yntra_vault_core::vault::manager::NewEntry {
        title: "Entry 1".to_string(),
        username: "user1".to_string(),
        password: "pass1".to_string(),
        url: "https://example.com".to_string(),
        email: "user1@example.com".to_string(),
        notes: "Notes 1".to_string(),
        tags: vec![],
        totp_secret: None,
        custom_fields: Vec::new(),
        entry_type: None,
        generate_passkey: None,
        attachments: None,
    };
    manager1.add_entry(entry1).unwrap();
    assert_eq!(manager1.list_entries().unwrap().len(), 1);

    // Mount into AppState
    *state.vault.lock().unwrap() = Some(manager1);

    // 2. Simulate external update/restore by modifying the file on disk
    let mut manager2 = VaultManager::open(&vault_path, master_password).unwrap();
    let entry2 = yntra_vault_core::vault::manager::NewEntry {
        title: "Entry 2".to_string(),
        username: "user2".to_string(),
        password: "pass2".to_string(),
        url: "https://restored.com".to_string(),
        email: "user2@restored.com".to_string(),
        notes: "Notes 2".to_string(),
        tags: vec![],
        totp_secret: None,
        custom_fields: Vec::new(),
        entry_type: None,
        generate_passkey: None,
        attachments: None,
    };
    manager2.add_entry(entry2).unwrap();

    // 3. Confirm AppState is still holding the stale 1-entry state
    {
        let vault_guard = state.vault.lock().unwrap();
        assert_eq!(vault_guard.as_ref().unwrap().list_entries().unwrap().len(), 1);
    }

    // 4. Reload AppState from disk
    {
        let mut vault_guard = state.vault.lock().unwrap();
        vault_guard.as_mut().unwrap().reload().unwrap();
    }

    // 5. Confirm AppState now reflects both entries
    {
        let vault_guard = state.vault.lock().unwrap();
        assert_eq!(vault_guard.as_ref().unwrap().list_entries().unwrap().len(), 2);
    }
}

#[tokio::test]
async fn test_autotype_lock_release_invariant() {
    let temp_dir = tempfile::tempdir().unwrap();
    let vault_path = temp_dir.path().join(format!("tauri_autotype_{}.vdb", Uuid::new_v4()));
    let master_password = "TauriAppTestPassword123!";

    let state = AppState {
        vault: Mutex::new(None),
        minimize_to_tray: AtomicBool::new(true),
        lock_on_focus_loss: AtomicBool::new(true),
        lock_on_system_lock: AtomicBool::new(true),
        smart_login_cancel: std::sync::Arc::new(AtomicBool::new(false)),
    };

    let mut manager = VaultManager::create("Tauri Autotype Vault", master_password, &vault_path).unwrap();
    let entry_id = manager
        .add_entry(yntra_vault_core::vault::manager::NewEntry {
            title: "Test Login".to_string(),
            username: "alice".to_string(),
            password: "super-secret-password".to_string(),
            url: "https://example.com/login".to_string(),
            email: "alice@example.com".to_string(),
            notes: String::new(),
            tags: vec![],
            totp_secret: Some("JBSWY3DPEHPK3PXP".to_string()),
            custom_fields: Vec::new(),
            entry_type: None,
            generate_passkey: None,
            attachments: None,
        })
        .unwrap();

    *state.vault.lock().unwrap() = Some(manager);

    // 1. Verify single password extraction releases vault lock immediately
    let pw = commands::extract_entry_password(&entry_id.to_string(), &state).unwrap();
    assert_eq!(&*pw, "super-secret-password");

    // The vault lock must NOT be held after extraction
    let lock_attempt = state.vault.try_lock();
    assert!(lock_attempt.is_ok(), "Vault mutex must be dropped immediately after extraction");
    drop(lock_attempt);

    // 2. Verify smart autotype credential extraction releases vault lock immediately
    let (username, password, totp_sec, url) =
        commands::extract_entry_autotype_smart(&entry_id.to_string(), &state).unwrap();
    assert_eq!(username, "alice");
    assert_eq!(password, "super-secret-password");
    assert_eq!(totp_sec, "JBSWY3DPEHPK3PXP");
    assert_eq!(url, "https://example.com/login");

    // The vault lock must NOT be held after smart credential extraction
    let lock_attempt2 = state.vault.try_lock();
    assert!(lock_attempt2.is_ok(), "Vault mutex must be dropped immediately after smart extraction");
    drop(lock_attempt2);

    // 3. Verify that focus loss handler can lock vault concurrently without deadlock
    if state.lock_on_focus_loss.load(Ordering::Relaxed) {
        if let Ok(mut vault) = state.vault.lock() {
            if let Some(ref mut mgr) = *vault {
                mgr.lock();
            }
            *vault = None;
        }
    }
    assert!(state.vault.lock().unwrap().is_none());
}

