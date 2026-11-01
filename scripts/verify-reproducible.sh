#!/usr/bin/env bash
# Yntra Vault - Reproducible Build Verification Script (Bash)
set -euo pipefail

echo "======================================================="
echo " Yntra Vault: Reproducible Build Verification Suite     "
echo "======================================================="

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${SCRIPT_DIR}/.."
DIR1="reproducible-build-1"
DIR2="reproducible-build-2"

cleanup() {
    rm -rf "${SCRIPT_DIR}/../${DIR1}" "${SCRIPT_DIR}/../${DIR2}" || true
}
trap cleanup EXIT

echo -e "\n>>> Executing Isolated Build #1..."
"${SCRIPT_DIR}/build-reproducible.sh" "${DIR1}" --clean

echo -e "\n>>> Executing Isolated Build #2..."
"${SCRIPT_DIR}/build-reproducible.sh" "${DIR2}" --clean

MANIFEST1="${SCRIPT_DIR}/../${DIR1}/reproducible-manifest.json"
MANIFEST2="${SCRIPT_DIR}/../${DIR2}/reproducible-manifest.json"

if [ ! -f "${MANIFEST1}" ] || [ ! -f "${MANIFEST2}" ]; then
    echo "ERROR: Manifest files missing!"
    exit 1
fi

SHA1_CORE="$(grep -o '"sha256_core": "[^"]*"' "${MANIFEST1}" | cut -d'"' -f4)"
SHA2_CORE="$(grep -o '"sha256_core": "[^"]*"' "${MANIFEST2}" | cut -d'"' -f4)"

SHA1_TAURI="$(grep -o '"sha256_tauri": "[^"]*"' "${MANIFEST1}" | cut -d'"' -f4)"
SHA2_TAURI="$(grep -o '"sha256_tauri": "[^"]*"' "${MANIFEST2}" | cut -d'"' -f4)"

echo -e "\n======================================================="
echo " Verification Summary: Build #1 vs Build #2"
echo "======================================================="
echo "Core Engine Build #1: ${SHA1_CORE}"
echo "Core Engine Build #2: ${SHA2_CORE}"
echo "Tauri App Build #1:   ${SHA1_TAURI}"
echo "Tauri App Build #2:   ${SHA2_TAURI}"

for artifact in core tauri cli frontend; do
    first="$(grep -o "\"sha256_${artifact}\": \"[^\"]*\"" "$MANIFEST1" | cut -d'"' -f4)"
    second="$(grep -o "\"sha256_${artifact}\": \"[^\"]*\"" "$MANIFEST2" | cut -d'"' -f4)"
    if [[ ! "$first" =~ ^[a-f0-9]{64}$ ]] || [ "$first" != "$second" ]; then
        echo "VERIFICATION FAILED: Missing or different ${artifact} checksum" >&2
        exit 1
    fi
done
if [ "${SHA1_CORE}" = "${SHA2_CORE}" ] && [ "${SHA1_TAURI}" = "${SHA2_TAURI}" ]; then
    mkdir -p reproducible-dist
    cp "$MANIFEST2" reproducible-dist/reproducible-manifest.json
    echo -e "\nVERIFICATION PASSED: Independent builds match SHA-256 byte-for-byte!"
    exit 0
else
    echo -e "\nVERIFICATION FAILED: Binary outputs differed!"
    exit 1
fi
