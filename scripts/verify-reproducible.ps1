# Yntra Vault - Reproducible Build Verification Script (PowerShell)
$ErrorActionPreference = "Stop"

Write-Host "=======================================================" -ForegroundColor Cyan
Write-Host " Yntra Vault: Reproducible Build Verification Suite     " -ForegroundColor Cyan
Write-Host "=======================================================" -ForegroundColor Cyan

$Dir1 = "reproducible-build-1"
$Dir2 = "reproducible-build-2"

try {
    # 1. Execute Build #1
    Write-Host "`n>>> Executing Isolated Build #1..." -ForegroundColor Yellow
    & "$PSScriptRoot/build-reproducible.ps1" -OutDir $Dir1 -Clean
    if (-not (Test-Path "$Dir1/reproducible-manifest.json")) {
        throw "Build #1 failed to generate manifest!"
    }
    $Manifest1 = Get-Content "$Dir1/reproducible-manifest.json" | ConvertFrom-Json

    # 2. Execute Build #2
    Write-Host "`n>>> Executing Isolated Build #2..." -ForegroundColor Yellow
    & "$PSScriptRoot/build-reproducible.ps1" -OutDir $Dir2 -Clean
    if (-not (Test-Path "$Dir2/reproducible-manifest.json")) {
        throw "Build #2 failed to generate manifest!"
    }
    $Manifest2 = Get-Content "$Dir2/reproducible-manifest.json" | ConvertFrom-Json

    # 3. Compare Checksums
    Write-Host "`n=======================================================" -ForegroundColor Cyan
    Write-Host " Verification Summary: Build #1 vs Build #2             " -ForegroundColor Cyan
    Write-Host "=======================================================" -ForegroundColor Cyan

    $MatchCount = 0
    $MismatchCount = 0

    $Keys1 = $Manifest1.artifacts.psobject.properties.Name

    foreach ($key in $Keys1) {
        $hash1 = $Manifest1.artifacts.$key.sha256
        $hash2 = $Manifest2.artifacts.$key.sha256

        Write-Host "`nArtifact: $key" -ForegroundColor Gray
        Write-Host "  Build #1 SHA-256: $hash1" -ForegroundColor Gray
        Write-Host "  Build #2 SHA-256: $hash2" -ForegroundColor Gray

        if ($hash1 -eq $hash2) {
            Write-Host "  RESULT: MATCH (100% Deterministic)" -ForegroundColor Green
            $MatchCount++
        } else {
            Write-Host "  RESULT: MISMATCH (Non-deterministic output detected)" -ForegroundColor Red
            $MismatchCount++
        }
    }

    Write-Host "`n-------------------------------------------------------" -ForegroundColor Gray
    if ($MismatchCount -eq 0 -and $MatchCount -gt 0) {
        Write-Host "VERIFICATION PASSED: All $MatchCount artifacts are byte-for-byte reproducible!" -ForegroundColor Green
    } else {
        Write-Error "VERIFICATION FAILED: $MismatchCount artifact(s) differed between independent builds."
        exit 1
    }
}
finally {
    # Cleanup temporary verification folders
    if (Test-Path $Dir1) { Remove-Item -Recurse -Force $Dir1 }
    if (Test-Path $Dir2) { Remove-Item -Recurse -Force $Dir2 }
}
