# Yntra Vault - Reproducible Build Script (PowerShell)
param (
    [string]$OutDir = "reproducible-dist",
    [switch]$Clean
)

$ErrorActionPreference = "Stop"

Write-Host "================================================" -ForegroundColor Cyan
Write-Host " Yntra Vault: SOTA Reproducible Build Pipeline  " -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan

# 1. Determine SOURCE_DATE_EPOCH from Git commit timestamp or default fixed epoch
$GitEpoch = git log -1 --format=%ct 2>$null
if ($GitEpoch -and $GitEpoch -match "^\d+$") {
    $env:SOURCE_DATE_EPOCH = $GitEpoch
    Write-Host "Using Git commit timestamp for SOURCE_DATE_EPOCH: $GitEpoch" -ForegroundColor Green
} else {
    $env:SOURCE_DATE_EPOCH = "1700000000"
    Write-Host "Using fallback fixed epoch for SOURCE_DATE_EPOCH: 1700000000" -ForegroundColor Yellow
}

# 2. Normalize Build Environment
$env:ZERO_AR_DATE = "1"
$env:TZ = "UTC"
$env:LC_ALL = "C.UTF-8"
$env:LANG = "C.UTF-8"
$env:CARGO_INCREMENTAL = "0"
$env:RUSTFLAGS = "--remap-path-prefix=$PSScriptRoot\..=/yntra-vault/ -C codegen-units=1 -C target-cpu=x86-64 -C link-arg=/TIMESTAMP:$env:SOURCE_DATE_EPOCH -C link-arg=/BREPRO -C link-arg=/PDBALTPATH:yntra-vault-app.pdb"

# 3. Optional Clean Step
if ($Clean) {
    Write-Host "Performing clean build..." -ForegroundColor Yellow
    if (Test-Path "dist") { Remove-Item -Recurse -Force "dist" }
    if (Test-Path "src-core/target/release") { Remove-Item -Recurse -Force "src-core/target/release" }
    if (Test-Path "src-tauri/target/release") { Remove-Item -Recurse -Force "src-tauri/target/release" }
}

# 4. Build Frontend Assets
Write-Host "`n[1/3] Building frontend bundle with Bun..." -ForegroundColor Cyan
bun install --frozen-lockfile
bun run build

# 5. Build Core Cryptography Engine
Write-Host "`n[2/3] Building Rust Core Engine (release)..." -ForegroundColor Cyan
cargo build --manifest-path src-core/Cargo.toml --release --frozen

# 6. Build Desktop App Binary
Write-Host "`n[3/3] Building Tauri Desktop Binary (release)..." -ForegroundColor Cyan
cargo build --manifest-path src-tauri/Cargo.toml --release --frozen

# 7. Collect & Hash Artifacts
$TargetDir = Join-Path -Path $PSScriptRoot -ChildPath "..\$OutDir"
if (-not (Test-Path $TargetDir)) { New-Item -ItemType Directory -Force -Path $TargetDir | Out-Null }

Write-Host "`nComputing SHA-256 checksums and generating reproducible manifest..." -ForegroundColor Cyan

$Artifacts = @()

# Find output binaries in src-core and src-tauri release targets
$CoreRelease = "src-core/target/release"
$TauriRelease = "src-tauri/target/release"

$CandidateFiles = @(
    "$CoreRelease/yntra_vault_core.dll",
    "$CoreRelease/libyntra_vault_core.rlib",
    "$CoreRelease/yntra_vault_core.lib",
    "$CoreRelease/libyntra_vault_core.a",
    "$TauriRelease/yntra-vault-app.exe",
    "$TauriRelease/yntra-vault-app"
)

$FoundArtifacts = @{}

foreach ($file in $CandidateFiles) {
    if (Test-Path $file) {
        $hash = (Get-FileHash -Path $file -Algorithm SHA256).Hash.ToLower()
        $name = Split-Path -Leaf $file
        $size = (Get-Item $file).Length
        $FoundArtifacts[$name] = @{
            path = $file
            sha256 = $hash
            size_bytes = $size
        }
        Write-Host "  Artifact: $name ($size bytes)" -ForegroundColor Gray
        Write-Host "  SHA256:   $hash" -ForegroundColor Green
    }
}

# Hash dist/ assets summary
if (Test-Path "dist") {
    $distFiles = Get-ChildItem -Path "dist" -Recurse -File
    $distHashes = @()
    foreach ($df in $distFiles) {
        $h = (Get-FileHash -Path $df.FullName -Algorithm SHA256).Hash.ToLower()
        $relPath = $df.FullName.Replace((Get-Location).Path, "").TrimStart("\", "/")
        $distHashes += "$h  $relPath"
    }
    $combinedDistString = ($distHashes | Sort-Object) -join "`n"
    $distBundleHash = [System.BitConverter]::ToString(([System.Security.Cryptography.SHA256]::Create()).ComputeHash([System.Text.Encoding]::UTF8.GetBytes($combinedDistString))).Replace("-","").ToLower()
    
    $FoundArtifacts["frontend_dist_bundle"] = @{
        sha256 = $distBundleHash
        file_count = $distFiles.Count
    }
    Write-Host "  Frontend Dist Bundle Hash: $distBundleHash ($($distFiles.Count) files)" -ForegroundColor Green
}

# Build Manifest JSON
$Manifest = @{
    schema_version = "1.0.0"
    project = "Yntra Vault"
    source_date_epoch = $env:SOURCE_DATE_EPOCH
    rust_version = (rustc --version)
    bun_version = (bun --version)
    build_time_utc = (Get-Date).ToUniversalTime().ToString("o")
    artifacts = $FoundArtifacts
}

$ManifestJson = $Manifest | ConvertTo-Json -Depth 5
Set-Content -Path "$TargetDir/reproducible-manifest.json" -Value $ManifestJson -Encoding UTF8

Write-Host "`nReproducible build completed successfully!" -ForegroundColor Green
Write-Host "Manifest saved to: $OutDir/reproducible-manifest.json" -ForegroundColor Yellow
