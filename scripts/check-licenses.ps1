$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

& (Join-Path $PSScriptRoot "check-forbidden-dependencies.ps1")

$noticePath = Join-Path $root "THIRD_PARTY_LICENSES.md"
if (-not (Test-Path -LiteralPath $noticePath)) {
  throw "THIRD_PARTY_LICENSES.md is missing. Run npm run license:generate first."
}

$unknownNotices = @(
  Get-Content -LiteralPath $noticePath |
    Where-Object { $_ -match '^- .+: (?:UNKNOWN|NOASSERTION)(?:;|$)' }
)
if ($unknownNotices.Count -gt 0) {
  throw ("Unknown or NOASSERTION dependency licenses found in THIRD_PARTY_LICENSES.md:`n{0}" -f ($unknownNotices -join "`n"))
}

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
