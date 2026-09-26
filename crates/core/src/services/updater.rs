//! Cross-Platform Updater Engine for Yntra Vault
//!
//! Provides cryptographic update checks, semantic version comparisons,
//! manifest fetching (with GitHub Releases API fallback), SHA-256 integrity verification,
//! and asset resolution for Desktop, Android, and CLI distributions.

use std::collections::HashMap;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use subtle::ConstantTimeEq;

use crate::{Result, VaultError};

pub const DEFAULT_UPDATE_ENDPOINT: &str =
    "https://github.com/YntraAB/yntra-vault/releases/latest/download/latest.json";
pub const GITHUB_RELEASES_API: &str =
    "https://api.github.com/repos/YntraAB/yntra-vault/releases/latest";

/// Platform update information formatted for Tauri v2 updater
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlatformUpdate {
    #[serde(default)]
    pub signature: String,
    pub url: String,
}

/// Android APK metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AndroidUpdateAsset {
    pub version: String,
    pub url: String,
    pub sha256: String,
    #[serde(default)]
    pub size_bytes: Option<u64>,
}

/// Standalone binary asset metadata (CLI, Portable, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BinaryAsset {
    pub version: String,
    pub url: String,
    pub sha256: String,
}

/// Supplemental multi-platform assets in `latest.json`
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtraAssets {
    #[serde(default)]
    pub android: Option<AndroidUpdateAsset>,
    #[serde(default)]
    pub cli: HashMap<String, BinaryAsset>,
    #[serde(default)]
    pub portable: HashMap<String, BinaryAsset>,
}

/// Complete universal update manifest (`latest.json`)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateManifest {
    pub version: String,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub pub_date: Option<String>,
    #[serde(default)]
    pub platforms: HashMap<String, PlatformUpdate>,
    #[serde(default)]
    pub extra: Option<ExtraAssets>,
}

/// Result of an update check
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckUpdateResult {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_notes: Option<String>,
    pub pub_date: Option<String>,
    pub download_url: Option<String>,
    pub sha256: Option<String>,
    pub signature: Option<String>,
    pub target_platform: String,
}

pub const MAX_UPDATE_PACKAGE_SIZE: usize = 250 * 1024 * 1024; // 250 MB

/// Parse semantic version string (e.g. "v0.2.3", "V0.2.3", "0.2.3", "0.2.3-rc1")
/// into a comparable tuple: `(major, minor, patch, is_prerelease)`
pub fn parse_version(v: &str) -> (u64, u64, u64, bool) {
    semver::Version::parse(v.trim().trim_start_matches(['v', 'V']))
        .map(|version| (version.major, version.minor, version.patch, !version.pre.is_empty()))
        .unwrap_or((0, 0, 0, false))
}

/// Returns true if `candidate` is strictly newer than `current`.
pub fn is_newer_version(current: &str, candidate: &str) -> bool {
    let parse = |value: &str| semver::Version::parse(value.trim().trim_start_matches(['v', 'V']));
    let (Ok(current), Ok(candidate)) = (parse(current), parse(candidate)) else {
        return false;
    };
    (current.pre.is_empty() == candidate.pre.is_empty() || !current.pre.is_empty())
        && candidate.cmp_precedence(&current).is_gt()
}

/// Verify data against expected SHA-256 hex string in constant time
pub fn verify_sha256(data: &[u8], expected_hex: &str) -> bool {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let computed_hash = hasher.finalize();
    let computed_hex = data_encoding::HEXLOWER.encode(computed_hash.as_ref());

    let clean_expected = expected_hex.trim().to_lowercase();
    if clean_expected.len() != 64 {
        return false;
    }

    computed_hex.as_bytes().ct_eq(clean_expected.as_bytes()).into()
}

/// Build standard HTTP client for lightweight manifest checks (15s timeout)
fn build_manifest_http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(format!("YntraVault-Updater/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(15))
        .https_only(true)
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(VaultError::NetworkError)
}

/// Build HTTP client for package downloads (300s / 5 min timeout)
fn build_download_http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(format!("YntraVault-Updater/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(300))
        .https_only(true)
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(VaultError::NetworkError)
}

/// Fetch and parse `UpdateManifest` from URL
pub async fn fetch_manifest(endpoint: &str) -> Result<UpdateManifest> {
    validate_update_url(endpoint)?;
    let client = build_manifest_http_client()?;
    let resp = client.get(endpoint).send().await.map_err(VaultError::NetworkError)?;

    if !resp.status().is_success() {
        return Err(VaultError::UpdateError(format!(
            "Failed to fetch update manifest: HTTP {}",
            resp.status()
        )));
    }

    let bytes = read_limited_response(resp, 1024 * 1024).await?;
    let manifest: UpdateManifest = serde_json::from_slice(&bytes).map_err(VaultError::JsonError)?;
    Ok(manifest)
}

/// GitHub Release API structure (used as fallback if `latest.json` is unavailable)
#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    digest: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    body: Option<String>,
    published_at: Option<String>,
    assets: Vec<GhAsset>,
}

/// Fallback to GitHub Release API
pub async fn fetch_from_github_api() -> Result<UpdateManifest> {
    let client = build_manifest_http_client()?;
    let resp = client
        .get(GITHUB_RELEASES_API)
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .map_err(VaultError::NetworkError)?;

    if !resp.status().is_success() {
        return Err(VaultError::UpdateError(format!(
            "GitHub API request failed with HTTP {}",
            resp.status()
        )));
    }

    let bytes = read_limited_response(resp, 1024 * 1024).await?;
    let gh_rel: GhRelease = serde_json::from_slice(&bytes).map_err(VaultError::JsonError)?;
    Ok(manifest_from_github_release(gh_rel))
}

fn manifest_from_github_release(gh_rel: GhRelease) -> UpdateManifest {
    let ver = gh_rel.tag_name.trim_start_matches('v').to_string();

    let mut manifest = UpdateManifest {
        version: ver.clone(),
        notes: gh_rel.body,
        pub_date: gh_rel.published_at,
        platforms: HashMap::new(),
        extra: Some(ExtraAssets::default()),
    };

    let mut extra = ExtraAssets::default();

    for asset in gh_rel.assets {
        let name_lower = asset.name.to_lowercase();
        let sha256 = asset.digest.as_deref().and_then(|d| d.strip_prefix("sha256:"))
            .filter(|hash| hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
            .unwrap_or("").to_string();
        if name_lower.ends_with(".apk") {
            extra.android = Some(AndroidUpdateAsset {
                version: ver.clone(),
                url: asset.browser_download_url.clone(),
                sha256: sha256.clone(),
                size_bytes: None,
            });
        } else if name_lower.contains("cli") && name_lower.ends_with(".exe") {
            extra.cli.insert(
                "windows-x86_64".to_string(),
                BinaryAsset {
                    version: ver.clone(),
                    url: asset.browser_download_url.clone(),
                    sha256: sha256.clone(),
                },
            );
        } else if name_lower.contains("portable") && name_lower.ends_with(".exe") {
            extra.portable.insert(
                "windows-x86_64".to_string(),
                BinaryAsset {
                    version: ver.clone(),
                    url: asset.browser_download_url.clone(),
                    sha256: sha256.clone(),
                },
            );
        } else {
            let platform = if name_lower.ends_with("_x64-setup.exe") {
                Some("windows-x86_64")
            } else if name_lower.ends_with("_amd64.appimage") {
                Some("linux-x86_64")
            } else if name_lower.ends_with("_aarch64.dmg") {
                Some("darwin-aarch64")
            } else if name_lower.ends_with("_x64.dmg") {
                Some("darwin-x86_64")
            } else { None };
            if let Some(platform) = platform {
                manifest.platforms.insert(platform.into(), PlatformUpdate { url: asset.browser_download_url, signature: String::new() });
            }
        }
    }

    manifest.extra = Some(extra);
    manifest
}

/// Check for updates across any target platform
pub async fn check_for_updates(
    current_version: &str,
    target_platform: &str,
    custom_endpoint: Option<&str>,
) -> Result<CheckUpdateResult> {
    let endpoint = custom_endpoint.unwrap_or(DEFAULT_UPDATE_ENDPOINT);

    validate_update_url(endpoint)?;

    // 1. Try to fetch manifest from latest.json
    let manifest = match fetch_manifest(endpoint).await {
        Ok(m) => m,
        Err(error) if custom_endpoint.is_some() => return Err(error),
        Err(_) => {
            // Fallback to GitHub Release API
            fetch_from_github_api().await?
        }
    };

    let latest_ver = manifest.version.clone();
    let has_update = is_newer_version(current_version, &latest_ver);

    let mut download_url = None;
    let mut sha256 = None;
    let mut signature = None;

    // Resolve asset for target platform
    match target_platform {
        "android" => {
            if let Some(ref extra) = manifest.extra
                && let Some(ref android) = extra.android {
                download_url = Some(android.url.clone());
                sha256 = if !android.sha256.is_empty() {
                    Some(android.sha256.clone())
                } else {
                    None
                };
            }
        }
        "windows-cli" => {
            if let Some(ref extra) = manifest.extra
                && let Some(cli) = extra.cli.get("windows-x86_64") {
                download_url = Some(cli.url.clone());
                sha256 = if !cli.sha256.is_empty() {
                    Some(cli.sha256.clone())
                } else {
                    None
                };
            } else if let Some(p) = manifest.platforms.get("windows-cli").or_else(|| manifest.platforms.get("windows-x86_64")) {
                download_url = Some(p.url.clone());
                if !p.signature.is_empty() {
                    signature = Some(p.signature.clone());
                }
            }
        }
        "windows-portable" => {
            if let Some(ref extra) = manifest.extra
                && let Some(port) = extra.portable.get("windows-x86_64") {
                download_url = Some(port.url.clone());
                sha256 = if !port.sha256.is_empty() {
                    Some(port.sha256.clone())
                } else {
                    None
                };
            } else if let Some(p) = manifest.platforms.get("windows-portable") {
                download_url = Some(p.url.clone());
                if !p.signature.is_empty() {
                    signature = Some(p.signature.clone());
                }
            }
        }
        platform => {
            if let Some(p) = manifest.platforms.get(platform) {
                download_url = Some(p.url.clone());
                signature = Some(p.signature.clone());
            }
        }
    }

    Ok(CheckUpdateResult {
        current_version: current_version.to_string(),
        latest_version: latest_ver,
        has_update,
        release_notes: manifest.notes,
        pub_date: manifest.pub_date,
        download_url,
        sha256,
        signature,
        target_platform: target_platform.to_string(),
    })
}

/// Download binary file payload into memory
pub async fn download_file(url: &str) -> Result<Vec<u8>> {
    let clean_url = url.trim();
    validate_update_url(clean_url)?;

    let client = build_download_http_client()?;
    let resp = client.get(clean_url).send().await.map_err(VaultError::NetworkError)?;

    if !resp.status().is_success() {
        return Err(VaultError::UpdateError(format!(
            "Failed to download update from {}: HTTP {}",
            clean_url,
            resp.status()
        )));
    }

    read_limited_response(resp, MAX_UPDATE_PACKAGE_SIZE).await
}


fn validate_update_url(raw: &str) -> Result<()> {
    let parsed = url::Url::parse(raw.trim()).map_err(|_| VaultError::UpdateError("Invalid update URL".into()))?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() || !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(VaultError::UpdateError("Insecure download URL rejected: only HTTPS without credentials is permitted".into()));
    }
    Ok(())
}

async fn read_limited_response(mut response: reqwest::Response, limit: usize) -> Result<Vec<u8>> {
    if response.content_length().is_some_and(|size| size > limit as u64) {
        return Err(VaultError::UpdateError("Download exceeds size limit".into()));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(VaultError::NetworkError)? {
        if chunk.len() > limit.saturating_sub(bytes.len()) {
            return Err(VaultError::UpdateError("Download exceeds size limit".into()));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// Stage and flush the complete update before replacing the installed executable.
pub fn replace_executable(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(std::path::Path::new("."));
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged.write_all(bytes)?;
    staged.as_file().set_permissions(std::fs::metadata(path)?.permissions())?;
    staged.as_file().sync_all()?;
    #[cfg(windows)]
    {
        let backup = path.with_extension("exe.old");
        if backup.exists() {
            return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "Restart before applying another update"));
        }
        std::fs::rename(path, &backup)?;
        if let Err(error) = staged.persist(path) {
            std::fs::rename(&backup, path)?;
            return Err(error.error);
        }
    }
    #[cfg(not(windows))]
    staged.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_fallback_resolves_desktop_installers_without_a_manifest() {
        let release: GhRelease = serde_json::from_value(serde_json::json!({
            "tag_name": "v0.2.3", "body": null, "published_at": null,
            "assets": [
                {"name":"Yntra.Vault_0.2.3_x64-setup.exe", "browser_download_url":"https://example.test/setup.exe"},
                {"name":"Yntra.Vault_0.2.3_amd64.AppImage", "browser_download_url":"https://example.test/app.AppImage"}
            ]
        })).unwrap();
        let manifest = manifest_from_github_release(release);
        assert_eq!(manifest.platforms["windows-x86_64"].url, "https://example.test/setup.exe");
        assert_eq!(manifest.platforms["linux-x86_64"].url, "https://example.test/app.AppImage");
    }

    #[test]
    fn test_version_parsing_and_comparison() {
        assert_eq!(parse_version("0.2.2"), (0, 2, 2, false));
        assert_eq!(parse_version("v0.2.2"), (0, 2, 2, false));
        assert_eq!(parse_version("V0.2.2"), (0, 2, 2, false));
        assert_eq!(parse_version("1.0.0-rc1"), (1, 0, 0, true));
        assert_eq!(parse_version("v1.2.3+build.7"), (1, 2, 3, false));

        // Newer checks
        assert!(is_newer_version("0.2.2", "0.2.3"));
        assert!(is_newer_version("0.2.2", "0.3.0"));
        assert!(is_newer_version("0.2.2", "1.0.0"));
        assert!(is_newer_version("v0.2.2", "v0.2.3"));
        assert!(is_newer_version("V0.2.2", "v0.2.3"));
        assert!(is_newer_version("0.2.2-beta", "0.2.2"));

        // Prerelease guard: stable users should not be prompted to upgrade to prereleases
        assert!(!is_newer_version("0.2.2", "0.2.3-rc1"));
        assert!(!is_newer_version("0.2.3", "0.2.4-beta"));

        // Not newer checks
        assert!(!is_newer_version("0.2.2", "0.2.2"));
        assert!(!is_newer_version("0.2.3", "0.2.2"));
        assert!(!is_newer_version("1.0.0", "0.9.9"));
        assert!(!is_newer_version("0.3.0", "0.2.9"));
        assert!(is_newer_version("1.0.0-rc.2", "1.0.0-rc.10"));
        assert!(!is_newer_version("1.0.0+build.1", "1.0.0+build.2"));
        assert!(!is_newer_version("1.0.0", "2.0.invalid"));
    }

    #[test]
    fn test_sha256_verification() {
        let data = b"Hello, Yntra Vault Update!";
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hex_hash = data_encoding::HEXLOWER.encode(hasher.finalize().as_ref());

        assert!(verify_sha256(data, &hex_hash));
        assert!(verify_sha256(data, &hex_hash.to_uppercase()));
        assert!(!verify_sha256(data, "0000000000000000000000000000000000000000000000000000000000000000"));
        assert!(!verify_sha256(b"Tampered data", &hex_hash));
        assert!(!verify_sha256(data, "short-hash"));
    }

    #[tokio::test]
    async fn test_insecure_url_rejected() {
        let result = download_file("http://example.com/insecure-binary.exe").await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Insecure download URL rejected"));
        assert!(fetch_manifest("http://example.com/latest.json").await.is_err());
        assert!(validate_update_url("https://user:password@example.com/latest.json").is_err());
    }

    #[tokio::test]
    async fn chunked_download_cannot_exceed_limit() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 4096];
            stream.read(&mut request).unwrap();
            stream.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n3\r\nabc\r\n3\r\ndef\r\n0\r\n\r\n").unwrap();
        });
        let response = reqwest::get(format!("http://{address}/")).await.unwrap();
        assert!(read_limited_response(response, 4).await.unwrap_err().to_string().contains("size limit"));
        server.join().unwrap();
    }

    #[test]
    fn executable_replacement_preserves_existing_file_on_staging_failure() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("app.exe");
        std::fs::write(&executable, b"old").unwrap();
        replace_executable(&executable, b"complete-new-binary").unwrap();
        assert_eq!(std::fs::read(&executable).unwrap(), b"complete-new-binary");
        #[cfg(windows)]
        {
            assert_eq!(std::fs::read(executable.with_extension("exe.old")).unwrap(), b"old");
            assert!(replace_executable(&executable, b"another").is_err());
            assert_eq!(std::fs::read(&executable).unwrap(), b"complete-new-binary");
        }
        assert!(replace_executable(&directory.path().join("missing/app.exe"), b"bytes").is_err());
    }

    #[test]
    fn test_manifest_deserialization() {
        let json_data = r#"{
            "version": "0.2.3",
            "notes": "Security and updater improvements",
            "pub_date": "2026-09-22T08:00:00Z",
            "platforms": {
                "windows-x86_64": {
                    "signature": "sig123",
                    "url": "https://example.com/win.zip"
                }
            },
            "extra": {
                "android": {
                    "version": "0.2.3",
                    "url": "https://example.com/app.apk",
                    "sha256": "abcdef123456"
                },
                "cli": {
                    "windows-x86_64": {
                        "version": "0.2.3",
                        "url": "https://example.com/cli.exe",
                        "sha256": "cli123"
                    }
                }
            }
        }"#;

        let manifest: UpdateManifest = serde_json::from_str(json_data).unwrap();
        assert_eq!(manifest.version, "0.2.3");
        assert_eq!(manifest.platforms.get("windows-x86_64").unwrap().signature, "sig123");
        assert_eq!(manifest.extra.as_ref().unwrap().android.as_ref().unwrap().sha256, "abcdef123456");
        assert_eq!(
            manifest.extra.as_ref().unwrap().cli.get("windows-x86_64").unwrap().url,
            "https://example.com/cli.exe"
        );
    }
}
