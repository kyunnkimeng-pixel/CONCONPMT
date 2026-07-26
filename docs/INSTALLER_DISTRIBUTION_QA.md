# PMTCONCON Studio Installer Distribution QA

Date: 2026-07-26

Version: 0.2.0

## Scope

This pass verifies the Windows release artifacts selected for PMTCONCON Studio 0.2.0,
their integrity metadata, and isolated packaged startup without changing the existing
local installation.

## Artifacts

| Artifact | Path | Result |
| --- | --- | --- |
| Release executable | `src-tauri/target/release/pmtconcon-studio.exe` | built and isolated-launch tested |
| MSI installer | `src-tauri/target/release/bundle/msi/PMTCONCON Studio_0.2.0_x64_en-US.msi` | built, metadata checked, not selected for publishing |
| NSIS installer | `src-tauri/target/release/bundle/nsis/PMTCONCON Studio_0.2.0_x64-setup.exe` | selected for publishing |
| Checksums | `src-tauri/target/release/bundle/SHA256SUMS.txt` | regenerated as NSIS-only and verified |

## Results

| Check | Result | Notes |
| --- | --- | --- |
| `npm.cmd run tauri -- build` | passed | Built release executable, MSI, and NSIS setup for x64 Windows. |
| `npm.cmd run release:checksums` | passed | Generated the checksum file from the current 0.2.0 NSIS artifact only. |
| NSIS size | passed | 5,374,624 bytes. |
| NSIS SHA-256 | passed | `81ad31a91f38309d0ac3327eb6b58252fa86e9b470fc9c5b9939a5246fbd8800`. |
| Checksum comparison | passed | Actual NSIS hash matches `SHA256SUMS.txt`, whose filename entry is the root-level GitHub asset basename. |
| Release executable metadata | passed | Product name `PMTCONCON Studio`, file/product version `0.2.0`. |
| Release app launch | passed | Packaged executable opened with isolated app data and window title `PMTCONCON Studio`. |
| Startup database | passed | SQLite integrity `ok`; 11 migrations applied through `011_icon_motion_recipes`. |
| MSI metadata | passed | Product name `PMTCONCON Studio`, version `0.2.0`, product code and stable upgrade code are present. |
| Authenticode signature | noted | NSIS setup is currently unsigned (`NotSigned`). |
| NSIS silent install | not rerun | The existing local installation was preserved. The release executable and isolated data initialization were tested directly. |
| Native click-through automation | environment-blocked | The Windows automation wrapper failed to initialize under the current sandbox ACL after two attempts. |

## Open Distribution Note

A clean Windows VM install/uninstall pass is not available in this environment. The MSI
is therefore built for validation but withheld from GitHub. The NSIS setup is the same
Tauri installer family previously used by the project and is paired with an exact
SHA-256 checksum.

## Distribution Recommendation

- Publish the unsigned NSIS installer with the matching NSIS-only checksum file.
- State that Windows may display an unknown-publisher warning.
- Do not publish the MSI until a separate clean-machine install/uninstall pass succeeds.

## Remote Publication Target

- Repository: `kyunnkimeng-pixel/CONCONPMT`
- Tag: `v0.2.0`
- Release name: `PMTCONCON Studio v0.2.0`
- Assets: `PMTCONCON Studio_0.2.0_x64-setup.exe`, `SHA256SUMS.txt`
- Body: `docs/RELEASE_NOTES_0.2.0.md`
