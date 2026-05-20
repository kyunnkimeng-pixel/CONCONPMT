# PMTCONCON Studio Installer Distribution QA

Date: 2026-05-14

Version: 0.1.1

## Scope

This pass checks whether the Windows distribution artifacts can be published with usable integrity information and whether the NSIS installer produces a launchable installed app.

## Artifacts

| Artifact | Path | Result |
| --- | --- | --- |
| MSI installer | `src-tauri/target/release/bundle/msi/PMTCONCON Studio_0.1.1_x64_en-US.msi` | built, not selected for publishing |
| NSIS installer | `src-tauri/target/release/bundle/nsis/PMTCONCON Studio_0.1.1_x64-setup.exe` | selected for publishing |
| Checksums | `src-tauri/target/release/bundle/SHA256SUMS.txt` | regenerated as NSIS-only and verified |

## Results

| Check | Result | Notes |
| --- | --- | --- |
| `npm.cmd run tauri -- build` | passed | Built release exe, MSI, and NSIS setup. Vite emitted the known large chunk warning. |
| `npm.cmd run release:checksums` | passed | Rewrote `SHA256SUMS.txt` for the current NSIS installer only. |
| Checksum verification | passed | `SHA256SUMS.txt` entries match the actual NSIS installer hash. |
| NSIS silent install | passed | `PMTCONCON Studio_0.1.1_x64-setup.exe /S` exited with code 0. |
| Installed app launch | passed | Installed app at `%LOCALAPPDATA%\PMTCONCON Studio\pmtconcon-studio.exe` launched through Tauri driver with window title `PMTCONCON Studio`. |
| MSI metadata validation | passed | Windows Installer database opens and reports product name `PMTCONCON Studio`, version `0.1.1`, and stable upgrade metadata. |
| Authenticode signature check | noted | MSI, NSIS setup, and installed exe are currently unsigned. |

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

The public `v0.1.1` release should be published at `https://github.com/kyunnkimeng-pixel/CONCONPMT/releases/tag/v0.1.1`.

Required remote state:

- Release name: `PMTCONCON Studio v0.1.1`
- Release state: published
- Assets include the NSIS setup and NSIS-only `SHA256SUMS.txt`

Required before considering the remote release complete:

- Upload the current NSIS setup artifact.
- Upload the current NSIS-only `SHA256SUMS.txt`.
- Add or update the release body from `docs/RELEASE_NOTES_0.1.1.md`.

Publication requires GitHub release mutation access.
