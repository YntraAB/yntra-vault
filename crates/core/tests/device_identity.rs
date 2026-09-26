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
