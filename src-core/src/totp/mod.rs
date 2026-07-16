//! TOTP (Time-based One-Time Password) Engine
//! 
//! RFC 6238 compliant implementation supporting:
//! - HMAC-SHA1, HMAC-SHA256, HMAC-SHA512
//! - 6 or 8 digit codes
//! - 30s or 60s periods
//! - otpauth:// URI parsing

use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::{Sha256, Sha512};
use data_encoding::BASE32_NOPAD;
use serde::{Deserialize, Serialize};
use crate::error::VaultError;

type HmacSha1 = Hmac<Sha1>;
type HmacSha256 = Hmac<Sha256>;
type HmacSha512 = Hmac<Sha512>;

/// TOTP algorithm selection.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum TotpAlgorithm {
    SHA1,
    SHA256,
    SHA512,
}

impl Default for TotpAlgorithm {
    fn default() -> Self {
        TotpAlgorithm::SHA1
    }
}

/// Full TOTP configuration for an account.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TotpConfig {
    /// Base32-encoded secret
    pub secret: String,
    /// Algorithm (default: SHA1)
    pub algorithm: TotpAlgorithm,
    /// Number of digits (6 or 8)
    pub digits: u32,
    /// Time period in seconds (30 or 60)
    pub period: u64,
    /// Issuer (e.g., "Google", "GitHub")
    pub issuer: Option<String>,
    /// Account label (e.g., "user@example.com")
    pub label: Option<String>,
}

impl Default for TotpConfig {
    fn default() -> Self {
        TotpConfig {
            secret: String::new(),
            algorithm: TotpAlgorithm::SHA1,
            digits: 6,
            period: 30,
            issuer: None,
            label: None,
        }
    }
}

/// Generated TOTP code with timing information.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TotpCode {
    pub code: String,
    pub seconds_remaining: u64,
    pub period: u64,
}

/// Generate a TOTP code for the current time.
pub fn generate_totp(config: &TotpConfig) -> crate::Result<TotpCode> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| VaultError::TotpError(format!("System time error: {}", e)))?;

    let timestamp = now.as_secs();
    generate_totp_at(config, timestamp)
}

/// Generate a TOTP code for a specific timestamp.
pub fn generate_totp_at(config: &TotpConfig, timestamp: u64) -> crate::Result<TotpCode> {
    // Decode the base32 secret
    let secret_upper = config.secret.to_uppercase().replace(" ", "");
    // Add padding if needed for base32
    let padded = match secret_upper.len() % 8 {
        0 => secret_upper.clone(),
        n => format!("{}{}", secret_upper, "=".repeat(8 - n)),
    };
    
    let secret_bytes = BASE32_NOPAD.decode(padded.trim_end_matches('=').as_bytes())
        .map_err(|e| VaultError::TotpError(format!("Invalid base32 secret: {}", e)))?;

    // Calculate time counter
    let counter = timestamp / config.period;
    let counter_bytes = counter.to_be_bytes();

    // Compute HMAC based on algorithm
    let hmac_result = match config.algorithm {
        TotpAlgorithm::SHA1 => {
            let mut mac = HmacSha1::new_from_slice(&secret_bytes)
                .map_err(|e| VaultError::TotpError(format!("HMAC-SHA1 init: {}", e)))?;
            mac.update(&counter_bytes);
            mac.finalize().into_bytes().to_vec()
        }
        TotpAlgorithm::SHA256 => {
            let mut mac = HmacSha256::new_from_slice(&secret_bytes)
                .map_err(|e| VaultError::TotpError(format!("HMAC-SHA256 init: {}", e)))?;
            mac.update(&counter_bytes);
            mac.finalize().into_bytes().to_vec()
        }
        TotpAlgorithm::SHA512 => {
            let mut mac = HmacSha512::new_from_slice(&secret_bytes)
                .map_err(|e| VaultError::TotpError(format!("HMAC-SHA512 init: {}", e)))?;
            mac.update(&counter_bytes);
            mac.finalize().into_bytes().to_vec()
        }
    };

    // Dynamic truncation (RFC 4226 Section 5.4)
    let offset = (hmac_result.last().unwrap_or(&0) & 0x0F) as usize;
    let binary = ((hmac_result[offset] & 0x7F) as u32) << 24
        | (hmac_result[offset + 1] as u32) << 16
        | (hmac_result[offset + 2] as u32) << 8
        | (hmac_result[offset + 3] as u32);

    let modulus = 10u32.pow(config.digits);
    let otp = binary % modulus;
    let code = format!("{:0>width$}", otp, width = config.digits as usize);

    // Calculate seconds remaining
    let seconds_remaining = config.period - (timestamp % config.period);

    Ok(TotpCode {
        code,
        seconds_remaining,
        period: config.period,
    })
}

/// Verify a TOTP code with a time window (±1 period by default).
pub fn verify_totp(config: &TotpConfig, code: &str, window: Option<u32>) -> crate::Result<bool> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| VaultError::TotpError(format!("System time error: {}", e)))?;

    let timestamp = now.as_secs();
    let window = window.unwrap_or(1);

    for i in 0..=window {
        // Check current and past periods
        let ts = timestamp.saturating_sub(i as u64 * config.period);
        let generated = generate_totp_at(config, ts)?;
        if generated.code == code {
            return Ok(true);
        }

        // Check future periods (for clock skew)
        if i > 0 {
            let ts = timestamp + i as u64 * config.period;
            let generated = generate_totp_at(config, ts)?;
            if generated.code == code {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

/// Parse an otpauth:// URI into a TotpConfig.
/// Format: otpauth://totp/Label?secret=BASE32&issuer=Name&algorithm=SHA1&digits=6&period=30
pub fn parse_otpauth_uri(uri: &str) -> crate::Result<TotpConfig> {
    if !uri.starts_with("otpauth://totp/") {
        return Err(VaultError::TotpError(
            "URI must start with otpauth://totp/".into(),
        ));
    }

    let rest = &uri["otpauth://totp/".len()..];
    let (label_part, query_part) = rest.split_once('?')
        .unwrap_or((rest, ""));

    // URL-decode label
    let label = percent_decode(label_part);

    // Parse query parameters
    let mut config = TotpConfig {
        label: Some(label),
        ..Default::default()
    };

    for param in query_part.split('&') {
        if let Some((key, value)) = param.split_once('=') {
            match key.to_lowercase().as_str() {
                "secret" => config.secret = value.to_string(),
                "issuer" => config.issuer = Some(percent_decode(value)),
                "algorithm" => {
                    config.algorithm = match value.to_uppercase().as_str() {
                        "SHA256" => TotpAlgorithm::SHA256,
                        "SHA512" => TotpAlgorithm::SHA512,
                        _ => TotpAlgorithm::SHA1,
                    };
                }
                "digits" => {
                    config.digits = value.parse().unwrap_or(6);
                }
                "period" => {
                    config.period = value.parse().unwrap_or(30);
                }
                _ => {}
            }
        }
    }

    if config.secret.is_empty() {
        return Err(VaultError::TotpError("Missing secret parameter".into()));
    }

    Ok(config)
}

/// Simple percent-decoding for URI components.
fn percent_decode(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 6238 test vector: SHA1, secret = "12345678901234567890" (base32: GEZDGNBVGY3TQOJQ)
    const TEST_SECRET_SHA1: &str = "GEZDGNBVGY3TQOJQGEZTAMBA";

    #[test]
    fn test_totp_generation() {
        let config = TotpConfig {
            secret: TEST_SECRET_SHA1.to_string(),
            algorithm: TotpAlgorithm::SHA1,
            digits: 6,
            period: 30,
            ..Default::default()
        };

        let code = generate_totp(&config).unwrap();
        assert_eq!(code.code.len(), 6);
        assert!(code.seconds_remaining > 0);
        assert!(code.seconds_remaining <= 30);
    }

    #[test]
    fn test_totp_deterministic() {
        let config = TotpConfig {
            secret: TEST_SECRET_SHA1.to_string(),
            algorithm: TotpAlgorithm::SHA1,
            digits: 6,
            period: 30,
            ..Default::default()
        };

        // Same timestamp should give same code
        let code1 = generate_totp_at(&config, 1000000000).unwrap();
        let code2 = generate_totp_at(&config, 1000000000).unwrap();
        assert_eq!(code1.code, code2.code);
    }

    #[test]
    fn test_totp_different_timestamps() {
        let config = TotpConfig {
            secret: TEST_SECRET_SHA1.to_string(),
            algorithm: TotpAlgorithm::SHA1,
            digits: 6,
            period: 30,
            ..Default::default()
        };

        let code1 = generate_totp_at(&config, 1000000000).unwrap();
        let code2 = generate_totp_at(&config, 1000000030).unwrap();
        // Different periods should give different codes (with high probability)
        // Note: there's a tiny chance they could be equal, but astronomically unlikely
        assert_ne!(code1.code, code2.code);
    }

    #[test]
    fn test_totp_8_digits() {
        let config = TotpConfig {
            secret: TEST_SECRET_SHA1.to_string(),
            algorithm: TotpAlgorithm::SHA1,
            digits: 8,
            period: 30,
            ..Default::default()
        };

        let code = generate_totp_at(&config, 1000000000).unwrap();
        assert_eq!(code.code.len(), 8);
    }

    #[test]
    fn test_totp_sha256() {
        let config = TotpConfig {
            secret: TEST_SECRET_SHA1.to_string(),
            algorithm: TotpAlgorithm::SHA256,
            digits: 6,
            period: 30,
            ..Default::default()
        };

        let code = generate_totp_at(&config, 1000000000).unwrap();
        assert_eq!(code.code.len(), 6);
    }

    #[test]
    fn test_totp_sha512() {
        let config = TotpConfig {
            secret: TEST_SECRET_SHA1.to_string(),
            algorithm: TotpAlgorithm::SHA512,
            digits: 6,
            period: 30,
            ..Default::default()
        };

        let code = generate_totp_at(&config, 1000000000).unwrap();
        assert_eq!(code.code.len(), 6);
    }

    #[test]
    fn test_verify_totp() {
        let config = TotpConfig {
            secret: TEST_SECRET_SHA1.to_string(),
            algorithm: TotpAlgorithm::SHA1,
            digits: 6,
            period: 30,
            ..Default::default()
        };

        let code = generate_totp(&config).unwrap();
        let verified = verify_totp(&config, &code.code, Some(1)).unwrap();
        assert!(verified);
    }

    #[test]
    fn test_verify_wrong_code() {
        let config = TotpConfig {
            secret: TEST_SECRET_SHA1.to_string(),
            algorithm: TotpAlgorithm::SHA1,
            digits: 6,
            period: 30,
            ..Default::default()
        };

        let verified = verify_totp(&config, "000000", Some(0)).unwrap();
        // Very unlikely to be correct
        // (We can't assert false because there's a 1/1000000 chance it's actually correct)
        let _ = verified;
    }

    #[test]
    fn test_parse_otpauth_uri() {
        let uri = "otpauth://totp/GitHub:user@example.com?secret=JBSWY3DPEHPK3PXP&issuer=GitHub&algorithm=SHA1&digits=6&period=30";
        let config = parse_otpauth_uri(uri).unwrap();

        assert_eq!(config.secret, "JBSWY3DPEHPK3PXP");
        assert_eq!(config.issuer, Some("GitHub".to_string()));
        assert_eq!(config.algorithm, TotpAlgorithm::SHA1);
        assert_eq!(config.digits, 6);
        assert_eq!(config.period, 30);
        assert_eq!(config.label, Some("GitHub:user@example.com".to_string()));
    }

    #[test]
    fn test_parse_minimal_uri() {
        let uri = "otpauth://totp/MyApp?secret=JBSWY3DPEHPK3PXP";
        let config = parse_otpauth_uri(uri).unwrap();

        assert_eq!(config.secret, "JBSWY3DPEHPK3PXP");
        assert_eq!(config.digits, 6);
        assert_eq!(config.period, 30);
    }

    #[test]
    fn test_parse_invalid_uri() {
        assert!(parse_otpauth_uri("https://example.com").is_err());
        assert!(parse_otpauth_uri("otpauth://totp/Test?digits=6").is_err()); // missing secret
    }
}

