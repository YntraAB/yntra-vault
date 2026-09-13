//! Favicon resolution and caching service.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

static FAVICON_CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn get_cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    FAVICON_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_http_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
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
    if let Ok(guard) = get_cache().lock() {
        if let Some(cached) = guard.get(&clean_domain) {
            return Ok(cached.clone());
        }
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
