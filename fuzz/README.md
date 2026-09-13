# Yntra Vault Continuous Fuzzing Harnesses

This directory contains coverage-guided continuous fuzzing targets for Yntra Vault, powered by [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz) and LLVM's `libFuzzer`.

## Fuzz Targets Overview

| Target | Description | Primary Attack Vectors Tested |
|---|---|---|
| `fuzz_vdb_header` | Binary `.vdb` deserialization (`VaultFile::from_bytes`) | Allocation bombs, integer overflows in length prefixes, malformed KDF params, corrupted biometric / hardware 2FA blocks |
| `fuzz_import_bitwarden_json` | Bitwarden JSON importer | Deeply nested JSON ASTs, unexpected type conversions, alternative URI spoofing, custom field corruption |
| `fuzz_import_keepass_xml` | KeePass XML importer | Unclosed XML tags, malformed group structures, XML entity bombs, string index boundary bugs |
| `fuzz_import_csv` | RFC 4180 CSV matrix & multi-vendor parsers | Unmatched quotes, mixed CRLF/LF sequences, embedded null bytes, multi-column overflow |
| `fuzz_import_autodetect` | End-to-end import parser with format sniffing | Ambiguous file signatures, parser confusion attacks, fallback recovery stability |
| `fuzz_attachments` | Per-entry attachment AEAD decryption | Corrupted nonces, tampered Poly1305 tags, truncated blobs, AAD mismatch detection |
| `fuzz_totp_uri` | OTPAuth URI parser & TOTP engine | Zero period division, digits overflow, base32 decoding errors, excessive URI lengths |

---

## Prerequisites

Continuous fuzzing with `libFuzzer` requires the Rust nightly toolchain and `cargo-fuzz`:

```bash
rustup toolchain install nightly
cargo install cargo-fuzz
```

On Windows, `cargo-fuzz` runs under LLVM Clang or WSL2/Linux.

---

## Running Fuzz Targets

To run a specific target (e.g. `fuzz_vdb_header`):

```bash
cargo +nightly fuzz run fuzz_vdb_header
```

### Running with Sanitizers (AddressSanitizer / UndefinedBehaviorSanitizer)

```bash
cargo +nightly fuzz run --sanitizer=address fuzz_vdb_header
```

### Running with Parallel Jobs and Max Run Time

```bash
# Run 4 parallel workers for 10 minutes (600 seconds)
cargo +nightly fuzz run -j 4 fuzz_import_bitwarden_json -- -max_total_time=600
```

---

## Corpus Management

Initial seed corpora are located in `fuzz/corpus/<target_name>/`. `cargo-fuzz` will automatically store interesting inputs discovered during fuzzing under `fuzz/corpus/<target_name>/`.

To minimize the corpus after a long run:

```bash
cargo +nightly fuzz cmin fuzz_vdb_header
```

---

## Offline Cross-Platform Smoke Testing

For developers without nightly LLVM or CI environments running on Windows MSVC, all parser targets are additionally covered by the standalone mutation smoke test suite:

```bash
cargo test --test fuzz_smoke_tests -p yntra-vault-core
```
