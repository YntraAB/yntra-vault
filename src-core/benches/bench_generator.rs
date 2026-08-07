//! Password & Passphrase Generator Benchmarks (Fast Developer Mode)

use criterion::{black_box, Criterion};
use std::time::Duration;
use yntra_vault_core::generator::{generate_password, GeneratorMode, GeneratorOptions};

pub fn bench_generator(c: &mut Criterion) {
    let mut group = c.benchmark_group("Password & Passphrase Generator");
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_millis(500));
    group.sample_size(10);

    let random_16 = GeneratorOptions {
        mode: GeneratorMode::Random,
        length: 16,
        uppercase: true,
        lowercase: true,
        digits: true,
        symbols: true,
        exclude_ambiguous: false,
        ..Default::default()
    };

    let random_64_no_ambig = GeneratorOptions {
        mode: GeneratorMode::Random,
        length: 64,
        uppercase: true,
        lowercase: true,
        digits: true,
        symbols: true,
        exclude_ambiguous: true,
        ..Default::default()
    };

    let diceware_6 = GeneratorOptions {
        mode: GeneratorMode::Diceware,
        word_count: 6,
        separator: "-".to_string(),
        capitalize_words: true,
        add_number: true,
        ..Default::default()
    };

    group.bench_function("CSPRNG Random Password (16 chars)", |b| {
        b.iter(|| generate_password(black_box(&random_16)));
    });

    group.bench_function("CSPRNG Random Password (64 chars, No Ambiguous)", |b| {
        b.iter(|| generate_password(black_box(&random_64_no_ambig)));
    });

    group.bench_function("Diceware Passphrase (6 words, Capitalized + Number)", |b| {
        b.iter(|| generate_password(black_box(&diceware_6)));
    });

    group.finish();
}
