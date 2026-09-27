use crate::{Result, VaultError};

pub const MAX_DISPLAY_NAME_LENGTH: usize = 128;

/// Apply only when choosing a new credential, never to legacy unlocks.
pub fn validate_new_master_password(password: &str) -> Result<()> {
    if password.trim().is_empty() || password.chars().count() < 12 {
        return Err(VaultError::InvalidState("Choose a non-blank password of at least 12 characters".into()));
    }
    Ok(())
}

pub fn validate_display_name(value: &str) -> Result<()> {
    if value.encode_utf16().take(MAX_DISPLAY_NAME_LENGTH + 1).count() > MAX_DISPLAY_NAME_LENGTH {
        return Err(VaultError::InvalidFormat(format!("Display names must not exceed {} characters", MAX_DISPLAY_NAME_LENGTH)));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_password_policy_counts_characters_without_normalizing_secrets() {
        for password in ["", "short", "            ", "\t\n            ", "🔐🔐🔐🔐🔐🔐"] {
            assert!(validate_new_master_password(password).is_err());
        }
        assert!(validate_new_master_password("🔐🔐🔐🔐🔐🔐🔐🔐🔐🔐🔐🔐").is_ok());
        assert!(validate_new_master_password("  valid passphrase  ").is_ok());
    }

    #[test]
    fn display_names_match_html_length_limits() {
        assert!(validate_display_name(&"å".repeat(128)).is_ok());
        assert!(validate_display_name(&"x".repeat(129)).is_err());
        assert!(validate_display_name(&"🔐".repeat(64)).is_ok());
        assert!(validate_display_name(&"🔐".repeat(65)).is_err());
    }
}
