//! Vault Search Tokenization Benchmarks

use criterion::{black_box, Criterion};
use std::time::Duration;
use yntra_vault_core::vault::search::{generate_trigrams, hash_trigram};

pub fn bench_vault_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("Encrypted Search Engine");
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_millis(500));
    group.sample_size(10);

    let text = "https://github.com/yntra/vault-private production-infrastructure-service";
    let key = [42u8; 32];

    group.bench_function("Generate Trigrams (Text String)", |b| {
        b.iter(|| generate_trigrams(black_box(text)));
    });

    let trigram = "ynt";
    group.bench_function("HMAC-SHA256 Trigram Tokenization", |b| {
        b.iter(|| hash_trigram(black_box(trigram), black_box(&key)));
    });

    group.finish();
}
