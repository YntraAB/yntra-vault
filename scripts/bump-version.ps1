param(
    [Parameter(Mandatory=$true, Position=0)]
    [string]$Version
)

$ErrorActionPreference = "Stop"
$scriptPath = Join-Path $PSScriptRoot "bump-version.mjs"

if (Get-Command bun -ErrorAction SilentlyContinue) {
    bun run $scriptPath $Version
} elseif (Get-Command node -ErrorAction SilentlyContinue) {
    node $scriptPath $Version
} else {
    throw "Neither bun nor node was found on PATH."
}
