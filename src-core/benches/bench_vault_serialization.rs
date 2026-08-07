//! Vault Serialization Benchmarks

use chrono::Utc;
use criterion::{black_box, BenchmarkId, Criterion, Throughput};
use std::time::Duration;
use uuid::Uuid;
use yntra_vault_core::crypto::cipher::EncryptedBlob;
use yntra_vault_core::vault::format::{FileHeader, KdfParams, VaultFile};
use yntra_vault_core::vault::types::{
    BreachStatus, CustomField, Entry, EntryType, FieldType, VaultData, VaultMetadata,
};

fn create_sample_entries(count: usize) -> Vec<Entry> {
    let mut entries = Vec::with_capacity(count);
    let now = Utc::now();
    for i in 0..count {
        let entry = Entry {
            id: Uuid::new_v4(),
            title: format!("Service Account {}", i),
            username: format!("user_{}@example.com", i),
            encrypted_password: EncryptedBlob {
                nonce: vec![0u8; 12],
                ciphertext: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            },
            url: format!("https://service-{}.example.com/login", i),
            email: format!("user_{}@example.com", i),
            notes: format!("Encrypted note for entry {}", i),
            tags: vec!["production".to_string(), "infrastructure".to_string()],
            favorite: i % 5 == 0,
            pinned: false,
            encrypted_totp_secret: None,
            custom_fields: vec![CustomField {
                id: Uuid::new_v4(),
                name: "API_KEY".to_string(),
                field_type: FieldType::Password,
                value: "secret_api_key_123".to_string(),
                sensitive: true,
            }],
            entry_type: EntryType::Login,
            created_at: now,
            updated_at: now,
            password_history: Vec::new(),
            breach_status: BreachStatus::Unknown,
            strength_score: None,
            password_changed_at: now,
            encrypted_passkey: None,
            passkey_public_key: None,
        };
        entries.push(entry);
    }
    entries
}

pub fn bench_vault_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("Vault Serialization & Format (.vdb)");
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_millis(500));
    group.sample_size(15);

    for entry_count in [10, 100, 1000].iter() {
        let entries = create_sample_entries(*entry_count);
        let vault_data = VaultData {
            metadata: VaultMetadata {
                id: Uuid::new_v4(),
                name: "Benchmark Vault".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                entry_count: *entry_count,
                version: 2,
            },
            entries,
            tags: vec![],
            trash: vec![],
            settings: Default::default(),
        };

        let serialized_bincode = bincode::serialize(&vault_data).unwrap();
        group.throughput(Throughput::Bytes(serialized_bincode.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("Bincode Payload Serialize", format!("{} entries", entry_count)),
            &vault_data,
            |b, v| {
                b.iter(|| bincode::serialize(black_box(v)).unwrap());
            },
        );

        group.bench_with_input(
            BenchmarkId::new("Bincode Payload Deserialize", format!("{} entries", entry_count)),
            &serialized_bincode,
            |b, bytes| {
                b.iter(|| bincode::deserialize::<VaultData>(black_box(bytes)).unwrap());
            },
        );

        let envelope = VaultFile {
            header: FileHeader {
                version: 2,
                flags: 0,
                salt: [1u8; 32],
                kdf_params: KdfParams::default(),
            },
            hmac: Some([7u8; 64]),
            biometric: None,
            hardware2fa: None,
            encrypted_payload: serialized_bincode.clone(),
        };

        let envelope_bytes = envelope.to_bytes().unwrap();

        group.bench_with_input(
            BenchmarkId::new(".vdb Envelope Build Bytes", format!("{} entries", entry_count)),
            &envelope,
            |b, env| {
                b.iter(|| env.to_bytes().unwrap());
            },
        );

        group.bench_with_input(
            BenchmarkId::new(".vdb Envelope Parse Bytes", format!("{} entries", entry_count)),
            &envelope_bytes,
            |b, bytes| {
                b.iter(|| VaultFile::from_bytes(black_box(bytes)).unwrap());
            },
        );
    }

    group.finish();
}
