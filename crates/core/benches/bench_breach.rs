//! Breach Detection & Password Strength Benchmarks

use criterion::{black_box, BenchmarkId, Criterion};
use std::time::Duration;
use yntra_vault_core::breach::bloom::is_breach_suspected;
use yntra_vault_core::breach::strength::analyze_password;

pub fn bench_breach_and_strength(c: &mut Criterion) {
    let mut group = c.benchmark_group("Breach & Password Strength");
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_millis(500));
    group.sample_size(10);

    let test_passwords = [
        "password123",
        "Tr0ub4dor&3",
        "CorrectHorseBatteryStaple2026!",
        "vK9#mQ2$pL7@xN4!",
    ];

    for pwd in &test_passwords {
        group.bench_with_input(
            BenchmarkId::new("Local Bloom Filter Lookup", pwd),
            pwd,
            |b, p| {
                b.iter(|| is_breach_suspected(black_box(p)));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("Password Strength Analysis", pwd),
            pwd,
            |b, p| {
                b.iter(|| analyze_password(black_box(p)));
            },
        );
    }

    group.finish();
}
