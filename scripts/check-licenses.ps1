$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

& (Join-Path $PSScriptRoot "check-forbidden-dependencies.ps1")

$cargoDeny = Get-Command cargo-deny -ErrorAction SilentlyContinue
if ($cargoDeny) {
  cargo deny --manifest-path (Join-Path $root "src-tauri/Cargo.toml") check licenses
} else {
  Write-Host "SKIPPED: cargo-deny is not installed. Optional: cargo install cargo-deny"
}

$cargoAbout = Get-Command cargo-about -ErrorAction SilentlyContinue
if (-not $cargoAbout) {
  Write-Host "SKIPPED: cargo-about is not installed. Optional: cargo install cargo-about"
}

Write-Host "License guardrail checks completed with unavailable optional tools marked as skipped."
