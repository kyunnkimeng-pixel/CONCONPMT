# Image Optimization Backend

Current stage: `LICENSE_SAFE_IMAGE_OPTIMIZATION_MVP`

This backend keeps PMTCONCON Studio MIT licensed by reusing the existing Rust `image`/`gif` export pipeline and avoiding bundled external optimizers.

## Architecture

- Baseline candidates are rendered through the same crop, resize, split, and loop settings as normal export.
- Candidate files are written under the app data generated variants directory.
- Candidate byte size is read from filesystem metadata after encoding.
- Original source files are never overwritten.
- Applying a candidate marks a processed asset variant active for one icon/profile/export piece.
- Export uses an active variant only when source, crop, and profile hashes still match.
- Final export output is still measured and validated after writing.

## Built-In Strategies

GIF:

- Render exact baseline GIF export piece.
- Preserve loop settings through the current GIF pipeline.
- Generate actual candidates with frame-preserving, moderate frame-reduction, and stronger frame-reduction presets where feasible.
- Measure every candidate.
- Show fallback suggestions when no candidate fits the active profile max bytes.

JPG:

- Render exact baseline export piece.
- Re-encode candidates using a quality ladder.
- Pick highest-quality measured candidates for quality, balanced, and smallest presets.

PNG:

- Render exact baseline export piece.
- Preserve alpha.
- Re-encode baseline and report practical fallback suggestions when the built-in permissive pipeline cannot reduce enough without changing format or dimensions.

## Anti-Crash Rules

- Generate candidates one item at a time.
- Use item-level `Result` errors.
- If one candidate fails, continue to the next candidate.
- Write candidate output to a temporary file before moving it into the variant store.
- Do not decode large GIF batches concurrently.
- Do not use `unwrap` or `expect` in candidate generation/export substitution paths.

## License Notes

The MVP does not add gifski, gifsicle, libimagequant/imagequant, pngquant, ffmpeg, or other external optimizer binaries.
