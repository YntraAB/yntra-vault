//! TOTP Engine (RFC 6238) Benchmarks (Fast Developer Mode)

use criterion::{black_box, Criterion};
use std::time::Duration;
use yntra_vault_core::totp::{generate_totp_at, TotpAlgorithm, TotpConfig};

pub fn bench_totp(c: &mut Criterion) {
    let mut group = c.benchmark_group("TOTP Engine (RFC 6238)");
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_millis(500));
    group.sample_size(10);

    let secret = "JBSWY3DPEHPK3PXP"; // Base32 test secret

    let config_sha1 = TotpConfig {
        secret: secret.to_string(),
        algorithm: TotpAlgorithm::SHA1,
        digits: 6,
        period: 30,
        issuer: Some("Google".to_string()),
        label: Some("test@example.com".to_string()),
    };

    let config_sha256 = TotpConfig {
        secret: secret.to_string(),
        algorithm: TotpAlgorithm::SHA256,
        digits: 6,
        period: 30,
        issuer: Some("AWS".to_string()),
        label: Some("admin@example.com".to_string()),
    };

    let config_sha512 = TotpConfig {
        secret: secret.to_string(),
        algorithm: TotpAlgorithm::SHA512,
        digits: 8,
        period: 30,
        issuer: Some("GitHub".to_string()),
        label: Some("security@example.com".to_string()),
    };

    let timestamp = 1700000000u64;

    group.bench_function("TOTP Generate (SHA-1 6-digit)", |b| {
        b.iter(|| generate_totp_at(black_box(&config_sha1), black_box(timestamp)).unwrap());
    });

    group.bench_function("TOTP Generate (SHA-256 6-digit)", |b| {
        b.iter(|| generate_totp_at(black_box(&config_sha256), black_box(timestamp)).unwrap());
    });

    group.bench_function("TOTP Generate (SHA-512 8-digit)", |b| {
        b.iter(|| generate_totp_at(black_box(&config_sha512), black_box(timestamp)).unwrap());
    });

    group.finish();
}
