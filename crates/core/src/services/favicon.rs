//! Favicon resolution and caching service.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

static FAVICON_CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
static EXTERNAL_FAVICONS_ENABLED: AtomicBool = AtomicBool::new(false);
static FETCH_SEMAPHORE: OnceLock<tokio::sync::Semaphore> = OnceLock::new();

/// Check whether external network resolution of favicons is enabled.
pub fn is_external_favicons_enabled() -> bool {
    EXTERNAL_FAVICONS_ENABLED.load(Ordering::Relaxed)
}

/// Enable or disable external network resolution of favicons.
pub fn set_external_favicons_enabled(enabled: bool) {
    EXTERNAL_FAVICONS_ENABLED.store(enabled, Ordering::Relaxed);
    if !enabled {
        clear_favicon_cache();
    }
}

/// Returns the local on-disk cache directory for favicons.
fn get_favicon_disk_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir());
        let dir = base.join("Yntra Vault").join("cache").join("favicons");
        let _ = std::fs::create_dir_all(&dir);
        Some(dir)
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(home) = std::env::var("HOME") {
            let mut path = PathBuf::from(home);
            path.push(".cache");
            path.push("yntra-vault");
            path.push("favicons");
            let _ = std::fs::create_dir_all(&path);
            Some(path)
        } else {
            None
        }
    }
}

/// Clears in-memory and on-disk favicon cache.
pub fn clear_favicon_cache() {
    if let Ok(mut guard) = get_cache().lock() {
        guard.clear();
    }
    if let Some(dir) = get_favicon_disk_dir() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

fn get_cache() -> &'static Mutex<HashMap<String, String>> {
    FAVICON_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_semaphore() -> &'static tokio::sync::Semaphore {
    FETCH_SEMAPHORE.get_or_init(|| tokio::sync::Semaphore::new(6))
}

/// Checks whether a domain or IP represents an internal, private, loopback, or metadata endpoint.
fn is_internal_or_private(domain: &str) -> bool {
    if domain.ends_with(".local")
        || domain.ends_with(".lan")
        || domain.ends_with(".internal")
        || domain.ends_with(".home")
        || domain.ends_with(".corp")
        || domain.ends_with(".onion")
        || domain.ends_with(".i2p")
    {
        return true;
    }

    if let Ok(ip) = domain.parse::<std::net::IpAddr>() {
        match ip {
            std::net::IpAddr::V4(ipv4) => {
                ipv4.is_loopback()
                    || ipv4.is_private()
                    || ipv4.is_link_local()
                    || ipv4.is_broadcast()
                    || ipv4.is_unspecified()
                    || (ipv4.octets()[0] == 169 && ipv4.octets()[1] == 254)
            }
            std::net::IpAddr::V6(ipv6) => {
                ipv6.is_loopback() || ipv6.is_unspecified()
            }
        }
    } else {
        false
    }
}

fn get_http_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(6))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 3 {
                    attempt.stop()
                } else if let Some(host) = attempt.url().host_str() {
                    if is_internal_or_private(host) {
                        attempt.stop()
                    } else {
                        attempt.follow()
                    }
                } else {
                    attempt.stop()
                }
            }))
            .build()
            .unwrap_or_default()
    })
}

/// Helper to fetch and convert an image response to a base64 Data URI
async fn try_fetch_candidate(client: &reqwest::Client, url: &str) -> Option<String> {
    let resp = client.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let ct_lower = content_type.to_ascii_lowercase();

    // Reject known non-image MIME types immediately (e.g. SPA HTML 200 OK responses)
    if ct_lower.starts_with("text/html")
        || ct_lower.starts_with("application/json")
        || ct_lower.starts_with("text/plain")
    {
        return None;
    }

    let bytes = resp.bytes().await.ok()?;
    // Reject tiny corrupt responses (< 32 bytes) or unreasonably large responses (> 512 KB)
    if bytes.len() < 32 || bytes.len() > 512 * 1024 {
        return None;
    }

    let is_image_mime = ct_lower.starts_with("image/") || ct_lower.contains("icon");
    let is_image_magic = bytes.starts_with(&[0x89, b'P', b'N', b'G']) // PNG
        || bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) // ICO
        || bytes.starts_with(b"GIF8") // GIF
        || bytes.starts_with(&[0xFF, 0xD8, 0xFF]) // JPEG
        || (bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP") // WebP
        || bytes.starts_with(b"<svg")
        || bytes.starts_with(b"<?xml"); // SVG

    if !is_image_mime && !is_image_magic {
        return None;
    }

    let clean_mime = if is_image_mime {
        ct_lower.split(';').next().unwrap_or("image/png").trim()
    } else if bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        "image/x-icon"
    } else {
        "image/png"
    };

    let b64 = data_encoding::BASE64.encode(&bytes);
    Some(format!("data:{clean_mime};base64,{b64}"))
}

/// Fetches a favicon for the specified domain.
/// Returns `Ok(Some(data_uri))` on success, or `Ok(None)` if not found / invalid / error.
pub async fn get_favicon(domain: &str) -> crate::Result<Option<String>> {
    let clean_domain = domain
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .split('?')
        .next()
        .unwrap_or("")
        .split('#')
        .next()
        .unwrap_or("")
        .split('@')
        .last()
        .unwrap_or("")
        .trim_start_matches("www.")
        .to_lowercase();

    if clean_domain.is_empty()
        || !clean_domain.contains('.')
        || clean_domain.starts_with('.')
        || clean_domain.ends_with('.')
        || !clean_domain.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return Ok(None);
    }

    // SSRF & Local network shield: Never query external endpoints for internal/private endpoints
    if is_internal_or_private(&clean_domain) {
        return Ok(None);
    }

    // Offline-first invariant: do not resolve or return external favicons unless enabled
    if !is_external_favicons_enabled() {
        return Ok(None);
    }

    // 1. Check in-memory cache
    if let Ok(guard) = get_cache().lock() {
        if let Some(cached) = guard.get(&clean_domain) {
            return Ok(Some(cached.clone()));
        }
    }

    // 2. Check local on-disk cache (persisted across restarts)
    if let Some(dir) = get_favicon_disk_dir() {
        let hash = blake3::hash(clean_domain.as_bytes()).to_hex();
        let cache_file = dir.join(format!("{hash}.dat"));
        let target_file = if cache_file.is_file() {
            Some(cache_file)
        } else {
            let safe_filename: String = clean_domain
                .chars()
                .map(|c| if c.is_alphanumeric() || c == '.' || c == '-' { c } else { '_' })
                .collect();
            let legacy_file = dir.join(format!("{safe_filename}.dat"));
            if legacy_file.is_file() {
                Some(legacy_file)
            } else {
                None
            }
        };

        if let Some(file) = target_file {
            if let Ok(data_uri) = std::fs::read_to_string(&file) {
                if data_uri.starts_with("data:") {
                    if let Ok(mut guard) = get_cache().lock() {
                        guard.insert(clean_domain.clone(), data_uri.clone());
                    }
                    return Ok(Some(data_uri));
                }
            }
        }
    }

    // 3. Rate-limited / bounded concurrent network resolution (max 6 parallel fetches)
    let _permit = get_semaphore().acquire().await.ok();
    let client = get_http_client();

    // Multi-tier fallback strategy:
    // Candidate 1: DuckDuckGo favicon CDN (fast, crisp multi-size ICOs, privacy-focused)
    let ddg_url = format!("https://icons.duckduckgo.com/ip3/{clean_domain}.ico");
    let mut result = try_fetch_candidate(client, &ddg_url).await;

    // Candidate 2: Google s2 favicons API (broad fallback coverage)
    if result.is_none() {
        let google_url = format!("https://www.google.com/s2/favicons?domain={clean_domain}&sz=64");
        result = try_fetch_candidate(client, &google_url).await;
    }

    // Candidate 3: Direct host favicon
    if result.is_none() {
        let direct_url = format!("https://{clean_domain}/favicon.ico");
        result = try_fetch_candidate(client, &direct_url).await;
    }

    // Candidate 4: Parent domain fallback (e.g. login.live.com -> live.com)
    if result.is_none() {
        let parts: Vec<&str> = clean_domain.split('.').collect();
        if parts.len() > 2 {
            let parent_domain = if parts.len() >= 3
                && ["co.uk", "com.au", "co.jp", "com.br", "co.nz"].iter().any(|suf| clean_domain.ends_with(suf))
            {
                if parts.len() >= 4 {
                    parts[parts.len() - 3..].join(".")
                } else {
                    clean_domain.clone()
                }
            } else {
                parts[parts.len() - 2..].join(".")
            };
            if parent_domain != clean_domain {
                let ddg_parent = format!("https://icons.duckduckgo.com/ip3/{parent_domain}.ico");
                result = try_fetch_candidate(client, &ddg_parent).await;
                if result.is_none() {
                    let google_parent = format!("https://www.google.com/s2/favicons?domain={parent_domain}&sz=64");
                    result = try_fetch_candidate(client, &google_parent).await;
                }
            }
        }
    }

    // 4. If successful, persist to both memory and disk caches
    if let Some(ref data_uri) = result {
        if let Ok(mut guard) = get_cache().lock() {
            guard.insert(clean_domain.clone(), data_uri.clone());
        }
        if let Some(dir) = get_favicon_disk_dir() {
            let hash = blake3::hash(clean_domain.as_bytes()).to_hex();
            let cache_file = dir.join(format!("{hash}.dat"));
            let _ = std::fs::write(&cache_file, data_uri);
        }
    }

    // Never cache `None` permanently so temporary network drops or timeouts can recover
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_favicon_offline_first_default() {
        set_external_favicons_enabled(false);
        assert!(!is_external_favicons_enabled());

        let res = get_favicon("github.com").await.unwrap();
        assert_eq!(res, None, "Must not query external endpoints when disabled");
    }

    #[tokio::test]
    async fn test_favicon_toggle_and_cache_clearing() {
        set_external_favicons_enabled(false);
        assert!(!is_external_favicons_enabled());

        set_external_favicons_enabled(true);
        assert!(is_external_favicons_enabled());

        // Disabling again should clear cache
        set_external_favicons_enabled(false);
        assert!(!is_external_favicons_enabled());
        let guard = get_cache().lock().unwrap();
        assert!(guard.is_empty());
    }

    #[tokio::test]
    async fn test_favicon_internal_and_private_rejected() {
        set_external_favicons_enabled(true);
        assert_eq!(get_favicon("127.0.0.1").await.unwrap(), None);
        assert_eq!(get_favicon("192.168.1.1").await.unwrap(), None);
        assert_eq!(get_favicon("10.0.0.5").await.unwrap(), None);
        assert_eq!(get_favicon("169.254.169.254").await.unwrap(), None);
        assert_eq!(get_favicon("router.local").await.unwrap(), None);
        assert_eq!(get_favicon("nas.lan").await.unwrap(), None);
        assert_eq!(get_favicon("secret.onion").await.unwrap(), None);
    }
}
