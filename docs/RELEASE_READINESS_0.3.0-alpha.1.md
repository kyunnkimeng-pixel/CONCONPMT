# PMTCONCON Studio 0.3.0-alpha.1 Release Readiness

Last updated: 2026-07-28

This page records the prerelease-readiness result for the local-result AI workspace
checkpoint. The latest stable release remains 0.2.0.

## Scope

Version 0.3.0-alpha.1 contains:

- Provider-neutral request, candidate, icon-version, source-lineage and active-source
  persistence.
- Restart-safe original/previous-version rollback and fail-closed source validation.
- Deterministic JPG/PNG candidate normalization with contain-pad and cover-crop modes.
- A dedicated three-tab AI workspace for local result import, candidate comparison and
  source history.
- Safe new-icon creation, compatible current-icon activation, repeat-creation history,
  reveal/open/continue actions and full nested-modal accessibility.
- Effective-source integration across preview, export, optimization, static work sheets,
  GIF frame sheets and collection/icon cloning.

This checkpoint does not contain a provider API, API-key UI, browser automation, GIF
frame AI processing or sprite-sheet AI generation.

## Verification Matrix

```powershell
cargo fmt --manifest-path src-tauri\Cargo.toml -- --check
npm.cmd run lint
npm.cmd run test
cargo test --manifest-path src-tauri\Cargo.toml
npm.cmd run build
npm.cmd run license:forbidden
npm.cmd run license:check
git diff --check
npm.cmd run tauri -- build --bundles nsis
npm.cmd run release:checksums
```

## Results

| Check | Result | Notes |
| --- | --- | --- |
| Rust format | passed | `cargo fmt -- --check`. |
| Frontend lint | passed | TypeScript compile check completed. |
| Frontend tests | passed | 38 files, 248 tests. |
| Rust tests | passed | 232 tests, including migration, provenance, normalization, rollback, clone, cleanup, export, optimizer and sheet regressions. |
| Production web build | passed | Vite emitted the existing large-chunk warning. |
| Dependency/license guardrails | passed | No forbidden dependency names; optional `cargo-deny` and `cargo-about` were unavailable and explicitly skipped. |
| Diff hygiene | passed | No whitespace errors or conflict markers; only existing LF-to-CRLF checkout warnings. |
| Browser workflow QA | passed | Existing headed AI-UX-3 run passed 13/13 at 1200×760 and 800×760 with one document-wide live region, no overflow and no unexpected provider/network call. |
| Tauri packaging | passed | Built the release executable and x64 NSIS setup for 0.3.0-alpha.1. |
| NSIS checksum | passed | Generated checksum independently matches the installer SHA-256. |
| Executable metadata | passed | Product name `PMTCONCON Studio`; product/file version `0.3.0-alpha.1`. |
| Authenticode | noted | Release executable and installer are unsigned. |
| MSI | intentionally skipped | Prerelease is NSIS-only; MSI remains withheld until clean-VM install/uninstall QA. |
| Packaged launch/install | not run | Existing local installation and user library were preserved; no isolated Windows VM was available. |

The non-blocking Vite warning reports a roughly 1,059 kB main JavaScript chunk after
minification.

## Generated Artifacts

- `src-tauri/target/release/pmtconcon-studio.exe`
- `src-tauri/target/release/bundle/nsis/PMTCONCON Studio_0.3.0-alpha.1_x64-setup.exe`
- `src-tauri/target/release/bundle/SHA256SUMS.txt`

Installer size: 5,547,656 bytes.

Installer SHA-256:
`4c4a54ae45cec120839e63f3b31c00d1e387eafcce5f8e36329ffe743f3ff9c8`

## Safety

- Imported originals are immutable.
- Migrations 012–016 are additive and validated by migration/FK tests.
- Provider/network/token code is absent from this checkpoint.
- No runtime dependency was added.
- PMTCONCON Studio remains MIT licensed.
- Local Playwright output is excluded from Git.

## Publication Target

- Tag: `v0.3.0-alpha.1`
- Release name: `PMTCONCON Studio v0.3.0-alpha.1`
- Release type: GitHub prerelease, never GitHub Latest
- Notes: [`RELEASE_NOTES_0.3.0-alpha.1.md`](RELEASE_NOTES_0.3.0-alpha.1.md)
- Public assets: unsigned NSIS setup and matching `SHA256SUMS.txt`
- Stable release retained: `v0.2.0`
