# GIF File-Size Optimization Design

Current stage: `GIF_FILE_SIZE_OPTIMIZATION_FULL_DESIGN`

This document describes the public design for local GIF size optimization in PMTCONCON Studio. It is design documentation; implementation status is tracked in `docs/FEATURE_INVENTORY.md` and the source code.

## Product Goal

The optimizer should help users reduce oversized GIF export results without damaging imported originals. It belongs in the export workflow because users discover the problem when generated output exceeds the active profile byte limit.

## Goals

- Detect oversized GIF export pieces during preflight or final validation.
- Show which export pieces exceed the active `max_bytes` value.
- Generate actual encoded candidate files instead of estimating success from settings.
- Let users compare baseline and candidate output before applying a result.
- Store applied results as generated variants.
- Re-run export validation after a candidate is applied.
- Export from the active variant only when the source, crop, and profile still match.

## Non-Goals

- No destructive overwrite of original source GIF files.
- No bundled external optimizer binaries.
- No network upload, cloud processing, scraping, login, or posting.
- No optimizer action should be shown before there is working local behavior behind it.

## MVP Scope

The license-safe MVP uses the existing Rust `image` and `gif` pipeline:

- Render a baseline GIF using the same crop, resize, split, format, and loop settings as export.
- Measure byte size from the encoded file on disk.
- Generate a small set of candidate variants.
- Preserve animation timing and loop settings where possible.
- Store candidates in the generated variants area.
- Allow one candidate to become the active export variant.

Advanced visual-difference scoring, external optimizer integration, and batch optimization remain future work.

## User Flow

1. The user opens the export workspace.
2. A GIF item is marked as over the configured byte limit.
3. The user opens optimization for that item.
4. The backend renders and measures the baseline export piece.
5. If optimization is needed, the backend generates candidates.
6. The UI shows each candidate with byte size, limit, frame count change, color limit, duration change, loop behavior, and quality note.
7. The user previews a candidate and applies it.
8. The app marks the candidate as the active variant and revalidates the item.

## Candidate Strategy

Candidate presets should be understandable:

- Quality-first: smallest change from the baseline.
- Balanced: moderate size reduction with conservative frame handling.
- Size-first: strongest built-in reduction that still preserves animation.

Each candidate must be backed by a real encoded file. A candidate is valid only when the file exists, animation is preserved, and byte size is known from filesystem metadata.

## Data Model

Generated variants should track:

- Variant ID.
- Icon ID and piece index.
- Source asset hash.
- Crop and profile hash.
- Strategy name.
- Output path.
- Byte size.
- Frame count.
- Duration.
- Loop mode.
- Created timestamp.
- Active/inactive state.

## Safety Rules

- Original imported GIFs are never overwritten.
- Candidate generation uses temporary files before storing final variants.
- Candidate failures are item-level errors.
- Failed candidates do not prevent other candidates from running.
- Export falls back to normal rendering when no active variant matches the current source, crop, and profile.
- Final export output is measured again after writing.

## License Position

The built-in optimizer must remain compatible with the MIT-licensed project. It must not bundle or link tools such as gifski, gifsicle, libimagequant/imagequant, pngquant, ffmpeg, or other dependencies with incompatible or unclear licensing.
