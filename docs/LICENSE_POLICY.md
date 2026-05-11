# License Policy

PMTCONCON Studio is distributed under the MIT License. The repository license, npm package metadata, and Rust crate metadata must remain MIT unless the user explicitly approves a legal review and relicensing plan.

## Allowed Licenses

Built-in dependencies may be used by default when their license metadata is clear and compatible with MIT distribution:

- MIT
- Apache-2.0
- Apache-2.0 WITH LLVM-exception
- BSD-2-Clause
- BSD-3-Clause
- ISC
- Zlib
- CC0-1.0
- Unicode-3.0
- Unicode-DFS-2016

## Review Required

These require manual review before adding or upgrading:

- MPL-2.0
- OpenSSL license
- OFL-1.1 for bundled fonts
- License expressions involving `AND`
- License expressions that include a GPL/LGPL/AGPL alternative through `OR`, even when a permissive alternative is available
- Custom licenses
- Unclear transitive native dependencies
- NOASSERTION or unknown metadata

## Denied by Default

Do not add dependencies under:

- GPL, AGPL, or LGPL when there is no clear permissive alternative license path
- SSPL
- BUSL
- Commons Clause
- PolyForm Noncommercial
- Commercial-only licenses
- Source-available-only licenses
- Unknown or NOASSERTION metadata unless manually verified

## Built-In Optimizer Policy

Built-in GIF, PNG, and JPG optimization must preserve PMTCONCON Studio's MIT licensing posture. The optimizer should reuse the existing Rust imaging pipeline first. It may use only clearly permissive-compatible crates.

The following are not allowed as bundled, linked, or default dependencies for this MIT app:

- gifski
- gifsicle
- libimagequant / imagequant
- pngquant
- ffmpeg

## External Optimizer Future Policy

Optional external optimizer integration is future-only, disabled by default, and out of scope for the built-in MVP. If ever added, it must be explicit, user-configured, documented separately, and must not ship the external binary in PMTCONCON Studio.

## Third-Party Notices

- Preserve upstream copyright and license notices.
- Update `THIRD_PARTY_LICENSES.md` when dependencies change.
- Do not claim legal review from generated notices.
- During release preparation, regenerate notices with `npm.cmd run license:generate` and run license checks where tools are available.

## What Must Be Reviewed

Review dependencies that are bundled, linked, or used to build the distributed app:

- Rust crates in `src-tauri/Cargo.lock`.
- npm packages declared in `package.json` and installed in `node_modules` for the frontend build.
- Any binary that would be shipped inside the app bundle.

Do not treat every unrelated program installed on the developer machine as an app dependency. For example, an optional external optimizer that is not bundled, not linked, and not invoked by default is not part of the built-in app license surface. If it becomes a user-configured future integration, document it separately and keep it disabled by default.

If a dependency license is written as `MIT OR Apache-2.0 OR LGPL...`, review it once and use the permissive MIT/Apache path for PMTCONCON Studio distribution. That is different from adding an LGPL-only dependency.

`Cargo.lock` itself does not store license fields, so notice generation reads local Cargo registry `Cargo.toml` metadata when available. If local metadata is unavailable, leave the item as `UNKNOWN` or run a proper notice tool such as `cargo-about`.

## Guardrail Commands

```powershell
npm.cmd run license:forbidden
npm.cmd run license:check
npm.cmd run license:generate
```

Optional tools are not installed automatically:

```powershell
cargo install cargo-deny
cargo install cargo-about
```

## Release Checklist

- Root `LICENSE` is MIT.
- `package.json` has `"license": "MIT"`.
- `src-tauri/Cargo.toml` has `license = "MIT"`.
- `npm.cmd run license:forbidden` passes.
- `npm.cmd run license:check` either passes or records skipped optional tooling.
- `THIRD_PARTY_LICENSES.md` is current.
- No denied optimizer dependency is bundled, linked, or used by default.
