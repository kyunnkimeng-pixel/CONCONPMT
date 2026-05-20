# PMTCONCON Studio Release Readiness

Last updated: 2026-05-14

This page is the public release-readiness checklist for PMTCONCON Studio. Internal native QA logs remain under ignored `docs/QA_*.md` files, while this page summarizes the user-visible readiness state that can be published with the repository.

## Release Scope

The current build covers the full local production workflow:

- Collection explorer, collection duplication, cover image management, and persistent navigation state.
- JPG, PNG, animated GIF, folder, drag-and-drop, and replacement import flows.
- Icon grid selection, Ctrl/Shift multi-select, drag ordering, delete, duplicate, context menus, memo notes, and alt editing.
- Single, horizontal double, and vertical double icon crop/export semantics.
- GIF preview, crop/resize, loop mode, ping-pong, text overlay, file-size candidates, and playback FPS variant application.
- Usage preview in a DCInside-like comment layout.
- Export workspace validation, selected-item rerun, optimization candidates, and `alts.txt` generation.
- Static Sprite Sheet / Work Sheet import, export, manifest reimport, PNG alpha preservation, page splitting, selected-icon sheet export, shared grid presets, manual slice mode, and experimental auto-detect proposals.
- GIF frame work sheet export/reimport with manifest-based duration and loop preservation.

## User Data Safety

- Imported originals are copied into the app library and are not overwritten by crop, resize, sheet reimport, GIF reimport, or optimization flows.
- Reimported static sheets create new icons or processed variants according to the selected mode.
- Reimported GIF frame sheets create a new animated GIF variant; the original GIF source remains intact.
- Generated files live under predictable app-data output folders and are removed only by explicit cleanup behavior.
- PMTCONCON Studio does not implement DCInside login, upload, posting, scraping, browser automation, cloud sync, account, marketplace, or premium features.

## License Readiness

- Repository license: MIT.
- Dependency policy: no GPL, AGPL, LGPL, SSPL, BUSL, Commons Clause, PolyForm Noncommercial, commercial-only, source-available-only, unknown-license, or NOASSERTION dependencies.
- Forbidden bundled/default tools remain disallowed: gifski, gifsicle, libimagequant, imagequant, pngquant, and ffmpeg.
- Third-party notices are published in [`../THIRD_PARTY_LICENSES.md`](../THIRD_PARTY_LICENSES.md).
- License policy is documented in [`LICENSE_POLICY.md`](LICENSE_POLICY.md).

## Verification Matrix

Run these commands from the repository root before tagging or publishing a release:

```powershell
npm.cmd run lint
npm.cmd run test
cargo test --manifest-path src-tauri\Cargo.toml
npm.cmd run build
npm.cmd run license:forbidden
npm.cmd run license:check
npm.cmd run tauri -- build
npm.cmd run release:checksums
```

Expected notes:

- Vite may report a large chunk warning; this is not currently a release blocker.
- `license:check` may skip optional `cargo-deny` or `cargo-about` if those tools are not installed, but the repository guardrails still run.
- Native UI evidence and temporary app data belong under the ignored local QA artifact directory.

## Current Verification Result

The latest release-readiness pass completed successfully:

| Command | Result | Notes |
| --- | --- | --- |
| `node <docs-link-check>` | passed | `docs/index.html`, `README.md`, and this page resolve local links and manual image assets. |
| `npm.cmd run lint` | passed | TypeScript compile check completed. |
| `npm.cmd run test` | passed | 9 frontend test files, 46 tests. |
| `cargo test --manifest-path src-tauri\Cargo.toml` | passed | 84 Rust tests, including sheet, GIF, presets, manual slice, auto-detect, export, and optimization coverage. |
| `npm.cmd run build` | passed | Vite emitted the known large-chunk warning. |
| `npm.cmd run license:forbidden` | passed | No forbidden optimizer dependency names found. |
| `npm.cmd run license:check` | passed | Optional `cargo-deny` and `cargo-about` were not installed and were marked skipped by the script. |
| `npm.cmd run tauri -- build` | passed | Built the release exe plus MSI and NSIS installers. |
| `npm.cmd run release:checksums` | passed | Regenerated `SHA256SUMS.txt` from the current NSIS installer only. |
| checksum verification | passed | NSIS SHA-256 hash matches `SHA256SUMS.txt`. |
| NSIS silent install | passed | The setup installed to `%LOCALAPPDATA%\PMTCONCON Studio` and exited with code 0. |
| installed app launch | passed | The installed app launched through Tauri driver with window title `PMTCONCON Studio`. |
| MSI metadata validation | passed | Windows Installer database opens and reports product name `PMTCONCON Studio`, version `0.1.1`, and upgrade metadata. |
| Authenticode signature check | noted | The MSI, NSIS setup, and installed exe are not signed. |
| `node <native-smoke-harness>` | passed | Native Tauri E2E smoke covered startup, collection operations, import, alt validation, context menus, selection/reorder, editor apply, GIF loop, usage preview, and export validation. |

Generated installer paths from the latest pass:

- `src-tauri/target/release/bundle/nsis/PMTCONCON Studio_0.1.1_x64-setup.exe`
- `src-tauri/target/release/bundle/SHA256SUMS.txt`

The MSI artifact is built but withheld from the selected release assets until a clean Windows VM MSI install/uninstall pass is completed.

Remote publication status:

- GitHub release `v0.1.1` should be published as the latest release.
- Publish only the current NSIS setup plus NSIS-only `SHA256SUMS.txt`.

## Manual QA Coverage

Recent native/manual QA covered:

- App launch with isolated temp app data.
- Collection creation, rename, duplicate, sorting, and navigation.
- Multi-file and folder import.
- Alt validation, batch alt editing, selection, context menus, delete cancel, and drag reorder.
- Icon memo save, hover, edit, and clear.
- Editor crop apply, horizontal double split, GIF loop/FPS, text overlay, and live preview.
- Static sheet import/export/reimport with transparent PNG fixtures.
- Selected-only work sheet export.
- Grid preset save/apply/default behavior.
- Manual slice mode and auto-detect proposal flow.
- GIF frame sheet export/reimport dialog validation.
- Export workspace validation and packaging build.

## Release Checklist

- [ ] Verify `README.md` links to the public manual and release notes.
- [ ] Verify `docs/index.html` references only tracked `docs/manual-assets/manual-*` screenshots.
- [ ] Verify all linked manual assets exist.
- [ ] Run the verification matrix above.
- [ ] Confirm `THIRD_PARTY_LICENSES.md` is current when dependencies change.
- [ ] Confirm the installer artifacts are generated by `npm.cmd run tauri -- build`.
- [ ] Run `npm.cmd run release:checksums` after the final package build.
- [ ] Publish [Release Notes 0.1.1](RELEASE_NOTES_0.1.1.md) with the installer.
- [ ] Review [Installer Distribution QA](INSTALLER_DISTRIBUTION_QA.md) before publishing MSI artifacts.
- [ ] Review the diff and include only public repository files. Keep ignored native QA artifacts and local-only workflow files untracked.
