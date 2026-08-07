//! Memory Safety & Zeroization Benchmarks (Fast Developer Mode)

use criterion::{black_box, BenchmarkId, Criterion};
use std::time::Duration;
use yntra_vault_core::crypto::kdf::{derive_master_key, derive_subkeys, generate_salt};
use yntra_vault_core::crypto::mem::ScrambledString;
use zeroize::Zeroizing;

pub fn bench_memory_zeroize(c: &mut Criterion) {
    let mut group = c.benchmark_group("Memory Safety & Zeroization Cost");
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_millis(500));
    group.sample_size(10);

    let salt = generate_salt();
    let master_key = derive_master_key(b"TestPassword123!", &salt).unwrap();

    group.bench_function("SubKeys Allocation & ZeroizeOnDrop", |b| {
        b.iter(|| {
            let subkeys = derive_subkeys(black_box(&master_key)).unwrap();
            black_box(subkeys);
        });
    });

    for size in [32, 256, 4096, 65536].iter() {
        group.bench_with_input(
            BenchmarkId::new("Zeroizing Buffer Allocation & Drop", format!("{} B", size)),
            size,
            |b, &s| {
                b.iter(|| {
                    let mut buf = Zeroizing::new(vec![0xAAu8; s]);
                    black_box(&mut buf);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("Standard Vector Allocation & Drop (Un-zeroed)", format!("{} B", size)),
            size,
            |b, &s| {
                b.iter(|| {
                    let mut buf = vec![0xAAu8; s];
                    black_box(&mut buf);
                });
            },
        );
    }

    let sensitive_text = "Master_Super_Secret_Password_Value_2026!";
    group.bench_function("ScrambledString Heap Encrypt", |b| {
        b.iter(|| ScrambledString::new(black_box(sensitive_text)));
    });

    let scrambled = ScrambledString::new(sensitive_text);
    group.bench_function("ScrambledString Heap Decrypt & Zeroizing Drop", |b| {
        b.iter(|| {
            let decrypted = scrambled.decrypt().unwrap();
            black_box(decrypted);
        });
    });

    group.finish();
}
