#!/usr/bin/env bash
# Yntra Vault - Reproducible Build Script (Bash)
set -euo pipefail

echo "================================================"
echo " Yntra Vault: SOTA Reproducible Build Pipeline  "
echo "================================================"

# 1. Determine SOURCE_DATE_EPOCH
if [ -z "${SOURCE_DATE_EPOCH:-}" ]; then
    if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
        export SOURCE_DATE_EPOCH="$(git log -1 --format=%ct)"
        echo "Using Git commit timestamp for SOURCE_DATE_EPOCH: ${SOURCE_DATE_EPOCH}"
    else
        export SOURCE_DATE_EPOCH="1700000000"
        echo "Using fallback fixed epoch for SOURCE_DATE_EPOCH: 1700000000"
    fi
fi

# 2. Normalize Build Environment
export ZERO_AR_DATE=1
export TZ=UTC
export LC_ALL=C.UTF-8
export LANG=C.UTF-8
export CARGO_INCREMENTAL=0
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
export RUSTFLAGS="--remap-path-prefix=${ROOT_DIR}=/yntra-vault/ -C codegen-units=1 -C target-cpu=x86-64"

OUT_DIR="${1:-reproducible-dist}"
mkdir -p "${ROOT_DIR}/${OUT_DIR}"

# 3. Build Frontend
echo -e "\n[1/3] Building frontend bundle with Bun..."
bun install --frozen-lockfile
bun run build

# 4. Build Core Engine
echo -e "\n[2/3] Building Rust Core Engine (release)..."
cargo build --manifest-path src-core/Cargo.toml --release --frozen

# 5. Build Desktop Binary
echo -e "\n[3/3] Building Tauri Desktop Binary (release)..."
cargo build --manifest-path src-tauri/Cargo.toml --release --frozen

# 6. Generate SHA-256 Checksums
echo -e "\nComputing SHA-256 checksums..."
MANIFEST_FILE="${ROOT_DIR}/${OUT_DIR}/reproducible-manifest.json"

SHA_CORE=""
if [ -f "src-core/target/release/libyntra_vault_core.rlib" ]; then
    SHA_CORE="$(sha256sum src-core/target/release/libyntra_vault_core.rlib | awk '{print $1}')"
elif [ -f "src-core/target/release/yntra_vault_core.dll" ]; then
    SHA_CORE="$(sha256sum src-core/target/release/yntra_vault_core.dll | awk '{print $1}')"
fi

SHA_TAURI=""
if [ -f "src-tauri/target/release/yntra-vault-app" ]; then
    SHA_TAURI="$(sha256sum src-tauri/target/release/yntra-vault-app | awk '{print $1}')"
elif [ -f "src-tauri/target/release/yntra-vault-app.exe" ]; then
    SHA_TAURI="$(sha256sum src-tauri/target/release/yntra-vault-app.exe | awk '{print $1}')"
fi

cat <<EOF > "${MANIFEST_FILE}"
{
  "schema_version": "1.0.0",
  "project": "Yntra Vault",
  "source_date_epoch": "${SOURCE_DATE_EPOCH}",
  "rust_version": "$(rustc --version)",
  "bun_version": "$(bun --version)",
  "sha256_core": "${SHA_CORE}",
  "sha256_tauri": "${SHA_TAURI}"
}
EOF

echo "Reproducible build complete. Manifest written to ${OUT_DIR}/reproducible-manifest.json"
