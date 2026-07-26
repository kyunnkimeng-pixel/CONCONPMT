param(
  [string]$BundleDir = "src-tauri\target\release\bundle",
  [ValidateSet("nsis", "all")]
  [string]$ArtifactSet = "nsis"
)

$ErrorActionPreference = "Stop"

$root = Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")
$bundleRoot = Join-Path $root $BundleDir
$tauriConfig = Get-Content -LiteralPath (Join-Path $root "src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json
$productName = [string]$tauriConfig.productName
$version = [string]$tauriConfig.version

if ([string]::IsNullOrWhiteSpace($productName) -or [string]::IsNullOrWhiteSpace($version)) {
  throw "Unable to read productName/version from src-tauri\tauri.conf.json"
}

$artifacts = switch ($ArtifactSet) {
  "all" {
    @(
      "msi\$productName`_$version`_x64_en-US.msi",
      "nsis\$productName`_$version`_x64-setup.exe"
    )
  }
  "nsis" {
    @("nsis\$productName`_$version`_x64-setup.exe")
  }
}

if (!(Test-Path -LiteralPath $bundleRoot)) {
  throw "Bundle directory does not exist: $bundleRoot"
}

$lines = foreach ($relativePath in $artifacts) {
  $fullPath = Join-Path $bundleRoot $relativePath
  if (!(Test-Path -LiteralPath $fullPath)) {
    throw "Missing release artifact: $fullPath"
  }

  $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $fullPath).Hash.ToLowerInvariant()
  "$hash  $(Split-Path -Leaf $relativePath)"
}

$outputPath = Join-Path $bundleRoot "SHA256SUMS.txt"
[System.IO.File]::WriteAllLines(
  $outputPath,
  [string[]]$lines,
  [System.Text.UTF8Encoding]::new($false)
)
Write-Host "Wrote $outputPath"
