//! Integration tests for Yntra Vault cross-platform updater engine

use sha2::{Sha256, Digest};

#[tokio::test]
#[ignore = "checks the published official release over HTTPS; never installs or accesses a vault"]
async fn published_release_update_check() {
    let expected = std::env::var("YNTRA_RELEASE_EXPECTED").expect("explicit expected release version required");
    for platform in ["windows-x86_64", "windows-portable", "windows-cli", "android", "linux-x86_64"] {
        let update = yntra_vault_core::services::updater::check_for_updates("0.0.0", platform, None).await.unwrap();
        assert_eq!(update.latest_version, expected);
        assert!(update.has_update);
        assert!(update.download_url.as_ref().is_some_and(|url| url.starts_with("https://github.com/YntraAB/yntra-vault/releases/download/")));
        if matches!(platform, "windows-portable" | "windows-cli" | "android") {
            assert!(update.sha256.as_ref().is_some_and(|hash| hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())));
        }
        let current = yntra_vault_core::services::updater::check_for_updates(&expected, platform, None).await.unwrap();
        assert!(!current.has_update);
        eprintln!("Official update check passed for {platform}: older version offered update, current version unchanged");
    }
}
use std::collections::HashMap;
use yntra_vault_core::services::updater::{
    is_newer_version, parse_version, verify_sha256, download_file,
    AndroidUpdateAsset, BinaryAsset, ExtraAssets, PlatformUpdate, UpdateManifest,
};

#[test]
fn test_semantic_versioning_comprehensive() {
    // Basic versions
    assert_eq!(parse_version("0.2.2"), (0, 2, 2, false));
    assert_eq!(parse_version("v0.2.3"), (0, 2, 3, false));
    assert_eq!(parse_version("V0.2.3"), (0, 2, 3, false));
    assert_eq!(parse_version("  v1.0.0  "), (1, 0, 0, false));

    // Pre-releases
    assert_eq!(parse_version("0.2.3-rc1"), (0, 2, 3, true));
    assert_eq!(parse_version("v0.2.3-beta.2"), (0, 2, 3, true));

    // Valid update progressions
    assert!(is_newer_version("0.2.2", "0.2.3"));
    assert!(is_newer_version("0.2.2", "0.3.0"));
    assert!(is_newer_version("0.2.2", "1.0.0"));
    assert!(is_newer_version("v0.2.2", "v0.2.3"));
    assert!(is_newer_version("V0.2.2", "v0.2.3"));
    assert!(is_newer_version("0.2.3-beta", "0.2.3")); // beta to stable is newer

    // Equal versions must NOT trigger update
    assert!(!is_newer_version("0.2.3", "0.2.3"));
    assert!(!is_newer_version("v0.2.3", "0.2.3"));
    assert!(!is_newer_version("0.2.3", "v0.2.3"));
    assert!(!is_newer_version("V0.2.3", "v0.2.3"));

    // Older versions must NOT trigger update
    assert!(!is_newer_version("0.2.3", "0.2.2"));
    assert!(!is_newer_version("1.0.0", "0.9.9"));
    assert!(!is_newer_version("0.3.0", "0.2.9"));

    // Stable users must NOT be prompted to update to pre-release
    assert!(!is_newer_version("0.2.3", "0.2.4-rc1"));
    assert!(!is_newer_version("0.2.3", "0.2.4-beta"));
    assert!(!is_newer_version("0.2.3", "0.3.0-alpha"));
}

#[test]
fn test_constant_time_sha256_verification() {
    let payload = b"Yntra Vault cryptographic update payload binary bytes 2026";
    let mut hasher = Sha256::new();
    hasher.update(payload);
    let valid_hash = data_encoding::HEXLOWER.encode(hasher.finalize().as_ref());

    // Valid hash passes
    assert!(verify_sha256(payload, &valid_hash));
    // Uppercase hash passes
    assert!(verify_sha256(payload, &valid_hash.to_uppercase()));
    // Trimmed whitespace passes
    assert!(verify_sha256(payload, &format!("  {}  ", valid_hash)));

    // Tampered payload fails
    assert!(!verify_sha256(b"Tampered payload", &valid_hash));
    // Bit flip in hash fails
    let mut corrupted_hash = valid_hash.clone();
    corrupted_hash.replace_range(0..1, if corrupted_hash.starts_with('a') { "b" } else { "a" });
    assert!(!verify_sha256(payload, &corrupted_hash));

    // Malformed / short hashes fail
    assert!(!verify_sha256(payload, ""));
    assert!(!verify_sha256(payload, "abc"));
    assert!(!verify_sha256(payload, "0123456789abcdef"));
    assert!(!verify_sha256(payload, &format!("{}extra", valid_hash)));
}

#[test]
fn test_manifest_platform_asset_resolution() {
    let mut platforms = HashMap::new();
    platforms.insert(
        "windows-x86_64".to_string(),
        PlatformUpdate {
            signature: "sig-win-setup".to_string(),
            url: "https://github.com/YntraAB/yntra-vault/releases/download/v0.2.3/Yntra.Vault_0.2.3_x64-setup.exe".to_string(),
        },
    );
    platforms.insert(
        "linux-x86_64".to_string(),
        PlatformUpdate {
            signature: "sig-linux-appimage".to_string(),
            url: "https://github.com/YntraAB/yntra-vault/releases/download/v0.2.3/Yntra.Vault_0.2.3_amd64.AppImage".to_string(),
        },
    );

    let mut cli_map = HashMap::new();
    cli_map.insert(
        "windows-x86_64".to_string(),
        BinaryAsset {
            version: "0.2.3".to_string(),
            url: "https://github.com/YntraAB/yntra-vault/releases/download/v0.2.3/Yntra.Vault_0.2.3_cli.exe".to_string(),
            sha256: "1111111111111111111111111111111111111111111111111111111111111111".to_string(),
        },
    );

    let mut portable_map = HashMap::new();
    portable_map.insert(
        "windows-x86_64".to_string(),
        BinaryAsset {
            version: "0.2.3".to_string(),
            url: "https://github.com/YntraAB/yntra-vault/releases/download/v0.2.3/Yntra.Vault_0.2.3_portable.exe".to_string(),
            sha256: "2222222222222222222222222222222222222222222222222222222222222222".to_string(),
        },
    );

    let extra = ExtraAssets {
        android: Some(AndroidUpdateAsset {
            version: "0.2.3".to_string(),
            url: "https://github.com/YntraAB/yntra-vault/releases/download/v0.2.3/Yntra.Vault_0.2.3_universal.apk".to_string(),
            sha256: "3333333333333333333333333333333333333333333333333333333333333333".to_string(),
            size_bytes: Some(42_000_000),
        }),
        cli: cli_map,
        portable: portable_map,
    };

    let manifest = UpdateManifest {
        version: "0.2.3".to_string(),
        notes: Some("Security enhancements and automated updater engine".to_string()),
        pub_date: Some("2026-09-22T08:00:00Z".to_string()),
        platforms,
        extra: Some(extra),
    };

    // Serialize to JSON and deserialize back
    let serialized = serde_json::to_string_pretty(&manifest).unwrap();
    let deserialized: UpdateManifest = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized.version, "0.2.3");

    // Check Android resolution
    let android_asset = deserialized.extra.as_ref().unwrap().android.as_ref().unwrap();
    assert_eq!(android_asset.sha256, "3333333333333333333333333333333333333333333333333333333333333333");
    assert!(android_asset.url.ends_with(".apk"));

    // Check Windows Portable resolution
    let port_asset = deserialized.extra.as_ref().unwrap().portable.get("windows-x86_64").unwrap();
    assert_eq!(port_asset.sha256, "2222222222222222222222222222222222222222222222222222222222222222");
    assert!(port_asset.url.ends_with(".exe"));

    // Check Windows CLI resolution
    let cli_asset = deserialized.extra.as_ref().unwrap().cli.get("windows-x86_64").unwrap();
    assert_eq!(cli_asset.sha256, "1111111111111111111111111111111111111111111111111111111111111111");

    // Check Desktop NSIS resolution
    let nsis = deserialized.platforms.get("windows-x86_64").unwrap();
    assert_eq!(nsis.signature, "sig-win-setup");
}

#[tokio::test]
async fn test_insecure_transport_rejection() {
    // HTTP plaintext is strictly rejected
    let res = download_file("http://evil.com/malware.apk").await;
    assert!(res.is_err());
    let err = res.unwrap_err().to_string();
    assert!(err.contains("Insecure download URL rejected: only HTTPS"));

    // file:// protocol is strictly rejected
    let res_file = download_file("file:///etc/passwd").await;
    assert!(res_file.is_err());
}

#[test]
fn test_inplace_executable_replacement_simulation() {
    let temp_dir = tempfile::tempdir().unwrap();
    let current_exe = temp_dir.path().join("yntra_mock_app.exe");
    #[cfg(windows)]
    let old_exe = current_exe.with_extension("exe.old");

    // 1. Initial running executable state
    std::fs::write(&current_exe, b"INITIAL_BINARY_V0.2.2").unwrap();
    assert_eq!(std::fs::read(&current_exe).unwrap(), b"INITIAL_BINARY_V0.2.2");

    // Vault files and the remembered-vault index must survive an actual update.
    let vault = temp_dir.path().join("personal.vdb");
    let recent = temp_dir.path().join("recent-vaults.json");
    std::fs::write(&vault, b"ENCRYPTED_VAULT_FIXTURE").unwrap();
    let recent_bytes = serde_json::to_vec(&serde_json::json!([{ "path": vault, "name": "Personal" }])).unwrap();
    std::fs::write(&recent, &recent_bytes).unwrap();
    // 2. Exercise the production replacement function, not a simulated protocol.
    let new_bytes = b"UPDATED_BINARY_V0.2.3_CRYPTOGRAPHICALLY_VERIFIED";

    yntra_vault_core::services::updater::replace_executable(&current_exe, new_bytes).unwrap();
    assert_eq!(std::fs::read(&vault).unwrap(), b"ENCRYPTED_VAULT_FIXTURE");
    assert_eq!(std::fs::read(&recent).unwrap(), recent_bytes);

    // 3. Verify target executable now has updated bytes
    assert_eq!(std::fs::read(&current_exe).unwrap(), new_bytes);

    // 4. Verify old binary exists at .exe.old
    #[cfg(windows)]
    {
    assert!(old_exe.exists());
    assert_eq!(std::fs::read(&old_exe).unwrap(), b"INITIAL_BINARY_V0.2.2");

    // 5. Cleanup on next startup
    std::fs::remove_file(&old_exe).unwrap();
    assert!(!old_exe.exists());
    }
    assert_eq!(std::fs::read(&current_exe).unwrap(), new_bytes);
}
