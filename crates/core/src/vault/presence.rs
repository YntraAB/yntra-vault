//! Non-secret snapshots for checking removable storage outside the vault mutex.
use super::{VaultManager, usb::UsbDevice};
use std::path::PathBuf;
use subtle::ConstantTimeEq;

#[derive(Clone, PartialEq, Eq)]
pub struct PresenceSnapshot {
    session_id: uuid::Uuid,
    path: PathBuf,
    usb: Option<([u8; 32], [u8; 32])>,
}

impl VaultManager {
    pub fn presence_snapshot(&self) -> Option<PresenceSnapshot> {
        self.is_unlocked().then(|| PresenceSnapshot {
            session_id: self.session_id,
            path: self.path.clone(),
            usb: self
                .storage
                .as_ref()
                .and_then(|s| s.header.usb_marker.map(|marker| (s.header.salt, marker))),
        })
    }
}

impl PresenceSnapshot {
    pub fn matches(&self, manager: &VaultManager) -> bool {
        manager.presence_snapshot().as_ref() == Some(self)
    }

    pub fn file_present(&self) -> bool {
        // Errors and directories fail closed. A leftover .tmp file grants no access.
        std::fs::metadata(&self.path).is_ok_and(|metadata| metadata.is_file())
    }

    pub fn requires_usb(&self) -> bool {
        self.usb.is_some()
    }

    pub fn usb_present(&self) -> bool {
        if !self.requires_usb() {
            return true;
        }
        super::usb::list_usb_devices().is_ok_and(|devices| self.matches_devices(&devices))
    }

    fn matches_devices(&self, devices: &[UsbDevice]) -> bool {
        let Some((salt, marker)) = self.usb else {
            return true;
        };
        devices
            .iter()
            .filter(|device| {
                bool::from(marker.ct_eq(&super::storage::usb_marker(&salt, &device.serial)))
            })
            .count()
            == 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usb_presence_requires_exactly_one_bound_device() {
        let salt = [7; 32];
        let mut snapshot = PresenceSnapshot {
            session_id: uuid::Uuid::new_v4(),
            path: "unused".into(),
            usb: Some((
                salt,
                super::super::storage::usb_marker(&salt, "TEST-BOUND-USB"),
            )),
        };
        let device = |serial: &str| UsbDevice {
            id: "synthetic".into(),
            name: "fixture".into(),
            serial: serial.into(),
        };
        assert!(snapshot.matches_devices(&[device("TEST-BOUND-USB")]));
        assert!(!snapshot.matches_devices(&[]));
        assert!(!snapshot.matches_devices(&[device("TEST-OTHER-USB")]));
        assert!(!snapshot.matches_devices(&[device("TEST-BOUND-USB"), device("TEST-BOUND-USB")]));
        snapshot.usb = None;
        assert!(snapshot.usb_present()); // No real enumeration for unbound vaults.
    }

    #[test]
    fn file_loss_cannot_be_hidden_by_leftover_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.vdb");
        let snapshot = PresenceSnapshot {
            session_id: uuid::Uuid::new_v4(),
            path: path.clone(),
            usb: None,
        };
        std::fs::write(&path, b"synthetic").unwrap();
        assert!(snapshot.file_present());
        std::fs::write(path.with_extension("vdb.tmp"), b"leftover").unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!(!snapshot.file_present());
        std::fs::create_dir(&path).unwrap();
        assert!(!snapshot.file_present());
    }
}
