#![no_main]

use libfuzzer_sys::fuzz_target;
use yntra_vault_core::vault::importer::{parse_csv_matrix, ImportFormat, Importer};

fuzz_target!(|data: &[u8]| {
    if let Ok(utf8_str) = std::str::from_utf8(data) {
        let _ = parse_csv_matrix(utf8_str);
        let _ = Importer::parse_str(utf8_str, ImportFormat::BitwardenCsv);
        let _ = Importer::parse_str(utf8_str, ImportFormat::OnePasswordCsv);
        let _ = Importer::parse_str(utf8_str, ImportFormat::KeepassCsv);
        let _ = Importer::parse_str(utf8_str, ImportFormat::ChromeCsv);
        let _ = Importer::parse_str(utf8_str, ImportFormat::LastPassCsv);
        let _ = Importer::parse_str(utf8_str, ImportFormat::DashlaneCsv);
        let _ = Importer::parse_str(utf8_str, ImportFormat::GenericCsv);
    }
});
