//! Multi-Layer Encryption Pipeline Benchmarks

use criterion::{black_box, BenchmarkId, Criterion, Throughput};
use std::time::Duration;
use yntra_vault_core::crypto::cipher::{
    compute_hmac, decrypt_entry, decrypt_vault, encrypt_entry, encrypt_vault, verify_hmac,
};
use yntra_vault_core::crypto::kdf::{EntryKey, HmacKey, VaultKey};

fn dummy_vault_key() -> VaultKey {
    VaultKey { bytes: [42u8; 32] }
}

fn dummy_entry_key() -> EntryKey {
    EntryKey { bytes: [84u8; 32] }
}

fn dummy_hmac_key() -> HmacKey {
    HmacKey { bytes: [126u8; 64] }
}

pub fn bench_cipher(c: &mut Criterion) {
    let mut group = c.benchmark_group("Multi-Layer Encryption Pipeline");
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_millis(500));
    group.sample_size(15);

    let vault_key = dummy_vault_key();
    let entry_key = dummy_entry_key();
    let hmac_key = dummy_hmac_key();

    for size in [1024, 64 * 1024, 1024 * 1024, 10 * 1024 * 1024].iter() {
        let plaintext = vec![0x5A; *size];
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("XChaCha20-Poly1305 Encrypt", format!("{} B", size)),
            &plaintext,
            |b, payload| {
                b.iter(|| encrypt_vault(black_box(payload), black_box(&vault_key)).unwrap());
            },
        );

        let encrypted = encrypt_vault(&plaintext, &vault_key).unwrap();
        group.bench_with_input(
            BenchmarkId::new("XChaCha20-Poly1305 Decrypt", format!("{} B", size)),
            &encrypted,
            |b, blob| {
                b.iter(|| decrypt_vault(black_box(blob), black_box(&vault_key)).unwrap());
            },
        );
    }

    let entry_credential = b"StrictlyConfidentialP@ssw0rd!2026";
    group.throughput(Throughput::Bytes(entry_credential.len() as u64));

    group.bench_function("XChaCha20-Poly1305 Entry Encrypt", |b| {
        b.iter(|| encrypt_entry(black_box(entry_credential), black_box(&entry_key)).unwrap());
    });

    let encrypted_entry = encrypt_entry(entry_credential, &entry_key).unwrap();
    group.bench_function("XChaCha20-Poly1305 Entry Decrypt", |b| {
        b.iter(|| decrypt_entry(black_box(&encrypted_entry), black_box(&entry_key)).unwrap());
    });

    let payload = vec![0x7E; 64 * 1024];
    group.throughput(Throughput::Bytes(payload.len() as u64));

    group.bench_function("HMAC-SHA512 Compute (64KB)", |b| {
        b.iter(|| compute_hmac(black_box(&payload), black_box(&hmac_key)));
    });

    let mac = compute_hmac(&payload, &hmac_key);
    group.bench_function("HMAC-SHA512 Verify (64KB)", |b| {
        b.iter(|| verify_hmac(black_box(&payload), black_box(&mac), black_box(&hmac_key)).unwrap());
    });

    group.finish();
}
