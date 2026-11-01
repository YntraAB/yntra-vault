# Yntra Vault - Reproducible Build Script (PowerShell)
param (
    [string]$OutDir = "reproducible-dist",
    [switch]$Clean
)

$ErrorActionPreference = "Stop"
$BuildRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Set-Location -LiteralPath $BuildRoot
function Invoke-BuildTool([string]$Tool, [string[]]$ToolArguments) {
    & $Tool @ToolArguments
    if ($LASTEXITCODE -ne 0) { throw "$Tool failed (exit $LASTEXITCODE)" }
}

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
    foreach ($relative in @('dist','target/release','crates/core/target/release','src-tauri/target/release')) {
        $target = [IO.Path]::GetFullPath((Join-Path $BuildRoot $relative))
        if (-not $target.StartsWith($BuildRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Cleanup escaped build root' }
        if (Test-Path -LiteralPath $target) {
            $entry = Get-Item -LiteralPath $target -Force
            while ($entry.FullName -ne $BuildRoot) {
                if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Refusing cleanup through a reparse point' }
                $entry = $entry.Parent
            }
            Remove-Item -LiteralPath $target -Recurse -Force
        }
    }
}

# 4. Build Frontend Assets
Write-Host "`n[1/3] Building frontend bundle with Bun..." -ForegroundColor Cyan
Invoke-BuildTool 'bun' @('install','--frozen-lockfile')
Invoke-BuildTool 'bun' @('run','build')

# 5. Build Core Cryptography Engine
Write-Host "`n[2/3] Building Rust Core Engine (release)..." -ForegroundColor Cyan
Invoke-BuildTool 'cargo' @('fetch','--locked')
Invoke-BuildTool 'cargo' @('build','--manifest-path','crates/core/Cargo.toml','--release','--frozen')

# 6. Build Desktop App Binary
Write-Host "`n[3/3] Building Tauri Desktop Binary (release)..." -ForegroundColor Cyan
Invoke-BuildTool 'cargo' @('build','--workspace','--release','--frozen')

# 7. Collect & Hash Artifacts
$TargetDir = Join-Path -Path $PSScriptRoot -ChildPath "..\$OutDir"
if (-not (Test-Path $TargetDir)) { New-Item -ItemType Directory -Force -Path $TargetDir | Out-Null }

Write-Host "`nComputing SHA-256 checksums and generating reproducible manifest..." -ForegroundColor Cyan

$Artifacts = @()
foreach ($required in @('target/release/libyntra_vault_core.rlib','target/release/yntra-vault-app.exe','target/release/yntra.exe','dist/index.html')) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) { throw "Missing build output: $required" }
}

# Find output binaries in workspace target/release or fallback crate release targets
$CandidateFiles = @(
    "target/release/yntra.exe",
    "target/release/yntra_vault_core.dll",
    "target/release/libyntra_vault_core.rlib",
    "target/release/yntra_vault_core.lib",
    "target/release/libyntra_vault_core.a",
    "target/release/yntra-vault-app.exe",
    "target/release/yntra-vault-app",
    "crates/core/target/release/yntra_vault_core.dll",
    "crates/core/target/release/libyntra_vault_core.rlib",
    "crates/core/target/release/yntra_vault_core.lib",
    "crates/core/target/release/libyntra_vault_core.a",
    "src-tauri/target/release/yntra-vault-app.exe",
    "src-tauri/target/release/yntra-vault-app"
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
