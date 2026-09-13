#![no_main]

use libfuzzer_sys::fuzz_target;
use yntra_vault_core::vault::format::VaultFile;

fuzz_target!(|data: &[u8]| {
    // Fuzz binary .vdb parsing
    if let Ok(vault_file) = VaultFile::from_bytes(data) {
        // Exercise header AAD and re-serialization
        let _ = vault_file.header.aad_bytes();
        let _ = vault_file.header.kdf_params.validate();
        if let Ok(serialized) = vault_file.to_bytes() {
            let _ = VaultFile::from_bytes(&serialized);
        }
    }
});
