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
        Ok(())
    }

    fn run_smart_autotype(
        &self,
        guard: AutotypeGuard,
        _url: &str,
        _launch_browser: bool,
        char_delay_ms: u64,
        _field_delay_ms: u64,
    ) -> crate::Result<()> {
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(3));
            let driver = StubAutotypeDriver::new();
            let _ = driver.autotype_text_with_delay(&guard.username, char_delay_ms, 0);
            let _ = driver.autotype_text_with_delay("\t", char_delay_ms, 0);
            let _ = driver.autotype_text_with_delay(&guard.password, char_delay_ms, 0);
        });
        Ok(())
    }
}
