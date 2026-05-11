$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$forbidden = @(
  "gifski",
  "gifsicle",
  "libimagequant",
  "imagequant",
  "pngquant",
  "ffmpeg"
)

$targetFiles = @(
  "Cargo.toml",
  "Cargo.lock",
  "src-tauri/Cargo.toml",
  "src-tauri/Cargo.lock",
  "package.json",
  "package-lock.json",
  "pnpm-lock.yaml"
)

$existingTargets = foreach ($relativePath in $targetFiles) {
  $path = Join-Path $root $relativePath
  if (Test-Path -LiteralPath $path) {
    Get-Item -LiteralPath $path
  }
}

$sourceRoots = @("src", "src-tauri/src") | ForEach-Object {
  $path = Join-Path $root $_
  if (Test-Path -LiteralPath $path) {
    $path
  }
}

$sourceFiles = foreach ($sourceRoot in $sourceRoots) {
  Get-ChildItem -LiteralPath $sourceRoot -Recurse -File -Include *.rs,*.ts,*.tsx,*.js,*.mjs,*.json,*.toml,*.ps1,*.md
}

$matches = @()
foreach ($file in @($existingTargets) + @($sourceFiles)) {
  $fullPath = $file.FullName
  $relative = if ($fullPath.StartsWith($root, [System.StringComparison]::OrdinalIgnoreCase)) {
    $fullPath.Substring($root.Length).TrimStart('\', '/')
  } else {
    $fullPath
  }
  $content = Get-Content -LiteralPath $file.FullName -Raw -ErrorAction SilentlyContinue
  foreach ($name in $forbidden) {
    if ($content -match "(?i)\b$([regex]::Escape($name))\b") {
      $matches += [pscustomobject]@{
        File = $relative
        ForbiddenName = $name
      }
    }
  }
}

if ($matches.Count -gt 0) {
  Write-Host "Forbidden optimizer dependency reference found:" -ForegroundColor Red
  $matches | Sort-Object File, ForbiddenName | Format-Table -AutoSize
  exit 1
}

Write-Host "No forbidden optimizer dependency names found."
