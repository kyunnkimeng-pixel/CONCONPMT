# PMTCONCON Studio Installer Distribution QA

Date: 2026-07-23

Version: 0.1.2

## Scope

This pass checks whether the Windows distribution artifacts can be published with usable integrity information and whether the packaged release executable starts without touching the existing local installation.

## Artifacts

| Artifact | Path | Result |
| --- | --- | --- |
| MSI installer | `src-tauri/target/release/bundle/msi/PMTCONCON Studio_0.1.2_x64_en-US.msi` | built, not selected for publishing |
| NSIS installer | `src-tauri/target/release/bundle/nsis/PMTCONCON Studio_0.1.2_x64-setup.exe` | selected for publishing |
| Checksums | `src-tauri/target/release/bundle/SHA256SUMS.txt` | regenerated as NSIS-only and verified |

## Results

| Check | Result | Notes |
| --- | --- | --- |
| `npm.cmd run tauri -- build` | passed | Built release exe, MSI, and NSIS setup. Vite emitted the known large chunk warning. |
| `npm.cmd run release:checksums` | passed | Rewrote `SHA256SUMS.txt` for the current NSIS installer only. |
| Checksum verification | passed | `SHA256SUMS.txt` entries match the actual NSIS installer hash. |
| NSIS silent install | not rerun | The existing local PMTCONCON Studio installation was intentionally preserved; v0.1.1 previously passed the same NSIS silent-install path. |
| Release app launch | passed | The packaged v0.1.2 executable launched with isolated app data and window title `PMTCONCON Studio`. |
| MSI metadata validation | passed | Windows Installer database opens and reports product name `PMTCONCON Studio`, version `0.1.2`, and stable upgrade metadata. |
| Authenticode signature check | noted | The release exe, MSI, and NSIS setup are currently unsigned. |
| Native Tauri WebDriver smoke | environment-blocked | The QA EdgeDriver supports Edge 147 while local WebView2 is 150, so session creation stopped before the app flow. Other automated checks and isolated release startup passed. |

## Issue Found And Fixed

### IDQA-001: stale checksum file after rebuild

Severity: release-blocking if unresolved.

The existing `SHA256SUMS.txt` did not match the freshly rebuilt MSI and NSIS installers. A release checksum file must match the exact artifacts being published.

Fix:

- Added `scripts/write-release-checksums.ps1`.
- Added `npm.cmd run release:checksums`.
- Regenerated `src-tauri/target/release/bundle/SHA256SUMS.txt`.
- Rechecked both installer hashes against the regenerated checksum file.

Status: fixed and retested.

## Open Distribution Note

`msiexec /a` administrative extraction did not complete within 180 seconds in this local environment and produced no extraction log or files. The stray Windows Installer process was stopped. MSI database metadata validation passed, but a clean Windows VM/manual MSI install check is still required before publishing the MSI package.

Clean Windows VM status:

- Not run in this environment.
- No VirtualBox, VMware, Hyper-V cmdlets, or Windows Sandbox command surface is available.
- Checking Windows Sandbox optional-feature state requires elevation here.

## Distribution Recommendation

- Publish the NSIS installer with the matching NSIS-only `SHA256SUMS.txt`.
- Clearly label the Windows installer as unsigned until a signing certificate is configured.
- Do not publish the MSI until a separate clean-machine MSI install/uninstall pass is completed.

## Remote Publication Check

The public `v0.1.2` release should be published at `https://github.com/kyunnkimeng-pixel/CONCONPMT/releases/tag/v0.1.2`.

Required remote state:

- Release name: `PMTCONCON Studio v0.1.2`
- Release state: published
- Assets include the NSIS setup and NSIS-only `SHA256SUMS.txt`

Required before considering the remote release complete:

- Upload the current NSIS setup artifact.
- Upload the current NSIS-only `SHA256SUMS.txt`.
- Add or update the release body from `docs/RELEASE_NOTES_0.1.2.md`.

Publication requires GitHub release mutation access.
