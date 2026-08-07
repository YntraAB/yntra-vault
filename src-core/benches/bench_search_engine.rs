//! Full Search Engine & Index Rebuilding Benchmarks

use criterion::{black_box, BenchmarkId, Criterion};
use std::time::Duration;
use tempfile::tempdir;
use yntra_vault_core::vault::manager::{NewEntry, VaultManager};
use yntra_vault_core::vault::types::EntryType;

fn setup_benchmark_vault(count: usize) -> (tempfile::TempDir, VaultManager) {
    let dir = tempdir().expect("Failed to create temp dir");
    let file_path = dir.path().join("bench_search_vault.vdb");
    let password = "TestMasterPassword123!";

    let mut manager = VaultManager::create("Benchmark Vault", password, &file_path)
        .expect("Failed to create test vault");

    for i in 0..count {
        let new_entry = NewEntry {
            title: format!("AWS Production Cluster Node {}", i),
            username: format!("deploy_user_{}@devops.aws.com", i),
            password: format!("P@ssw0rd_Cluster_Node_{}!", i),
            url: format!("https://k8s-node-{}.internal.aws.com:6443", i),
            email: format!("deploy_user_{}@devops.aws.com", i),
            notes: format!("Production Kubernetes worker node {} setup instructions", i),
            tags: vec!["aws".to_string(), "k8s".to_string(), "production".to_string()],
            totp_secret: None,
            custom_fields: Vec::new(),
            entry_type: Some(EntryType::Login),
            generate_passkey: None,
        };
        manager.add_entry(new_entry).expect("Add entry");
    }

    (dir, manager)
}

pub fn bench_search_engine(c: &mut Criterion) {
    let mut group = c.benchmark_group("Search Engine & Indexing");
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_millis(500));
    group.sample_size(10);

    for count in [10, 100, 1000].iter() {
        let (_dir, mut manager) = setup_benchmark_vault(*count);

        group.bench_with_input(
            BenchmarkId::new("Full Trigram Search Index Rebuild", format!("{} entries", count)),
            &count,
            |b, _| {
                b.iter(|| {
                    manager.rebuild_search_index();
                });
            },
        );

        let queries = ["k8s", "aws", "deploy_user_5", "nonexistent_query_string"];
        for q in queries.iter() {
            group.bench_with_input(
                BenchmarkId::new(format!("Search Query '{}'", q), format!("{} entries", count)),
                q,
                |b, query| {
                    b.iter(|| {
                        manager.search_entries(black_box(query)).unwrap();
                    });
                },
            );
        }
    }

    group.finish();
}
