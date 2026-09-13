//! Fuzz Smoke Tests & Mutation Suite
//!
//! Provides deterministic adversarial input testing across all parser attack surfaces:
//! 1. Malformed .vdb headers & allocation bomb resistance
//! 2. Untrusted Bitwarden JSON exports
//! 3. Malformed KeePass XML archives
//! 4. CSV dialect fuzzing
//! 5. Corrupted attachment decryption & AAD tampering
//! 6. Malicious OTPAuth URIs and period/digits edge cases

use uuid::Uuid;
use yntra_vault_core::crypto::cipher::EncryptedBlob;
use yntra_vault_core::crypto::kdf::EntryKey;
use yntra_vault_core::totp::{generate_totp_at, parse_otpauth_uri};
use yntra_vault_core::vault::manager::VaultManager;
use yntra_vault_core::vault::format::{FileHeader, KdfParams, VaultFile, FORMAT_VERSION};
use yntra_vault_core::vault::importer::{
    clean_totp_secret, parse_csv_matrix, ImportFormat, Importer,
};
use yntra_vault_core::vault::types::FieldScope;

#[test]
fn test_fuzz_vdb_header_allocation_bombs() {
    // 1. Completely empty or undersized buffer
    assert!(VaultFile::from_bytes(&[]).is_err());
    assert!(VaultFile::from_bytes(b"YN").is_err());

    // 2. Valid magic bytes followed by bogus version
    let mut header = b"YNTR\xFF\xFF\x00\x00".to_vec();
    assert!(VaultFile::from_bytes(&header).is_err());

    // 3. Valid magic and version 3, but astronomical KDF length (e.g. 2 GB)
    header = b"YNTR".to_vec();
    header.extend_from_slice(&3u16.to_le_bytes()); // version 3
    header.extend_from_slice(&0u16.to_le_bytes()); // flags 0
    header.extend_from_slice(&[0u8; 32]);          // salt 32 bytes
    header.extend_from_slice(&(2_000_000_000u32).to_le_bytes()); // 2 GB KDF length!
    // Must return Err cleanly without attempting to allocate 2 GB
    assert!(VaultFile::from_bytes(&header).is_err());

    // 4. Astronomical payload length (e.g. u64::MAX)
    let valid_file = VaultFile {
        header: FileHeader {
            version: FORMAT_VERSION,
            flags: 0,
            salt: [1u8; 32],
            kdf_params: KdfParams::default(),
        },
        hmac: None,
        biometric: None,
        hardware2fa: None,
        encrypted_payload: vec![1, 2, 3, 4],
    };
    let mut valid_bytes = valid_file.to_bytes().expect("Valid to_bytes");

    // Mutate the payload length (last 8 bytes before the 4-byte payload) to u64::MAX
    let payload_len_pos = valid_bytes.len() - 4 - 8;
    valid_bytes[payload_len_pos..payload_len_pos + 8].copy_from_slice(&u64::MAX.to_le_bytes());
    // Must reject immediately without OOM
    assert!(VaultFile::from_bytes(&valid_bytes).is_err());
}

#[test]
fn test_fuzz_vdb_header_random_bit_flips() {
    let file = VaultFile {
        header: FileHeader {
            version: FORMAT_VERSION,
            flags: 0,
            salt: [7u8; 32],
            kdf_params: KdfParams::default(),
        },
        hmac: None,
        biometric: None,
        hardware2fa: None,
        encrypted_payload: vec![10, 20, 30, 40, 50],
    };
    let baseline = file.to_bytes().expect("Baseline bytes");

    // Test truncations at every single byte boundary
    for cut in 0..baseline.len() {
        let truncated = &baseline[..cut];
        let _ = VaultFile::from_bytes(truncated);
    }

    // Test single byte mutations across the entire serialized file
    for i in 0..baseline.len() {
        let mut mutated = baseline.clone();
        mutated[i] ^= 0xFF;
        let _ = VaultFile::from_bytes(&mutated);
    }
}

#[test]
fn test_fuzz_import_bitwarden_json_adversarial() {
    let inputs = [
        "",
        "{}",
        "[]",
        "{\"items\": null}",
        "{\"items\": [null, 123, \"str\", {}, true]}",
        "{\"encrypted\": true}",
        "{\"encrypted\": \"not_a_bool\"}",
        "{\"items\": [{\"login\": {\"uris\": [{\"uri\": 123}]}}]}",
        "{\"items\": [{\"type\": 999999, \"login\": null}]}",
        "{\"items\": [{\"fields\": [{\"type\": 9999, \"name\": null, \"value\": null}]}]}",
        "{\"items\": [{\"login\": {\"totp\": \"otpauth://invalid-uri\"}}]}",
        "{\"items\": [{\"login\": {\"totp\": \"\"}}]}",
        "[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]",
        "{\"items\": [{\"name\": \"\0\0\0\x01\x02\x03\r\n\t\"}]}",
    ];

    for input in &inputs {
        let _ = Importer::parse_str(input, ImportFormat::BitwardenJson);
        let _ = Importer::parse_str(input, ImportFormat::AutoDetect);
    }
}

#[test]
fn test_fuzz_import_keepass_xml_adversarial() {
    let inputs = [
        "",
        "<",
        "</>",
        "<KeePassFile>",
        "<KeePassFile></KeePassFile>",
        "<Group><Name></Group>",
        "<Group><Name>Test<Entry><String><Key>Title",
        "<Entry><String><Key>Title</Key><Value>Unclosed Title",
        "<Entry><String><Key>Password</Key><Value>Pass</Value></String></Entry>",
        "<Group><Name>Group</Name><Entry><String><Key>TimeOtp-Secret-Base32</Key><Value>invalid_base32!!</Value></String></Entry></Group>",
        "<<<<Group>>>><<<<Entry>>>></<<<Entry>>>>",
        "<Entry><String><Key>Title</Key><Value>\0\0\0\x01\x02</Value></String></Entry>",
    ];

    for input in &inputs {
        let _ = Importer::parse_str(input, ImportFormat::KeepassXml);
        let _ = Importer::parse_str(input, ImportFormat::AutoDetect);
        let _ = clean_totp_secret(input);
    }
}

#[test]
fn test_fuzz_import_csv_adversarial() {
    let inputs = [
        "",
        "\n",
        "\r\n",
        "\"",
        "\"\"",
        "\"\"\"",
        "\"\"\"\"",
        "a,b,c\n1,2",
        "a,b,c\n1,2,3,4,5",
        "\"unclosed quoted string\nacross multiple lines\nwith commas, and \"\"quotes\"\"",
        "name,username,password\n\0,\0,\0",
        ",,,,,\n,,,,,",
        "folder,name,notes,login_uri,login_username,login_password,login_totp\n\"F\",,\"Note\",\"http://a.com\",\"u\",\"p\",",
    ];

    for input in &inputs {
        let matrix = parse_csv_matrix(input);
        let _ = matrix.len();
        let _ = Importer::parse_str(input, ImportFormat::BitwardenCsv);
        let _ = Importer::parse_str(input, ImportFormat::KeepassCsv);
        let _ = Importer::parse_str(input, ImportFormat::ChromeCsv);
        let _ = Importer::parse_str(input, ImportFormat::GenericCsv);
        let _ = Importer::parse_str(input, ImportFormat::AutoDetect);
    }
}

#[test]
fn test_fuzz_attachments_decryption_adversarial() {
    let entry_key = EntryKey { bytes: [0x5au8; 32] };
    let entry_id = Uuid::from_u128(0x1111_2222_3333_4444);
    let attachment_id = Uuid::from_u128(0x5555_6666_7777_8888);

    // 1. Test empty blob
    let empty_blob = EncryptedBlob {
        nonce: Vec::new(),
        ciphertext: Vec::new(),
    };
    assert!(VaultManager::decrypt_entry_field(
        &empty_blob,
        &entry_key,
        &entry_id,
        FieldScope::Attachment {
            attachment_id: &attachment_id,
        },
    ).is_err());

    // 2. Test invalid nonce lengths (must be 12 or 24)
    for nonce_len in [1, 5, 11, 13, 23, 25, 32, 64] {
        let blob = EncryptedBlob {
            nonce: vec![0u8; nonce_len],
            ciphertext: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        };
        assert!(VaultManager::decrypt_entry_field(
            &blob,
            &entry_key,
            &entry_id,
            FieldScope::Attachment {
                attachment_id: &attachment_id,
            },
        ).is_err());
    }

    // 3. Test tampered ciphertext and tag
    let mut valid_blob = yntra_vault_core::crypto::cipher::encrypt_entry_with_aad(
        b"Sensitive attachment bytes",
        &entry_key,
        b"attachment:test",
    ).expect("encrypt");

    // Tamper with ciphertext
    if let Some(byte) = valid_blob.ciphertext.last_mut() {
        *byte ^= 0x01;
    }
    assert!(VaultManager::decrypt_entry_field(
        &valid_blob,
        &entry_key,
        &entry_id,
        FieldScope::Attachment {
            attachment_id: &attachment_id,
        },
    ).is_err());
}

#[test]
fn test_fuzz_totp_uri_adversarial() {
    let uris = [
        "",
        "otpauth://",
        "otpauth://totp/",
        "otpauth://totp/?secret=",
        "otpauth://totp/?secret=JBSWY3DPEHPK3PXP",
        "otpauth://totp/Test?secret=JBSWY3DPEHPK3PXP&period=0",           // Zero period must not panic!
        "otpauth://totp/Test?secret=JBSWY3DPEHPK3PXP&period=9999999999",  // Large period
        "otpauth://totp/Test?secret=JBSWY3DPEHPK3PXP&digits=0",           // 0 digits must not panic!
        "otpauth://totp/Test?secret=JBSWY3DPEHPK3PXP&digits=10",          // 10 digits must not panic!
        "otpauth://totp/Test?secret=JBSWY3DPEHPK3PXP&digits=100",         // 100 digits must not panic!
        "otpauth://totp/Test?secret=JBSWY3DPEHPK3PXP&algorithm=UNKNOWN",
        "otpauth://totp/Test?secret=JBSWY3DPEHPK3PXP&algorithm=SHA256&digits=8&period=60",
    ];

    for uri in &uris {
        if let Ok(config) = parse_otpauth_uri(uri) {
            let _ = generate_totp_at(&config, 0);
            let _ = generate_totp_at(&config, 1_700_000_000);
            let _ = generate_totp_at(&config, u64::MAX);
            let _ = yntra_vault_core::totp::verify_totp(&config, "123456", None);
            let _ = yntra_vault_core::totp::verify_totp(&config, "000000", Some(5));
        }
    }
}
