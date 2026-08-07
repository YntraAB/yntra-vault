//! Yntra Vault Core Benchmark Suite Entrypoint
//!
//! Consolidates 12 modular benchmark suites under 1 single release binary target.
//! Compiles release dependencies ONCE in seconds, preserving instant developer iterations.

mod bench_kdf;
mod bench_cipher;
mod bench_vault_serialization;
mod bench_vault_search;
mod bench_generator;
mod bench_totp;
mod bench_breach;
mod bench_io_atomic_save;
mod bench_search_engine;
mod bench_passkey;
mod bench_concurrency;
mod bench_memory_zeroize;

use criterion::{criterion_group, criterion_main};

criterion_group!(
    benches,
    bench_kdf::bench_kdf,
    bench_cipher::bench_cipher,
    bench_vault_serialization::bench_vault_serialization,
    bench_vault_search::bench_vault_search,
    bench_generator::bench_generator,
    bench_totp::bench_totp,
    bench_breach::bench_breach_and_strength,
    bench_io_atomic_save::bench_io_atomic_save,
    bench_search_engine::bench_search_engine,
    bench_passkey::bench_passkey,
    bench_concurrency::bench_concurrency,
    bench_memory_zeroize::bench_memory_zeroize
);
criterion_main!(benches);
