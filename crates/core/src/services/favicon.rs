//! Favicon resolution and caching service.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

static FAVICON_CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
static EXTERNAL_FAVICONS_ENABLED: AtomicBool = AtomicBool::new(false);

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

/// Clears the in-memory favicon cache.
pub fn clear_favicon_cache() {
    if let Ok(mut guard) = get_cache().lock() {
        guard.clear();
    }
}

fn get_cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    FAVICON_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_http_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(4))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
            .build()
            .unwrap_or_default()
    })
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
        .trim_start_matches("www.")
        .to_lowercase();

    if clean_domain.is_empty() || !clean_domain.contains('.') {
        return Ok(None);
    }

    // Check in-memory cache first
    if let Ok(guard) = get_cache().lock()
        && let Some(cached) = guard.get(&clean_domain) {
            return Ok(cached.clone());
        }

    // Offline-first invariant: do not query external third parties unless explicitly enabled
    if !is_external_favicons_enabled() {
        return Ok(None);
    }

    let client = get_http_client();
    let url = format!("https://www.google.com/s2/favicons?domain={clean_domain}&sz=64");

    let result: Option<String> = match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let content_type = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("image/png")
                .to_string();

            match resp.bytes().await {
                Ok(bytes) if !bytes.is_empty() => {
                    let b64 = data_encoding::BASE64.encode(&bytes);
                    Some(format!("data:{content_type};base64,{b64}"))
                }
                _ => None,
            }
        }
        _ => None,
    };

    // Store in cache
    if let Ok(mut guard) = get_cache().lock() {
        guard.insert(clean_domain, result.clone());
    }

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
}
