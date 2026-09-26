//! Explicit real-device smoke test. Uses only a newly created test directory.
use std::path::PathBuf;
use yntra_vault_core::vault::VaultManager;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parent = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Supply the USB test parent directory")?,
    );
    if !parent.is_dir() {
        return Err("Test parent must exist".into());
    }
    let devices = yntra_vault_core::vault::usb::list_usb_devices()?;
    if devices.len() != 1 {
        return Err("Expected exactly one supported connected USB storage device".into());
    }
    let test = tempfile::Builder::new()
        .prefix("yntra-usb-probe-")
        .tempdir_in(&parent)?;
    let local = tempfile::tempdir()?;
    let path = test.path().join("probe.vdb");
    let mut manager = VaultManager::create("USB probe", "test-only password 024", &path)?;
    let kit = manager.generate_emergency_kit("test-only password 024")?;
    manager.set_usb_binding("test-only password 024", None, Some(&devices[0].id))?;
    manager.lock();
    let mut reopened = VaultManager::open(&path, "test-only password 024")?;
    reopened.data.metadata.name = "USB probe updated".into();
    reopened.save()?;
    let copied = local.path().join("renamed.vdb");
    std::fs::copy(&path, &copied)?;
    let mut copied_manager = VaultManager::open(&copied, "test-only password 024")?;
    copied_manager.change_master_password("test-only password 024", "changed test password 024")?;
    std::fs::copy(&copied, &path)?;
    let restored = VaultManager::open(&path, "changed test password 024")?;
    assert!(restored.protection_info().usb_bound);
    let recovered = VaultManager::recover_with_shares(
        &path,
        &kit.shares[0].share_data,
        &kit.shares[2].share_data,
        "recovered test password 024",
    )?;
    assert!(!recovered.protection_info().usb_bound);
    assert_eq!(recovered.data.metadata.name, "USB probe updated");
    println!(
        "PASS: hardware discovery, USB enrollment, reopen, rename, copy to computer and back, password change preserving binding, and recovery using the original kit. Existing files untouched."
    );
    Ok(())
}
