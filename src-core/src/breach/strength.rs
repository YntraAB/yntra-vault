//! Password strength analyzer
//!
//! Calculates entropy, estimates crack time, and identifies weaknesses.
//! Runs in real-time on every keystroke in the frontend.

use crate::vault::types::{StrengthScore, StrengthLevel};

/// Common passwords to check against (top 200).
const COMMON_PASSWORDS: &[&str] = &[
    "password", "123456", "12345678", "qwerty", "abc123", "monkey", "1234567",
    "letmein", "trustno1", "dragon", "baseball", "iloveyou", "master", "sunshine",
    "ashley", "bailey", "shadow", "123123", "654321", "superman", "qazwsx",
    "michael", "football", "password1", "password123", "admin", "welcome",
    "login", "starwars", "hello", "charlie", "donald", "princess", "access",
    "freedom", "whatever", "mustang", "batman", "passw0rd", "hunter2",
];

/// Common patterns that reduce entropy.
const COMMON_PATTERNS: &[&str] = &[
    "1234", "abcd", "qwer", "asdf", "zxcv", "!@#$", "0000", "1111",
    "aaaa", "pass", "word", "love", "user", "name", "test",
];

/// Analyze a password and return a comprehensive strength score.
pub fn analyze_password(password: &str) -> StrengthScore {
    let mut warnings = Vec::new();
    let len = password.len();

    // Check if it's a common password
    if COMMON_PASSWORDS.contains(&password.to_lowercase().as_str()) {
        return StrengthScore {
            entropy_bits: 0.0,
            crack_time: "instantly".to_string(),
            level: StrengthLevel::Critical,
            warnings: vec!["This is a commonly used password".to_string()],
        };
    }

    // Calculate character set size
    let has_lower = password.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = password.chars().any(|c| c.is_ascii_uppercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_symbol = password.chars().any(|c| !c.is_alphanumeric() && c.is_ascii());
    let has_unicode = password.chars().any(|c| !c.is_ascii());

    let mut charset_size: f64 = 0.0;
    if has_lower { charset_size += 26.0; }
    if has_upper { charset_size += 26.0; }
    if has_digit { charset_size += 10.0; }
    if has_symbol { charset_size += 33.0; }
    if has_unicode { charset_size += 100.0; } // Approximate

    if charset_size == 0.0 {
        charset_size = 26.0; // Fallback
    }

    // Base entropy: log2(charset_size) * length
    let mut entropy = charset_size.log2() * len as f64;

    // ─── Penalties ───────────────────────────────────────────────

    // Length penalty
    if len < 8 {
        warnings.push(format!("Too short ({} characters, recommend 12+)", len));
        entropy *= 0.7;
    } else if len < 12 {
        warnings.push("Consider using 12+ characters".to_string());
    }

    // Single character type
    if [has_lower, has_upper, has_digit, has_symbol].iter().filter(|&&x| x).count() == 1 {
        warnings.push("Uses only one character type".to_string());
        entropy *= 0.8;
    }

    // Check for common patterns
    let pw_lower = password.to_lowercase();
    for pattern in COMMON_PATTERNS {
        if pw_lower.contains(pattern) {
            warnings.push(format!("Contains common pattern: '{}'", pattern));
            entropy *= 0.85;
            break;
        }
    }

    // Check for repeated characters
    let mut max_repeat = 1;
    let mut current_repeat = 1;
    let chars: Vec<char> = password.chars().collect();
    for i in 1..chars.len() {
        if chars[i] == chars[i - 1] {
            current_repeat += 1;
            max_repeat = max_repeat.max(current_repeat);
        } else {
            current_repeat = 1;
        }
    }
    if max_repeat >= 3 {
        warnings.push(format!("Contains {} repeated characters", max_repeat));
        entropy *= 0.9;
    }

    // Check for sequential characters (abc, 123, etc.)
    let mut sequential = 0;
    for i in 2..chars.len() {
        let a = chars[i - 2] as i32;
        let b = chars[i - 1] as i32;
        let c = chars[i] as i32;
        if (b - a == 1 && c - b == 1) || (a - b == 1 && b - c == 1) {
            sequential += 1;
        }
    }
    if sequential >= 3 {
        warnings.push("Contains sequential characters".to_string());
        entropy *= 0.85;
    }

    // No warnings means good practices
    if !has_upper && len >= 8 { warnings.push("Add uppercase letters".to_string()); }
    if !has_digit && len >= 8 { warnings.push("Add numbers".to_string()); }
    if !has_symbol && len >= 8 { warnings.push("Add special characters".to_string()); }

    // Classify
    let level = match entropy as u32 {
        0..=24 => StrengthLevel::Critical,
        25..=49 => StrengthLevel::Weak,
        50..=74 => StrengthLevel::Fair,
        75..=99 => StrengthLevel::Strong,
        _ => StrengthLevel::Excellent,
    };

    let crack_time = estimate_crack_time(entropy);

    StrengthScore {
        entropy_bits: (entropy * 10.0).round() / 10.0,
        crack_time,
        level,
        warnings,
    }
}

/// Estimate how long it would take to crack a password.
/// Assumes 10 billion guesses per second (modern GPU cluster).
fn estimate_crack_time(entropy_bits: f64) -> String {
    if entropy_bits <= 0.0 {
        return "instantly".to_string();
    }

    // 10^10 guesses per second
    let guesses_per_second: f64 = 10_000_000_000.0;
    let total_combinations = 2.0f64.powf(entropy_bits);
    let seconds = total_combinations / guesses_per_second / 2.0; // Average case

    if seconds < 1.0 {
        "instantly".to_string()
    } else if seconds < 60.0 {
        format!("{:.0} seconds", seconds)
    } else if seconds < 3600.0 {
        format!("{:.0} minutes", seconds / 60.0)
    } else if seconds < 86400.0 {
        format!("{:.0} hours", seconds / 3600.0)
    } else if seconds < 86400.0 * 365.0 {
        format!("{:.0} days", seconds / 86400.0)
    } else if seconds < 86400.0 * 365.0 * 1000.0 {
        format!("{:.0} years", seconds / (86400.0 * 365.0))
    } else if seconds < 86400.0 * 365.0 * 1_000_000.0 {
        format!("{:.0} thousand years", seconds / (86400.0 * 365.0 * 1000.0))
    } else if seconds < 86400.0 * 365.0 * 1_000_000_000.0 {
        format!("{:.0} million years", seconds / (86400.0 * 365.0 * 1_000_000.0))
    } else {
        format!("{:.0} billion years", seconds / (86400.0 * 365.0 * 1_000_000_000.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_common_password() {
        let score = analyze_password("password");
        assert_eq!(score.level, StrengthLevel::Critical);
        assert_eq!(score.entropy_bits, 0.0);
    }

    #[test]
    fn test_short_password() {
        let score = analyze_password("abc");
        assert!(score.level <= StrengthLevel::Weak);
        assert!(score.warnings.iter().any(|w| w.contains("short")));
    }

    #[test]
    fn test_strong_password() {
        let score = analyze_password("Kj#9xM!qR2$vNp8&wL4z");
        assert!(score.level >= StrengthLevel::Strong);
        assert!(score.entropy_bits > 75.0);
    }

    #[test]
    fn test_excellent_password() {
        let score = analyze_password("c0rr3ct-h0rse-b@tt3ry-st@pl3-!42");
        assert!(score.level >= StrengthLevel::Strong);
    }

    #[test]
    fn test_only_lowercase() {
        let score = analyze_password("abcdefghijklmnop");
        assert!(score.warnings.iter().any(|w| w.contains("one character type") || w.contains("Add")));
    }

    #[test]
    fn test_repeated_characters() {
        let score = analyze_password("aaaaaBBBBB11111");
        assert!(score.warnings.iter().any(|w| w.contains("repeated")));
    }

    #[test]
    fn test_crack_time_format() {
        assert_eq!(estimate_crack_time(0.0), "instantly");
        assert!(estimate_crack_time(10.0) == "instantly"); // 2^10 / 10B = instant
        let t60 = estimate_crack_time(60.0);
        assert!(t60.contains("year") || t60.contains("day"), "60 bits: {}", t60);
        let t128 = estimate_crack_time(128.0);
        assert!(t128.contains("billion") || t128.contains("million") || t128.contains("year"), "128 bits: {}", t128);
    }

    #[test]
    fn test_entropy_increases_with_length() {
        let short = analyze_password("Aa1!");
        let long = analyze_password("Aa1!Bb2@Cc3#Dd4$");
        assert!(long.entropy_bits > short.entropy_bits);
    }

    #[test]
    fn test_entropy_increases_with_charset() {
        let letters_only = analyze_password("abcdefghijklmnop");
        let mixed = analyze_password("aBcDeFgH1234!@#$");
        assert!(mixed.entropy_bits > letters_only.entropy_bits);
    }
}
