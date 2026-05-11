$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$outputPath = Join-Path $root "THIRD_PARTY_LICENSES.md"
$generatedAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")

$packageJsonPath = Join-Path $root "package.json"
$workspaceCargoTomlPath = Join-Path $root "src-tauri/Cargo.toml"
$cargoLockPath = Join-Path $root "src-tauri/Cargo.lock"
$cargoRegistrySrc = Join-Path $env:USERPROFILE ".cargo/registry/src"

function Add-Line([System.Collections.Generic.List[string]] $lines, [string] $value = "") {
  $lines.Add($value) | Out-Null
}

function Get-NpmPackageJsonPath([string] $name) {
  if ($name.StartsWith("@")) {
    $parts = $name.Split("/")
    if ($parts.Length -eq 2) {
      return Join-Path $root ("node_modules/{0}/{1}/package.json" -f $parts[0], $parts[1])
    }
  }

  return Join-Path $root ("node_modules/{0}/package.json" -f $name)
}

function Format-LicenseValue($value) {
  if ($null -eq $value) {
    return "UNKNOWN"
  }
  if ($value -is [string]) {
    if ($value.Trim().Length -eq 0) {
      return "UNKNOWN"
    }
    return $value
  }
  return ($value | ConvertTo-Json -Compress -Depth 8)
}

function Get-NpmPackageInfo([string] $name, [string] $declaredVersion, [string] $scope) {
  $path = Get-NpmPackageJsonPath $name
  if (Test-Path -LiteralPath $path) {
    $pkg = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
    return [pscustomobject]@{
      Name = $name
      DeclaredVersion = $declaredVersion
      InstalledVersion = if ($pkg.version) { $pkg.version } else { "unknown" }
      Scope = $scope
      License = Format-LicenseValue $pkg.license
      Notice = "from node_modules package.json"
    }
  }

  return [pscustomobject]@{
    Name = $name
    DeclaredVersion = $declaredVersion
    InstalledVersion = "not installed"
    Scope = $scope
    License = "UNKNOWN"
    Notice = "package.json not found under node_modules"
  }
}

function Get-CargoPackages([string] $lockPath) {
  $packages = New-Object System.Collections.Generic.List[object]
  $currentName = $null
  $currentVersion = $null
  $inPackage = $false

  foreach ($line in Get-Content -LiteralPath $lockPath) {
    if ($line -eq "[[package]]") {
      if ($currentName -and $currentVersion) {
        $packages.Add([pscustomobject]@{ Name = $currentName; Version = $currentVersion }) | Out-Null
      }
      $currentName = $null
      $currentVersion = $null
      $inPackage = $true
      continue
    }

    if (-not $inPackage) {
      continue
    }

    if ($line -match '^name = "(.+)"$') {
      $currentName = $Matches[1]
    } elseif ($line -match '^version = "(.+)"$') {
      $currentVersion = $Matches[1]
    }
  }

  if ($currentName -and $currentVersion) {
    $packages.Add([pscustomobject]@{ Name = $currentName; Version = $currentVersion }) | Out-Null
  }

  return $packages | Sort-Object Name, Version -Unique
}

function Get-CargoPackagesForNotice([string] $lockPath) {
  $cargo = Get-Command cargo -ErrorAction SilentlyContinue
  if ($cargo -and (Test-Path -LiteralPath $workspaceCargoTomlPath)) {
    try {
      $metadataJson = & cargo metadata --manifest-path $workspaceCargoTomlPath --format-version 1 --locked 2>$null
      if ($LASTEXITCODE -eq 0 -and $metadataJson) {
        $metadata = $metadataJson | ConvertFrom-Json
        $packageById = @{}
        $nodeById = @{}

        foreach ($package in $metadata.packages) {
          $packageById[$package.id] = $package
        }

        foreach ($node in $metadata.resolve.nodes) {
          $nodeById[$node.id] = $node
        }

        $rootId = $metadata.resolve.root
        if (-not $rootId -and $metadata.workspace_members -and $metadata.workspace_members.Count -gt 0) {
          $rootId = $metadata.workspace_members[0]
        }

        if ($rootId) {
          $included = New-Object 'System.Collections.Generic.HashSet[string]'
          $queued = New-Object 'System.Collections.Generic.Queue[string]'
          $queued.Enqueue($rootId)

          while ($queued.Count -gt 0) {
            $id = $queued.Dequeue()
            if (-not $included.Add($id)) {
              continue
            }

            if (-not $nodeById.ContainsKey($id)) {
              continue
            }

            $node = $nodeById[$id]
            foreach ($dep in $node.deps) {
              $includeDep = $false
              foreach ($depKind in $dep.dep_kinds) {
                if (Test-CargoDepKindAppliesToWindowsNotice $depKind) {
                  $includeDep = $true
                }
              }

              if ($includeDep -and $dep.pkg) {
                $queued.Enqueue($dep.pkg)
              }
            }
          }

          $packages = New-Object System.Collections.Generic.List[object]
          foreach ($id in $included) {
            if ($packageById.ContainsKey($id)) {
              $package = $packageById[$id]
              $packages.Add([pscustomobject]@{ Name = $package.name; Version = $package.version }) | Out-Null
            }
          }

          if ($packages.Count -gt 0) {
            return $packages | Sort-Object Name, Version -Unique
          }
        }
      }
    } catch {
      Write-Host "WARN: cargo metadata failed; falling back to Cargo.lock scan."
    }
  }

  return Get-CargoPackages $lockPath
}

function Test-CargoDepKindAppliesToWindowsNotice($depKind) {
  if ($depKind.kind -eq "dev") {
    return $false
  }

  if (-not $depKind.target) {
    return $true
  }

  $target = [string]$depKind.target
  if ($target -match "fuzzing") {
    return $false
  }

  if ($target -match "windows") {
    return $true
  }

  if ($target -match "unix|linux|macos|ios|android|wasm|redox|dragonfly|freebsd|openbsd|netbsd|solaris|illumos") {
    return $false
  }

  return $true
}

function Find-CrateCargoToml([string] $name, [string] $version) {
  if (-not (Test-Path -LiteralPath $cargoRegistrySrc)) {
    return $null
  }

  $crateDir = "{0}-{1}" -f $name, $version
  foreach ($registry in Get-ChildItem -LiteralPath $cargoRegistrySrc -Directory -ErrorAction SilentlyContinue) {
    $candidate = Join-Path $registry.FullName (Join-Path $crateDir "Cargo.toml")
    if (Test-Path -LiteralPath $candidate) {
      return $candidate
    }
  }

  return $null
}

function Get-CrateLicense([string] $name, [string] $version) {
  if (Test-Path -LiteralPath $workspaceCargoTomlPath) {
    $inPackage = $false
    $localName = $null
    $localVersion = $null
    $localLicense = $null
    $localLicenseFile = $null

    foreach ($line in Get-Content -LiteralPath $workspaceCargoTomlPath) {
      if ($line -eq "[package]") {
        $inPackage = $true
        continue
      }

      if ($inPackage -and $line -match '^\[') {
        break
      }

      if (-not $inPackage) {
        continue
      }

      if ($line -match '^name = "(.+)"') {
        $localName = $Matches[1]
      } elseif ($line -match '^version = "(.+)"') {
        $localVersion = $Matches[1]
      } elseif ($line -match '^license = "(.+)"') {
        $localLicense = $Matches[1]
      } elseif ($line -match '^license-file = "(.+)"') {
        $localLicenseFile = $Matches[1]
      }
    }

    if ($localName -eq $name -and $localVersion -eq $version) {
      if ($localLicense) {
        return [pscustomobject]@{
          License = $localLicense
          Notice = "from workspace Cargo.toml"
        }
      }

      if ($localLicenseFile) {
        return [pscustomobject]@{
          License = "SEE-LICENSE-FILE: $localLicenseFile"
          Notice = "from workspace Cargo.toml"
        }
      }

      return [pscustomobject]@{
        License = "UNKNOWN"
        Notice = "workspace Cargo.toml has no license or license-file field"
      }
    }
  }

  $cargoToml = Find-CrateCargoToml $name $version
  if (-not $cargoToml) {
    return [pscustomobject]@{
      License = "UNKNOWN"
      Notice = "crate Cargo.toml not found in local cargo registry cache"
    }
  }

  $license = $null
  $licenseFile = $null
  foreach ($line in Get-Content -LiteralPath $cargoToml) {
    if ($line -match '^license = "(.+)"') {
      $license = $Matches[1]
    } elseif ($line -match '^license-file = "(.+)"') {
      $licenseFile = $Matches[1]
    }
  }

  if ($license) {
    return [pscustomobject]@{
      License = $license
      Notice = "from local cargo registry Cargo.toml"
    }
  }

  if ($licenseFile) {
    return [pscustomobject]@{
      License = "SEE-LICENSE-FILE: $licenseFile"
      Notice = "from local cargo registry Cargo.toml"
    }
  }

  return [pscustomobject]@{
    License = "UNKNOWN"
    Notice = "no license or license-file field found in local Cargo.toml"
  }
}

function Get-LicenseReviewNote([string] $name, [string] $version, [string] $license, [string] $source) {
  if ($license -eq "UNKNOWN" -or $license -match "NOASSERTION") {
    return "{0} {1} ({2}): license metadata is unavailable and needs manual review." -f $name, $version, $source
  }

  if ($license -match "SEE-LICENSE-FILE") {
    return "{0} {1} ({2}): uses a license file; preserve and review the referenced upstream file." -f $name, $version, $source
  }

  if ($license -match "OFL-1.1") {
    return "{0} {1} ({2}): OFL-1.1 font license; acceptable for bundled fonts only after preserving font notices." -f $name, $version, $source
  }

  if ($license -match "\bAND\b") {
    return "{0} {1} ({2}): compound AND license expression needs manual review." -f $name, $version, $source
  }

  if ($license -match "LGPL|GPL|AGPL") {
    if ($license -match "\bOR\b" -and $license -match "MIT|Apache-2.0|BSD-2-Clause|BSD-3-Clause|ISC|Zlib") {
      return "{0} {1} ({2}): expression includes a copyleft alternative, but also a permissive alternative. Review once and use the permissive license path." -f $name, $version, $source
    }

    return "{0} {1} ({2}): copyleft license detected; do not bundle/link until reviewed and approved." -f $name, $version, $source
  }

  return $null
}

$lines = New-Object System.Collections.Generic.List[string]
$reviewNotes = New-Object System.Collections.Generic.List[string]
Add-Line $lines "# Third-Party Licenses"
Add-Line $lines
Add-Line $lines "Generated: $generatedAt"
Add-Line $lines
Add-Line $lines "This file is a best-effort notice index for PMTCONCON Studio. It does not replace legal review. Preserve upstream copyright and license notices when distributing binaries."
Add-Line $lines
Add-Line $lines "## Scope"
Add-Line $lines
Add-Line $lines "This notice covers libraries that are installed into the project and may be bundled, linked, or used to build the distributed app. It does not cover unrelated programs installed on the developer machine. External optimizer binaries are not bundled by PMTCONCON Studio."
Add-Line $lines
Add-Line $lines "## License Policy"
Add-Line $lines
Add-Line $lines "PMTCONCON Studio is MIT licensed. Built-in optimization must not bundle or link GPL, AGPL, LGPL, SSPL, BUSL, Commons Clause, PolyForm Noncommercial, commercial-only, unknown-license, or source-available-only dependencies."
Add-Line $lines

if (Test-Path -LiteralPath $packageJsonPath) {
  $package = Get-Content -LiteralPath $packageJsonPath -Raw | ConvertFrom-Json
  Add-Line $lines "## npm Dependencies"
  Add-Line $lines
  $allNpm = @()
  if ($package.dependencies) {
    $package.dependencies.PSObject.Properties | ForEach-Object {
      $allNpm += Get-NpmPackageInfo $_.Name $_.Value "dependency"
    }
  }
  if ($package.devDependencies) {
    $package.devDependencies.PSObject.Properties | ForEach-Object {
      $allNpm += Get-NpmPackageInfo $_.Name $_.Value "devDependency"
    }
  }
  foreach ($dep in ($allNpm | Sort-Object Name)) {
    Add-Line $lines ("- {0} {1} ({2}, declared {3}): {4}; {5}." -f $dep.Name, $dep.InstalledVersion, $dep.Scope, $dep.DeclaredVersion, $dep.License, $dep.Notice)
    $note = Get-LicenseReviewNote $dep.Name $dep.InstalledVersion $dep.License "npm"
    if ($note) {
      $reviewNotes.Add($note) | Out-Null
    }
  }
  Add-Line $lines
}

if (Test-Path -LiteralPath $cargoLockPath) {
  Add-Line $lines "## Rust Crates"
  Add-Line $lines
  foreach ($crate in Get-CargoPackagesForNotice $cargoLockPath) {
    $licenseInfo = Get-CrateLicense $crate.Name $crate.Version
    Add-Line $lines ("- {0} {1}: {2}; {3}." -f $crate.Name, $crate.Version, $licenseInfo.License, $licenseInfo.Notice)
    $note = Get-LicenseReviewNote $crate.Name $crate.Version $licenseInfo.License "Rust"
    if ($note) {
      $reviewNotes.Add($note) | Out-Null
    }
  }
  Add-Line $lines
}

Add-Line $lines "## Review Notes"
Add-Line $lines
if ($reviewNotes.Count -eq 0) {
  Add-Line $lines "- No generated review notes."
} else {
  foreach ($note in ($reviewNotes | Sort-Object -Unique)) {
    Add-Line $lines "- $note"
  }
}
Add-Line $lines

Add-Line $lines "## Skipped Tooling"
Add-Line $lines
Add-Line $lines "- `cargo-about`: optional, not installed automatically."
Add-Line $lines "- `cargo-deny`: optional, not installed automatically."
Add-Line $lines "- npm license notice tooling: not installed automatically."
Add-Line $lines
Add-Line $lines "## Denied Built-In Optimizer Dependencies"
Add-Line $lines
Add-Line $lines "The app must not bundle or link gifski, gifsicle, libimagequant/imagequant, pngquant, or ffmpeg as default built-in dependencies."

Set-Content -LiteralPath $outputPath -Value $lines -Encoding UTF8
Write-Host "Wrote $outputPath"
