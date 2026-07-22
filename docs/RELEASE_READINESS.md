# PMTCONCON Studio Release Readiness

Last updated: 2026-07-23

This page is the public release-readiness checklist for PMTCONCON Studio. Internal native QA logs remain under ignored `docs/QA_*.md` files, while this page summarizes the user-visible readiness state that can be published with the repository.

## Release Scope

The current build covers the full local production workflow:

- Collection explorer, collection duplication, cover image management, and persistent navigation state.
- JPG, PNG, animated GIF, folder, drag-and-drop, and replacement import flows.
- Icon grid selection, Ctrl/Shift multi-select, drag ordering, delete, duplicate, context menus, memo notes, and alt editing.
- Single, horizontal double, and vertical double icon crop/export semantics.
- Compact sticky editor output preview with live GIF, text-overlay, piece-count, display-size, and output-size feedback.
- Bounded sequential imports with shared file, dimension, pixel, GIF-frame, and workload limits.
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

The latest release-readiness pass completed with required packaging checks passed and one environment-blocked native WebDriver run:

| Command | Result | Notes |
| --- | --- | --- |
| Markdown local-link check | passed | `README.md`, release notes, release readiness, and installer QA local links resolve. |
| `npm.cmd run lint` | passed | TypeScript compile check completed. |
| `npm.cmd run test` | passed | 11 frontend test files, 53 tests. |
| `cargo test --manifest-path src-tauri\Cargo.toml` | passed | 86 Rust tests, including import limits, sheet, GIF, presets, manual slice, auto-detect, export, and optimization coverage. |
| `npm.cmd run build` | passed | Vite emitted the known large-chunk warning. |
| `npm.cmd run license:forbidden` | passed | No forbidden optimizer dependency names found. |
| `npm.cmd run license:check` | passed | Optional `cargo-deny` and `cargo-about` were not installed and were marked skipped by the script. |
| `npm.cmd run tauri -- build` | passed | Built the release exe plus MSI and NSIS installers. |
| `npm.cmd run release:checksums` | passed | Regenerated `SHA256SUMS.txt` from the current NSIS installer only. |
| checksum verification | passed | NSIS SHA-256 hash matches `SHA256SUMS.txt`. |
| Editor output preview manual check | passed | The user confirmed the crop-adjacent compact sticky preview in the running desktop app before release packaging. |
| NSIS silent install | not rerun | An existing local installation was intentionally preserved; the same Tauri NSIS installer family passed silent install QA for v0.1.1. |
| release app launch | passed | The v0.1.2 release executable launched with isolated app data and window title `PMTCONCON Studio`. |
| MSI metadata validation | passed | Windows Installer database opens and reports product name `PMTCONCON Studio`, version `0.1.2`, and stable upgrade metadata. |
| Authenticode signature check | noted | The release exe, MSI, and NSIS setup are not signed. |
| native Tauri WebDriver smoke | environment-blocked | The tracked EdgeDriver supports Edge 147 while local WebView2 is 150, so no app session was created. Unit, Rust, packaging, checksum, MSI metadata, and isolated release startup checks passed independently. |

Generated installer paths from the latest pass:

- `src-tauri/target/release/bundle/nsis/PMTCONCON Studio_0.1.2_x64-setup.exe`
- `src-tauri/target/release/bundle/SHA256SUMS.txt`

The MSI artifact is built but withheld from the selected release assets until a clean Windows VM MSI install/uninstall pass is completed.

Remote publication status:

- GitHub release `v0.1.2` should be published as the latest release.
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
- [ ] Publish [Release Notes 0.1.2](RELEASE_NOTES_0.1.2.md) with the installer.
- [ ] Review [Installer Distribution QA](INSTALLER_DISTRIBUTION_QA.md) before publishing MSI artifacts.
- [ ] Review the diff and include only public repository files. Keep ignored native QA artifacts and local-only workflow files untracked.
