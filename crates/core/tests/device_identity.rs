use yntra_vault_core::services::sync::pairing::{initialize_local_device_identity, resolve_local_device_info};

#[test]
fn persisted_device_identity_is_independent_of_display_name() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("device-id");
    let id = uuid::Uuid::new_v4();
    std::fs::write(&path, id.to_string()).unwrap();
    initialize_local_device_identity(&path).unwrap();
    assert_eq!(resolve_local_device_info(Some("Original name")).id, id);
    assert_eq!(resolve_local_device_info(Some("New name")).id, id);
    assert_eq!(std::fs::read_to_string(path).unwrap(), id.to_string());
}

#[test]
fn equally_named_installations_remain_visible_and_independently_revocable() {
    use yntra_vault_core::vault::{VaultManager, TrustedDevice};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("same-named-devices.vdb");
    let mut manager = VaultManager::create("Devices", "Synthetic-Device-Test-2026!", &path).unwrap();
    let first = TrustedDevice { id: uuid::Uuid::new_v4(), name: "Shared computer".into(), os: "Windows".into(), signing_public_key: vec![1;32], ..Default::default() };
    let second = TrustedDevice { id: uuid::Uuid::new_v4(), signing_public_key: vec![2;32], ..first.clone() };
    manager.register_trusted_device(first.clone()).unwrap();
    manager.register_trusted_device(second.clone()).unwrap();
    assert_eq!(manager.get_trusted_devices().len(), 2);
    let devices = manager.get_trusted_devices();
    let first_id = devices.iter().find(|device| device.signing_public_key == first.signing_public_key).unwrap().id;
    let second_migrated = devices.iter().find(|device| device.signing_public_key == second.signing_public_key).unwrap().clone();
    manager.revoke_trusted_device(first_id).unwrap();
    let reopened = VaultManager::open(&path, "Synthetic-Device-Test-2026!").unwrap();
    assert_eq!(reopened.get_trusted_devices(), vec![second_migrated]);
}
