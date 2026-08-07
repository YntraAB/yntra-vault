//! Concurrency & Mutex Contention Benchmarks

use criterion::{black_box, BenchmarkId, Criterion};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tempfile::tempdir;
use yntra_vault_core::vault::manager::{NewEntry, VaultManager};
use yntra_vault_core::vault::types::EntryType;

fn setup_concurrent_vault(count: usize) -> (tempfile::TempDir, Arc<Mutex<VaultManager>>) {
    let dir = tempdir().expect("Failed to create temp dir");
    let file_path = dir.path().join("bench_concurrent_vault.vdb");
    let password = "TestMasterPassword123!";

    let mut manager = VaultManager::create("Concurrent Vault", password, &file_path)
        .expect("Failed to create test vault");

    for i in 0..count {
        let new_entry = NewEntry {
            title: format!("Concurrent Service {}", i),
            username: format!("user_{}@cluster.org", i),
            password: format!("P@ssw0rd_Node_{}!", i),
            url: format!("https://service-{}.cluster.org:8443", i),
            email: format!("user_{}@cluster.org", i),
            notes: format!("Concurrent read/write test notes {}", i),
            tags: vec!["concurrent".to_string(), "k8s".to_string()],
            totp_secret: None,
            custom_fields: Vec::new(),
            entry_type: Some(EntryType::Login),
            generate_passkey: None,
        };
        manager.add_entry(new_entry).expect("Add entry");
    }

    (dir, Arc::new(Mutex::new(manager)))
}

pub fn bench_concurrency(c: &mut Criterion) {
    let mut group = c.benchmark_group("Mutex Contention & Concurrency");
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_millis(500));
    group.sample_size(10);

    for num_threads in [2, 4, 8, 16].iter() {
        let (_dir, vault_arc) = setup_concurrent_vault(500);

        group.bench_with_input(
            BenchmarkId::new("Concurrent Search Queries", format!("{} threads", num_threads)),
            num_threads,
            |b, &threads_count| {
                b.iter(|| {
                    let mut handles = Vec::with_capacity(threads_count);
                    for _ in 0..threads_count {
                        let arc_clone = Arc::clone(&vault_arc);
                        handles.push(thread::spawn(move || {
                            let mgr = arc_clone.lock().unwrap();
                            let _results = mgr.search_entries(black_box("concurrent")).unwrap();
                        }));
                    }
                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}
