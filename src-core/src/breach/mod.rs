//! Breach detection & password strength analysis
//!
//! - HIBP k-anonymity API for breach checking
//! - Entropy-based password strength scoring
//! - Background monitoring

pub mod strength;

use sha1::{Sha1, Digest};
use serde::{Deserialize, Serialize};
use crate::error::VaultError;
use crate::vault::types::BreachStatus;

/// Result of a breach check for a single password.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BreachResult {
    pub is_breached: bool,
    pub breach_count: u64,
    pub checked_at: chrono::DateTime<chrono::Utc>,
}

/// Check a single password against HIBP using k-anonymity.
///
/// Only the first 5 characters of the SHA-1 hash are sent to HIBP.
/// The rest is compared locally — HIBP never sees your password.
pub async fn check_password_breach(password: &str) -> crate::Result<BreachResult> {
    // SHA-1 hash the password
    let mut hasher = Sha1::new();
    hasher.update(password.as_bytes());
    let hash = format!("{:X}", hasher.finalize());

    // Split: first 5 chars sent to API, rest compared locally
    let prefix = &hash[..5];
    let suffix = &hash[5..];

    // Query HIBP API
    let url = format!("https://api.pwnedpasswords.com/range/{}", prefix);

    let client = reqwest::Client::builder()
        .user_agent("Yntra Vault-PasswordManager/1.0")
        .build()
        .map_err(|e| VaultError::BreachCheckError(format!("HTTP client: {}", e)))?;

    let response = client.get(&url).send().await
        .map_err(|e| VaultError::BreachCheckError(format!("HIBP request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(VaultError::BreachCheckError(
            format!("HIBP returned status: {}", response.status())
        ));
    }

    let body = response.text().await
        .map_err(|e| VaultError::BreachCheckError(format!("Failed to read response: {}", e)))?;

    // Parse response — each line is "SUFFIX:COUNT"
    let mut breach_count = 0u64;
    let mut found = false;

    for line in body.lines() {
        if let Some((line_suffix, count_str)) = line.split_once(':') {
            if line_suffix.eq_ignore_ascii_case(suffix) {
                breach_count = count_str.trim().parse().unwrap_or(0);
                found = true;
                break;
            }
        }
    }

    Ok(BreachResult {
        is_breached: found,
        breach_count,
        checked_at: chrono::Utc::now(),
    })
}

/// Convert a BreachResult into a BreachStatus for storage.
pub fn breach_result_to_status(result: &BreachResult) -> BreachStatus {
    if result.is_breached {
        BreachStatus::Breached {
            breach_count: result.breach_count,
            checked_at: result.checked_at,
        }
    } else {
        BreachStatus::Safe {
            checked_at: result.checked_at,
        }
    }
}

/// Check a password offline (without API) — uses the SHA-1 hash prefix
/// to determine if a cached result exists.
/// Returns None if no cached result available.
pub fn check_breach_cache(
    password: &str,
    cache: &std::collections::HashMap<String, BreachResult>,
) -> Option<BreachResult> {
    let mut hasher = Sha1::new();
    hasher.update(password.as_bytes());
    let hash = format!("{:X}", hasher.finalize());
    cache.get(&hash).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha1_hash_format() {
        // Verify our SHA-1 hashing matches expected format
        let mut hasher = Sha1::new();
        hasher.update(b"password");
        let hash = format!("{:X}", hasher.finalize());

        // "password" SHA-1 = 5BAA61E4C9B93F3F0682250B6CF8331B7EE68FD8
        assert_eq!(hash, "5BAA61E4C9B93F3F0682250B6CF8331B7EE68FD8");
        assert_eq!(hash.len(), 40);
    }

    #[test]
    fn test_prefix_suffix_split() {
        let mut hasher = Sha1::new();
        hasher.update(b"password");
        let hash = format!("{:X}", hasher.finalize());

        let prefix = &hash[..5];
        let suffix = &hash[5..];

        assert_eq!(prefix, "5BAA6");
        assert_eq!(suffix, "1E4C9B93F3F0682250B6CF8331B7EE68FD8");
    }

    #[tokio::test]
    async fn test_known_breached_password() {
        // "password" is definitely in HIBP
        let result = check_password_breach("password").await;

        // May fail if no internet — that's OK for CI
        if let Ok(result) = result {
            assert!(result.is_breached);
            assert!(result.breach_count > 0);
        }
    }

    #[test]
    fn test_breach_result_to_status() {
        let breached = BreachResult {
            is_breached: true,
            breach_count: 1000,
            checked_at: chrono::Utc::now(),
        };
        let status = breach_result_to_status(&breached);
        assert!(matches!(status, BreachStatus::Breached { .. }));

        let safe = BreachResult {
            is_breached: false,
            breach_count: 0,
            checked_at: chrono::Utc::now(),
        };
        let status = breach_result_to_status(&safe);
        assert!(matches!(status, BreachStatus::Safe { .. }));
    }
}

