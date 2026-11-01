//! Stub implementation of AutotypeDriver for macOS and Linux.

use super::{AutotypeDriver, AutotypeGuard};

pub(crate) struct StubAutotypeDriver;

impl StubAutotypeDriver {
    pub(crate) const fn new() -> Self {
        Self
    }
}

impl AutotypeDriver for StubAutotypeDriver {
    fn autotype_text_with_delay(
        &self,
        _text: &str,
        _char_delay_ms: u64,
        _settle_delay_ms: u64,
    ) -> crate::Result<()> {
        Err(crate::VaultError::InvalidState("Auto-Type is not supported on this platform".into()))
    }

    fn run_smart_autotype(
        &self,
        _guard: AutotypeGuard,
        _url: &str,
        _launch_browser: bool,
        _char_delay_ms: u64,
        _field_delay_ms: u64,
    ) -> crate::Result<()> {
        Err(crate::VaultError::InvalidState("Auto-Type is not supported on this platform".into()))
    }
}
