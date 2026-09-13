use criterion::Criterion;
use std::time::Duration;
use yntra_vault_core::crypto::clipboard::copy_to_clipboard_defended;

pub fn bench_clipboard(c: &mut Criterion) {
    let mut group = c.benchmark_group("clipboard_defense");
    group.sample_size(10);
    group.measurement_time(Duration::from_millis(500));

    let secret = "SuperSecretPassword123!@#$";

    group.bench_function("copy_defended", |b| {
        b.iter(|| {
            let _ = copy_to_clipboard_defended(secret, true, None);
        });
    });

    group.finish();
}
