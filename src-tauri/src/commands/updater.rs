//! Updater commands for Yntra Vault Desktop and Mobile.

use tauri::{AppHandle, Manager};
use std::sync::atomic::{AtomicBool, Ordering};
use yntra_vault_core::services::updater::{
    check_for_updates, download_file, verify_sha256, CheckUpdateResult,
};

static INSTALLING: AtomicBool = AtomicBool::new(false);
struct InstallGuard;
impl InstallGuard {
    fn acquire() -> Result<Self, String> {
        INSTALLING.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self).map_err(|_| "An update is already being prepared".into())
    }
}
impl Drop for InstallGuard {
    fn drop(&mut self) { INSTALLING.store(false, Ordering::Release); }
}

#[cfg(target_os = "android")]
pub(crate) struct AndroidUpdateInstaller(pub tauri::plugin::PluginHandle<tauri::Wry>);

#[cfg(target_os = "android")]
pub(crate) fn android_installer_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("yntra-update-installer")
        .setup(|app, api| {
            let handle = api.register_android_plugin("com.yntravault.app", "UpdateInstallerPlugin")?;
            app.manage(AndroidUpdateInstaller(handle));
            Ok(())
        }).build()
}

/// Get the current build version of Yntra Vault
#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Check for available application updates
#[tauri::command]
pub async fn check_app_update(
    custom_endpoint: Option<String>,
) -> Result<CheckUpdateResult, String> {
    let current_version = env!("CARGO_PKG_VERSION");

    // Detect target platform
    let target_platform = if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_os = "windows") {
        // Detect if running as portable standalone executable
        let is_portable = std::env::current_exe()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()))
            .map(|name| name.contains("portable"))
            .unwrap_or(false);

        if is_portable {
            "windows-portable"
        } else {
            "windows-x86_64"
        }
    } else if cfg!(target_os = "linux") {
        "linux-x86_64"
    } else if cfg!(target_os = "macos") {
        #[cfg(target_arch = "aarch64")]
        {
            "darwin-aarch64"
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            "darwin-x86_64"
        }
    } else {
        "unknown"
    };

    check_for_updates(
        current_version,
        target_platform,
        custom_endpoint.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())
}

/// Download Android APK to cache and trigger system package installer
#[tauri::command]
pub async fn download_and_install_apk(
    apk_url: String,
    expected_sha256: Option<String>,
    app: AppHandle,
) -> Result<String, String> {
    // 1. Enforce mandatory SHA-256 verification
    let expected_sha = match expected_sha256 {
        Some(ref s) if !s.trim().is_empty() => s.trim(),
        _ => {
            return Err(
                "Cryptographic verification failed: Mandatory SHA-256 hash missing from update manifest".to_string(),
            );
        }
    };

    if !cfg!(target_os = "android") {
        return Err("APK installation is only available on Android".into());
    }
    let _guard = InstallGuard::acquire()?;
    let _version = verify_official_asset("android", &apk_url, expected_sha).await?;

    // 2. Download APK bytes
    let bytes = download_file(&apk_url).await.map_err(|e| e.to_string())?;

    // 3. Verify SHA-256 in constant time before saving to filesystem
    if !verify_sha256(&bytes, expected_sha) {
        return Err("Cryptographic verification failed: APK SHA-256 hash mismatch".to_string());
    }

    // 4. Save APK file to cache / temp directory
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?.join("updates");
    std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;

    // A content-addressed file cannot be overwritten by a later update while
    // Android's external installer still holds its URI. Publish atomically.
    let apk_path = stage_apk(&cache_dir, expected_sha, &bytes)?;

    let apk_path_str = apk_path.to_string_lossy().to_string();

    // 5. Android rechecks identity, version and digest before granting read access.
    #[cfg(target_os = "android")]
    {
        app.state::<AndroidUpdateInstaller>().0
            .run_mobile_plugin::<()>("install", serde_json::json!({
                "path": apk_path_str, "sha256": expected_sha.to_ascii_lowercase(), "version": _version
            }))
            .map_err(|e| e.to_string())?;
    }

    Ok(apk_path_str)
}

/// Install Portable Windows update in-place
#[tauri::command]
pub async fn install_portable_update(
    url: String,
    expected_sha256: Option<String>,
) -> Result<(), String> {
    // 1. Enforce mandatory SHA-256 verification
    let expected_sha = match expected_sha256 {
        Some(ref s) if !s.trim().is_empty() => s.trim(),
        _ => {
            return Err(
                "Cryptographic verification failed: Mandatory SHA-256 hash missing from update manifest".to_string(),
            );
        }
    };

    if !cfg!(target_os = "windows") {
        return Err("Portable Windows updates are only available on Windows".into());
    }
    let _guard = InstallGuard::acquire()?;
    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    if !current_exe.file_name().is_some_and(|name| name.to_string_lossy().to_ascii_lowercase().contains("portable")) {
        return Err("Use the installer to update an installed desktop application".into());
    }
    verify_official_asset("windows-portable", &url, expected_sha).await?;

    // 2. Download binary payload
    let bytes = download_file(&url).await.map_err(|e| e.to_string())?;

    // 3. Verify SHA-256 in constant time before touching binary
    if !verify_sha256(&bytes, expected_sha) {
        return Err(
            "Cryptographic verification failed: Portable executable SHA-256 mismatch".to_string(),
        );
    }

    yntra_vault_core::services::updater::replace_executable(&current_exe, &bytes).map_err(|e| e.to_string())
}

fn stage_apk(directory: &std::path::Path, hash: &str, bytes: &[u8]) -> Result<std::path::PathBuf, String> {
    use std::io::Write;
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) || !verify_sha256(bytes, hash) {
        return Err("Invalid update package checksum".into());
    }
    let path = directory.join(format!("{}.apk", hash.to_ascii_lowercase()));
    if path.exists() {
        let existing = std::fs::read(&path).map_err(|e| e.to_string())?;
        if !verify_sha256(&existing, hash) { return Err("Cached update package is damaged".into()); }
        return Ok(path);
    }
    let mut file = tempfile::NamedTempFile::new_in(directory).map_err(|e| e.to_string())?;
    file.write_all(bytes).and_then(|_| file.as_file().sync_all()).map_err(|e| e.to_string())?;
    file.persist_noclobber(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

async fn verify_official_asset(platform: &str, url: &str, sha256: &str) -> Result<String, String> {
    if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid SHA-256 checksum".into());
    }
    let update = check_for_updates(env!("CARGO_PKG_VERSION"), platform, None)
        .await.map_err(|e| e.to_string())?;
    if !update.has_update || update.download_url.as_deref() != Some(url)
        || !update.sha256.as_deref().is_some_and(|hash| hash.eq_ignore_ascii_case(sha256)) {
        return Err("Package does not match the official update manifest".into());
    }
    Ok(update.latest_version.trim_start_matches(['v', 'V']).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_update_is_atomic_content_addressed_and_not_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let hash = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let path = stage_apk(directory.path(), hash, b"abc").unwrap();
        assert_eq!(stage_apk(directory.path(), hash, b"abc").unwrap(), path);
        assert!(stage_apk(directory.path(), hash, b"different").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"abc");
        std::fs::write(&path, b"damaged").unwrap();
        assert!(stage_apk(directory.path(), hash, b"abc").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"damaged");
    }

    #[test]
    fn update_preparation_is_exclusive_and_recovers_after_error() {
        let first = InstallGuard::acquire().unwrap();
        assert!(InstallGuard::acquire().is_err());
        drop(first);
        assert!(InstallGuard::acquire().is_ok());
    }

    #[tokio::test]
    async fn test_install_portable_missing_sha_fails() {
        let result = install_portable_update("https://example.com/binary.exe".to_string(), None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Mandatory SHA-256 hash missing"));

        let result_empty = install_portable_update("https://example.com/binary.exe".to_string(), Some("   ".to_string())).await;
        assert!(result_empty.is_err());
        assert!(result_empty.unwrap_err().contains("Mandatory SHA-256 hash missing"));
    }

    #[test]
    fn test_get_app_version() {
        assert_eq!(get_app_version(), env!("CARGO_PKG_VERSION"));
    }
}
