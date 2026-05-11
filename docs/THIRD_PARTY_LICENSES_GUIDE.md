# Third-Party License Notice Guide

Use this guide when dependencies change.

1. Run `npm.cmd run license:forbidden`.
2. Run `npm.cmd run license:check`.
3. If optional tools are missing, record them as skipped rather than passed.
4. Run `npm.cmd run license:generate`.
5. Review `THIRD_PARTY_LICENSES.md` before release.

Do not install global tools automatically. Optional commands for the user:

```powershell
cargo install cargo-deny
cargo install cargo-about
```

If any dependency has unknown, NOASSERTION, SSPL, BUSL, Commons Clause, PolyForm Noncommercial, commercial-only, source-available-only, or GPL/AGPL/LGPL-only licensing, stop and request manual review.

If a license expression includes a permissive alternative, such as `MIT OR Apache-2.0 OR LGPL...`, review it once and document that PMTCONCON Studio uses the permissive path. Do not treat it the same as an LGPL-only dependency.

OFL-1.1 font packages require notice preservation review, but they are not external optimizer binaries.

Current reviewed exceptions are listed in `docs/LICENSE_POLICY.md` and regenerated into `THIRD_PARTY_LICENSES.md` by `scripts/generate-third-party-licenses.ps1`. Do not remove them from the generator; otherwise the unresolved review notes will return on the next license notice refresh.

For GIF/image resize or rescale changes, confirm that the generated `Image, GIF, and Resize License Coverage` section includes the Rust crates used by the built-in pipeline and that no forbidden external optimizer package has been added.

Review scope:

- Review libraries bundled, linked, or used for PMTCONCON Studio's build/runtime.
- Review external binaries only if they are shipped with the app or invoked by default.
- Do not include unrelated tools installed on the developer PC.
