# PMTCONCON Studio Release Readiness

Last updated: 2026-07-26

This page records the public release-readiness result for PMTCONCON Studio 0.2.0.
Temporary native QA data and build artifacts remain outside the tracked source tree.

## Release Scope

Version 0.2.0 covers the complete local production workflow from the previous release
plus the five editor-completeness stages:

- Exact-scope reset/restore labels, public-manual discovery, memo access, and clearer
  sheet/collection tooltips.
- Non-destructive horizontal/vertical flip and 90-degree rotation for static images,
  GIFs, non-square cells, and horizontal/vertical multi-piece icons.
- Arbitrary frame-sheet to GIF creation with grid review, frame selection/reordering,
  per-frame duration, realtime playback, and forward/reverse/ping-pong generation.
- Versioned deterministic static effects and 16 bounded motion presets spanning spatial,
  procedural displacement, color/opacity, and overlay categories.
- Complete collection duplication with remapped durable metadata, independent previews,
  frame-sheet provenance, collection presets, and effective active optimized variants.

Imported originals remain immutable. Preview, export, optimization, static work sheets,
and GIF frame sheets use the same native render recipes.

## Verification Matrix

The release candidate was checked from the repository root with:

```powershell
cargo fmt --manifest-path src-tauri\Cargo.toml -- --check
npm.cmd run lint
npm.cmd run test
cargo test --manifest-path src-tauri\Cargo.toml
npm.cmd run build
npm.cmd run license:forbidden
npm.cmd run license:check
git diff --check
npm.cmd run tauri -- build
npm.cmd run release:checksums
```

## Current Verification Result

| Check | Result | Notes |
| --- | --- | --- |
| Rust format | passed | `cargo fmt -- --check`. |
| Frontend lint | passed | TypeScript compile check completed. |
| Frontend tests | passed | 25 files, 154 tests. |
| Rust tests | passed | 176 tests, including transform, frame-sheet GIF, effects, motion, clone, export, optimizer, and sheet regressions. |
| Production web build | passed | Vite emitted the known large-chunk warning. |
| Dependency/license guardrails | passed | No forbidden dependency names; optional `cargo-deny` and `cargo-about` were unavailable and explicitly skipped. |
| Diff hygiene | passed | No whitespace errors or conflict markers. Git reports only the repository's existing LF-to-CRLF checkout warnings. |
| Tauri packaging | passed | Built the release executable, x64 MSI, and x64 NSIS setup for 0.2.0. |
| NSIS checksum | passed | SHA-256 matches `SHA256SUMS.txt`; its filename entry matches GitHub's dot-normalized downloaded asset name. |
| Packaged executable metadata | passed | Product name `PMTCONCON Studio`, file/product version `0.2.0`. |
| Isolated packaged launch | passed | Release executable opened with window title `PMTCONCON Studio` and created a separate app-data library. |
| Isolated database startup | passed | SQLite integrity `ok`; all 11 migrations applied through `011_icon_motion_recipes`. |
| MSI metadata | passed | Product name/version and stable upgrade metadata were readable from the MSI database. |
| Authenticode | noted | Release executable and installers are unsigned. |
| Native click-through automation | environment-blocked | The Windows automation runtime could not initialize under the current sandbox ACL. Component tests, native tests, packaging, isolated startup, and database checks passed independently. |

Expected non-blocking note: Vite reports a roughly 978 kB main JavaScript chunk after
minification. This does not prevent the desktop package from starting.

## Generated Artifacts

- `src-tauri/target/release/pmtconcon-studio.exe`
- `src-tauri/target/release/bundle/msi/PMTCONCON Studio_0.2.0_x64_en-US.msi`
- `src-tauri/target/release/bundle/nsis/PMTCONCON Studio_0.2.0_x64-setup.exe`
- `src-tauri/target/release/bundle/SHA256SUMS.txt`

The selected public assets are the NSIS setup and the NSIS-only checksum file. The MSI
is withheld until a clean Windows VM install/uninstall pass is completed.

## User Data And License Safety

- Imported originals are copied into the app library and are not overwritten by crop,
  resize, transform, effects, motion, sheet reimport, or optimization flows.
- New SQLite migrations are additive and preserve existing libraries.
- PMTCONCON Studio remains MIT licensed and local-only.
- No new dependency was added for the 0.2.0 editor-completeness implementation.
- Third-party notices are published in [`../THIRD_PARTY_LICENSES.md`](../THIRD_PARTY_LICENSES.md).

## Publication Target

- Tag and release: `v0.2.0`
- Release name: `PMTCONCON Studio v0.2.0`
- Release notes: [`RELEASE_NOTES_0.2.0.md`](RELEASE_NOTES_0.2.0.md)
- Public assets: unsigned `PMTCONCON.Studio_0.2.0_x64-setup.exe` plus matching `SHA256SUMS.txt`
- MSI publication: deferred pending clean-machine QA
