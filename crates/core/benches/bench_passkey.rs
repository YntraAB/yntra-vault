//! Passkey (FIDO2 / WebAuthn) Cryptography Benchmarks

use criterion::{black_box, Criterion};
use std::time::Duration;
use yntra_vault_core::crypto::passkey::{
    generate_passkey_pair, sign_assertion, verify_assertion,
};

pub fn bench_passkey(c: &mut Criterion) {
    let mut group = c.benchmark_group("Passkey (FIDO2 / WebAuthn) ECDSA P-256");
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_millis(500));
    group.sample_size(10);

    group.bench_function("Passkey Keypair Generation (P-256)", |b| {
        b.iter(|| generate_passkey_pair().unwrap());
    });

    let pair = generate_passkey_pair().expect("Generate passkey pair");
    let auth_data = b"authenticator_data_32_bytes_test";
    let client_hash = b"client_data_hash_32_bytes_sample";

    group.bench_function("Assertion Signature Generation (Sign)", |b| {
        b.iter(|| {
            sign_assertion(
                black_box(&pair.private_key),
                black_box(auth_data),
                black_box(client_hash),
            )
            .unwrap()
        });
    });

    let sig = sign_assertion(&pair.private_key, auth_data, client_hash).unwrap();

    group.bench_function("Assertion Signature Verification (Verify)", |b| {
        b.iter(|| {
            verify_assertion(
                black_box(&pair.public_key),
                black_box(auth_data),
                black_box(client_hash),
                black_box(&sig),
            )
            .unwrap()
        });
    });

    group.finish();
}
