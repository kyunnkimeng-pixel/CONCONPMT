# PMTCONCON Studio 0.3.0-alpha.1 Installer Distribution QA

Date: 2026-07-28

Version: 0.3.0-alpha.1

## Scope

This pass verifies the Windows artifact selected for the AI-workspace prerelease without
replacing the existing 0.2.0 stable release or changing the user's installed application.

## Artifacts

| Artifact | Path | Result |
| --- | --- | --- |
| Release executable | `src-tauri/target/release/pmtconcon-studio.exe` | built; version metadata checked |
| NSIS installer | `src-tauri/target/release/bundle/nsis/PMTCONCON Studio_0.3.0-alpha.1_x64-setup.exe` | selected for prerelease publication |
| Checksums | `src-tauri/target/release/bundle/SHA256SUMS.txt` | generated and verified |
| MSI installer | n/a | intentionally not built for this prerelease |

## Results

| Check | Result | Notes |
| --- | --- | --- |
| `npm.cmd run tauri -- build --bundles nsis` | passed | Built release executable and x64 NSIS setup. |
| `npm.cmd run release:checksums` | passed | Generated the NSIS-only checksum file. |
| NSIS size | passed | 5,547,656 bytes. |
| NSIS SHA-256 | passed | `4c4a54ae45cec120839e63f3b31c00d1e387eafcce5f8e36329ffe743f3ff9c8`. |
| Checksum filename | passed | Uses GitHub's dot-normalized root asset name `PMTCONCON.Studio_0.3.0-alpha.1_x64-setup.exe`. |
| Release executable metadata | passed | Product name `PMTCONCON Studio`; file/product version `0.3.0-alpha.1`. |
| Authenticode signature | noted | Installer is unsigned (`NotSigned`). |
| Silent install/uninstall | not run | Existing installation was preserved and no clean Windows VM was available. |
| Isolated packaged launch | not run | Windows known-folder app-data resolution could affect the real library, so this prerelease did not claim an isolated launch. |

## Distribution Decision

- Publish only the unsigned NSIS setup and its exact checksum.
- Publish as a GitHub prerelease with `latest=false`.
- Do not replace or edit the existing v0.2.0 assets.
- Keep MSI publication deferred until clean-machine install/uninstall QA succeeds.
- State that Windows may display an unknown-publisher warning.

## Remote Target

- Repository: `kyunnkimeng-pixel/CONCONPMT`
- Tag: `v0.3.0-alpha.1`
- Release: `PMTCONCON Studio v0.3.0-alpha.1`
- Assets: `PMTCONCON.Studio_0.3.0-alpha.1_x64-setup.exe`, `SHA256SUMS.txt`
- Body: `docs/RELEASE_NOTES_0.3.0-alpha.1.md`
