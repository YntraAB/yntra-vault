//! Disk I/O & Atomic File Save Benchmarks

use criterion::{black_box, BenchmarkId, Criterion};
use std::time::Duration;
use tempfile::tempdir;
use yntra_vault_core::vault::manager::{NewEntry, VaultManager};
use yntra_vault_core::vault::types::EntryType;

fn create_populated_vault(count: usize) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempdir().expect("Failed to create temp dir");
    let file_path = dir.path().join("bench_vault.vdb");
    let password = "TestMasterPassword123!";

    let mut manager = VaultManager::create("Benchmark Vault", password, &file_path)
        .expect("Failed to create test vault");

    for i in 0..count {
        let new_entry = NewEntry {
            title: format!("Production Server {}", i),
            username: format!("admin_{}@infra.net", i),
            password: format!("P@ssw0rd_Server_{}!", i),
            url: format!("https://server-{}.infra.net:8443", i),
            email: format!("admin_{}@infra.net", i),
            notes: format!("Multi-line production credentials notes for node {}", i),
            tags: vec!["production".to_string(), "infrastructure".to_string(), "k8s".to_string()],
            totp_secret: None,
            custom_fields: Vec::new(),
            entry_type: Some(EntryType::Login),
            generate_passkey: None,
            attachments: None,
        };
        manager.add_entry(new_entry).expect("Add entry");
    }

    manager.save().expect("Initial save");
    (dir, file_path)
}

pub fn bench_io_atomic_save(c: &mut Criterion) {
    let mut group = c.benchmark_group("Disk I/O Atomic Operations");
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_millis(500));
    group.sample_size(10);

    for count in [10, 100, 1000].iter() {
        let (_dir, file_path) = create_populated_vault(*count);

        let mut manager = VaultManager::open(&file_path, "TestMasterPassword123!")
            .expect("Open vault for benchmark");

        group.bench_with_input(
            BenchmarkId::new("Atomic Vault File Save", format!("{} entries", count)),
            &count,
            |b, _| {
                b.iter(|| {
                    manager.save().expect("Atomic save failed");
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("Vault Disk Open & Decrypt", format!("{} entries", count)),
            &file_path,
            |b, path| {
                b.iter(|| {
                    let _v = VaultManager::open(black_box(path), black_box("TestMasterPassword123!"))
                        .expect("Open vault failed");
                });
            },
        );
    }

    group.finish();
}
