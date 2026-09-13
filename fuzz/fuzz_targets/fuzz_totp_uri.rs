#![no_main]

use libfuzzer_sys::fuzz_target;
use yntra_vault_core::totp::{generate_totp_at, parse_otpauth_uri, verify_totp};

fuzz_target!(|data: &[u8]| {
    if let Ok(utf8_str) = std::str::from_utf8(data) {
        if let Ok(config) = parse_otpauth_uri(utf8_str) {
            let _ = generate_totp_at(&config, 0);
            let _ = generate_totp_at(&config, 1_700_000_000);
            let _ = generate_totp_at(&config, u64::MAX / 2);
            let _ = verify_totp(&config, "123456", None);
            let _ = verify_totp(&config, "000000", Some(2));
        }
    }
});
