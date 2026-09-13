# Yntra Vault Reproducible Builds Specification & Audit Guide

This document defines the architecture, environment normalization rules, and verification procedures for achieving reproducible builds in Yntra Vault.

---

## 1. Overview & Security Objective

Yntra Vault is an offline-first high-security password manager. Users, auditors, and security researchers must be able to independently verify that distributed binary packages match the exact source code published in the official repository byte-for-byte.

A build is reproducible when executing the compilation process on identical source code produces identical SHA-256 binary outputs, regardless of:
- Machine build path (e.g. `C:\Users\...` vs `/home/...`)
- Host environment variables, locale, or timezone
- Host build execution timestamp
- Host CPU micro-architecture differences (via locked `target-cpu`)

---

## 2. Determinism Guarantees & Technical Invariants

### 2.1 Pinned Toolchain Matrix
- **Rust Compiler**: `1.95.0` (pinned via `rust-toolchain.toml`)
- **Package Manager**: `Bun 1.3.13` (pinned via `package.json` & lockfile)
- **Rust Edition**: `2024`

### 2.2 Compiler Path Remapping & PE Linker Parity (`.cargo/config.toml`)
To prevent absolute developer filesystem paths from embedding in panics, debug paths, symbol tables, and binary string constants, Cargo uses path remapping, micro-architecture pinning, and Windows PE linker reproducibility flags:
```toml
[build]
rustflags = [
    "--remap-path-prefix=./=/yntra-vault/",
    "-C", "codegen-units=1",
    "-C", "target-cpu=x86-64"
]

[target.x86_64-pc-windows-msvc]
rustflags = [
    "--remap-path-prefix=./=/yntra-vault/",
    "-C", "codegen-units=1",
    "-C", "target-cpu=x86-64",
    "-C", "link-arg=/TIMESTAMP:1700000000",
    "-C", "link-arg=/BREPRO",
    "-C", "link-arg=/PDBALTPATH:yntra-vault-app.pdb"
]
```

### 2.3 Release Profile Optimization (`Cargo.toml`)
Release binaries enforce single-unit Link Time Optimization (LTO) to eliminate non-determinism introduced by multi-threaded code generation partitions:
```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

### 2.4 Timestamp & Environment Normalization
All builds normalize timestamp attributes using the `SOURCE_DATE_EPOCH` standard (IEEE Std 1003.1):
- `SOURCE_DATE_EPOCH`: Derived from the Git commit timestamp (`git log -1 --format=%ct`), or fixed fallback `1700000000`.
- `ZERO_AR_DATE=1`: Forces deterministic timestamping in static library archives (`.a` / `.rlib`).
- `TZ=UTC`: Normalizes host timezone.
- `LC_ALL=C.UTF-8`: Normalizes locale formatting across platforms.
- `.gitattributes`: Enforces `eol=lf` line ending normalization across Windows (CRLF) and Linux (LF) checkouts.

---

## 3. Independent Verification Instructions

### 3.1 PowerShell (Windows)
Run the automated verification suite which executes two clean, isolated builds into separate output paths and verifies matching SHA-256 checksums:

```powershell
powershell -ExecutionPolicy Bypass -File ./scripts/verify-reproducible.ps1
```

To run a single reproducible build and generate `reproducible-manifest.json`:

```powershell
powershell -ExecutionPolicy Bypass -File ./scripts/build-reproducible.ps1 -Clean
```

### 3.2 Bash (Linux / macOS)
Execute the verification script:

```bash
chmod +x ./scripts/*.sh
./scripts/verify-reproducible.sh
```

---

## 4. Containerized Hermetic Builds (Docker) & CI Integration

### 4.1 Local Containerized Build
To verify builds within a hermetic Debian container with fully isolated system dependencies:

```bash
docker build -t yntra-vault-reproducible -f Dockerfile.reproducible .
docker run --rm yntra-vault-reproducible
```

### 4.2 GitHub Actions Continuous Integration
Automated reproducible build verification is configured in [`.github/workflows/reproducible-builds.yml`](file:///c:/Users/hellich/Desktop/yntra-vault-private/.github/workflows/reproducible-builds.yml). Every commit, pull request, and release tag triggers automated Linux containerized and Windows native dual-build attestations.

---

## 5. Software Bill of Materials (SBOM) & Attestation Manifest

Every reproducible build generates `reproducible-manifest.json` containing:
1. `schema_version`: Schema version of the attestation manifest.
2. `source_date_epoch`: Timestamp applied during build execution.
3. `rust_version`: Exact `rustc` compiler version output.
4. `bun_version`: Exact `bun` runtime version output.
5. `artifacts`: SHA-256 digest map of compiled binaries and frontend bundle hash.
