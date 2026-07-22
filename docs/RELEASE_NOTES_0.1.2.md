# PMTCONCON Studio 0.1.2 Release Notes

PMTCONCON Studio 0.1.2 is a Windows reliability and usability update. It keeps the existing local editing and export workflow while making imports safer, collection management more predictable, dialogs more accessible, and crop results easier to inspect.

## Highlights

- The icon editor now keeps a compact live output preview directly below the crop canvas and pins it above scrolled settings.
- Ordinary image, GIF, replacement, and work-sheet imports now enforce bounded file, dimension, pixel, frame, and workload limits.
- Collection deletion and navigation refresh behave consistently across the toolbar, context menu, keyboard, and sidebar.
- Custom dialogs now provide consistent focus trapping, Escape handling, and focus restoration.
- The JavaScript dependency lock is standardized on npm for reproducible Windows builds.

## Editor Output Preview

- Renamed the editor result area from `처리 미리보기` to `출력 미리보기`.
- Moved the live draft preview directly below the source crop canvas.
- Kept the preview visible while shape, cell size, crop mode, alignment, and GIF settings scroll underneath it.
- Added display size, output piece size, piece count, and `적용 전` context.
- Fit single, horizontal double, vertical double, and large custom previews into a bounded 220×128 display area without changing export dimensions.
- Preserved animated GIF playback, text overlays, and double-icon split lines.

## Import Safety And Reliability

- Process normal multi-file and folder imports sequentially instead of sending one unbounded payload.
- Reject oversized or excessive image workloads per file while allowing valid neighboring files to continue importing.
- Apply shared decode limits to normal images, replacement images, static sheets, GIF frame sheets, reimports, manual slices, and auto-detect analysis.
- Report concise Korean skip and validation reasons for unsupported or excessive inputs.
- Keep imported originals preserved in the application library.

## Explorer And Accessibility Improvements

- Added collection deletion through the toolbar, collection context menu, and Delete key with confirmation.
- Refresh the sidebar collection list immediately after collection mutations.
- Make the collection command bar wrap at the default window width instead of clipping actions.
- Prevent stale editor requests from replacing the currently selected icon state.
- Add dialog semantics, keyboard focus containment, Escape-to-close behavior, and focus restoration to custom modal surfaces.
- Repair corrupted Korean GIF and import error messages.

## Export And Dependency Notes

- Ordinary export warnings remain advisory and non-blocking; strict warning blocking remains optional.
- Added and committed `package-lock.json`, removed the old pnpm lockfile, and removed an unused vulnerable CLI dependency.
- Regenerated third-party notices while preserving the MIT license and dependency guardrails.

## Distribution Notes

- The selected Windows release artifact is the NSIS setup for version 0.1.2.
- `SHA256SUMS.txt` contains the checksum for the published NSIS setup.
- The installer is currently unsigned, so Windows may display a publisher warning.
- The MSI package is built but withheld until a clean Windows VM install/uninstall pass is completed.
- PMTCONCON Studio remains MIT licensed and local-only.

## Known Notes

- The production web bundle reports a large JavaScript chunk warning. This does not block the desktop package.
- Existing user libraries remain compatible; this release does not introduce a destructive data migration.
