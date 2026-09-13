//! Favicon resolution and caching service.
//!
//! Operates in strict zero-knowledge offline mode to prevent leaking vault entry
//! domains and user IP addresses to third-party servers.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

static FAVICON_CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

fn get_cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    FAVICON_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Fetches a favicon for the specified domain.
///
/// In compliance with Yntra Vault's offline-first architecture, external HTTP
/// requests to third-party services are disabled to protect vault domain privacy.
/// Returns `Ok(None)` to cleanly trigger high-contrast, initial-based avatars.
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

    // Check in-memory local cache first
    if let Ok(guard) = get_cache().lock() {
        if let Some(cached) = guard.get(&clean_domain) {
            return Ok(cached.clone());
        }
    }

    // Strict offline-first policy: no outbound network requests to third parties.
    // The UI gracefully renders a colored initials avatar for the entry.
    Ok(None)
}

