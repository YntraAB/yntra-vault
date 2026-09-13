#![no_main]

use libfuzzer_sys::fuzz_target;
use uuid::Uuid;
use yntra_vault_core::crypto::cipher::EncryptedBlob;
use yntra_vault_core::crypto::kdf::EntryKey;
use yntra_vault_core::vault::manager::VaultManager;
use yntra_vault_core::vault::types::FieldScope;

fuzz_target!(|data: &[u8]| {
    let (nonce, ciphertext) = if data.is_empty() {
        (&[][..], &[][..])
    } else {
        let nonce_len = (data[0] as usize) % 32;
        let rest = &data[1..];
        if rest.len() >= nonce_len {
            rest.split_at(nonce_len)
        } else {
            (rest, &[][..])
        }
    };

    let blob = EncryptedBlob {
        nonce: nonce.to_vec(),
        ciphertext: ciphertext.to_vec(),
    };

    let entry_key = EntryKey { bytes: [0x42u8; 32] };
    let entry_id = Uuid::from_u128(0x1234_5678_90ab_cdef);
    let attachment_id = Uuid::from_u128(0xfedc_ba09_8765_4321);

    // Decrypting untrusted attachment data must safely return Err on tampering or invalid nonces
    let _ = VaultManager::decrypt_entry_field(
        &blob,
        &entry_key,
        &entry_id,
        FieldScope::Attachment {
            attachment_id: &attachment_id,
        },
    );
});
