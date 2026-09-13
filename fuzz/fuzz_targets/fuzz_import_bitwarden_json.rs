#![no_main]

use libfuzzer_sys::fuzz_target;
use yntra_vault_core::vault::importer::{Importer, ImportFormat};

fuzz_target!(|data: &[u8]| {
    if let Ok(utf8_str) = std::str::from_utf8(data) {
        let _ = Importer::parse_str(utf8_str, ImportFormat::BitwardenJson);
    }
});
