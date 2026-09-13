//! Key Derivation Function (KDF) Benchmarks

use criterion::{black_box, Criterion};
use std::time::Duration;
use yntra_vault_core::crypto::kdf::{derive_master_key, derive_subkeys, generate_salt};

pub fn bench_kdf(c: &mut Criterion) {
    let mut group = c.benchmark_group("Key Derivation (KDF)");
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_millis(500));
    group.sample_size(10);

    let password = b"Super_Secure_Master_Password_2026!";
    let salt = generate_salt();

    group.bench_function("Salt Generation (32 bytes)", |b| {
        b.iter(|| generate_salt());
    });

    let master_key = derive_master_key(password, &salt).expect("Master key derivation");

    group.bench_function("HKDF-SHA512 Subkeys Derivation", |b| {
        b.iter(|| derive_subkeys(black_box(&master_key)).unwrap());
    });

    group.bench_function("Argon2id Master Key Derivation (256MB RAM)", |b| {
        b.iter(|| derive_master_key(black_box(password), black_box(&salt)).unwrap());
    });

    group.finish();
}
