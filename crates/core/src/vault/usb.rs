//! Physical USB identifiers. These are copy deterrents, not secret hardware credentials.

use crate::error::VaultError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct UsbDevice {
    pub id: String,
    pub name: String,
    #[serde(skip)]
    pub(crate) serial: String,
}

pub(crate) fn normalize_serial(value: &str) -> crate::Result<String> {
    let value = value.trim().to_ascii_uppercase();
    if value.len() < 6
        || value.len() > 256
        || !value.is_ascii()
        || value.bytes().any(|b| b.is_ascii_control())
        || value.chars().all(|c| c == '0' || c == ' ')
        || ["UNKNOWN", "DEFAULT", "NONE", "123456789", "1234567890"].contains(&value.as_str())
    {
        return Err(VaultError::InvalidState(
            "USB device has no usable hardware serial number".into(),
        ));
    }
    Ok(value)
}

pub fn list_usb_devices() -> crate::Result<Vec<UsbDevice>> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use std::process::{Command, Stdio};
        #[derive(Deserialize)]
        struct Disk {
            #[serde(rename = "SerialNumber")]
            serial: Option<String>,
            #[serde(rename = "Model")]
            model: Option<String>,
        }
        // Fixed script, no user input. CIM enumerates present physical disks, not volume IDs.
        let script = "@{disks=@(Get-CimInstance Win32_DiskDrive -Filter \"InterfaceType='USB'\" -ErrorAction Stop | Select-Object SerialNumber,Model)} | ConvertTo-Json -Compress";
        let system_root = std::env::var_os("SystemRoot").ok_or_else(|| {
            VaultError::InvalidState("Windows system directory unavailable".into())
        })?;
        let executable = std::path::PathBuf::from(system_root)
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let mut child = Command::new(executable)
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                script,
            ])
            .creation_flags(0x08000000)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let start = std::time::Instant::now();
        loop {
            if child.try_wait()?.is_some() {
                break;
            }
            if start.elapsed() > std::time::Duration::from_secs(10) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(VaultError::InvalidState("USB detection timed out".into()));
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        let output = child.wait_with_output()?;
        if !output.status.success() || output.stdout.len() > 65536 {
            return Err(VaultError::InvalidState("USB detection failed".into()));
        }
        #[derive(Deserialize)]
        struct Disks {
            disks: Vec<Disk>,
        }
        let disks: Disks = serde_json::from_slice(&output.stdout)
            .map_err(|_| VaultError::InvalidState("USB detection returned invalid data".into()))?;
        let mut devices = Vec::new();
        for disk in disks.disks {
            if let Ok(serial) = normalize_serial(disk.serial.as_deref().unwrap_or_default()) {
                let id = blake3::derive_key("yntra-usb-selection-v1", serial.as_bytes());
                devices.push(UsbDevice {
                    id: data_encoding::HEXLOWER.encode(&id),
                    name: disk.model.unwrap_or_else(|| "USB storage".into()),
                    serial,
                });
            }
        }
        // Duplicate identifiers cannot distinguish the connected devices.
        let counts = devices
            .iter()
            .fold(std::collections::HashMap::new(), |mut map, d| {
                *map.entry(d.id.clone()).or_insert(0usize) += 1;
                map
            });
        devices.retain(|d| counts.get(&d.id) == Some(&1));
        Ok(devices)
    }
    #[cfg(not(windows))]
    {
        Ok(Vec::new())
    }
}

pub(crate) fn selected_serial(id: &str) -> crate::Result<String> {
    list_usb_devices()?
        .into_iter()
        .find(|d| d.id == id)
        .map(|d| d.serial)
        .ok_or_else(|| {
            VaultError::InvalidState("Selected USB device is absent or unsupported".into())
        })
}
