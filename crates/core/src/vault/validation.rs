use crate::{Result, VaultError};

pub const MAX_DISPLAY_NAME_LENGTH: usize = 128;

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
    fn display_names_match_html_length_limits() {
        assert!(validate_display_name(&"å".repeat(128)).is_ok());
        assert!(validate_display_name(&"x".repeat(129)).is_err());
        assert!(validate_display_name(&"🔐".repeat(64)).is_ok());
        assert!(validate_display_name(&"🔐".repeat(65)).is_err());
    }
}
