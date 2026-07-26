# DECISIONS.md — Architecture decisions

## ADR-001: Desktop framework
Use Tauri 2 instead of Electron.

Reasoning:
- The app needs native file dialogs, drag/drop, local filesystem access, export folder opening, and local image processing.
- Tauri keeps the UI in a web frontend while using Rust for native commands and image work.
- The app should feel lightweight and Windows-friendly.

Tradeoff:
- Rust imaging/GIF work requires more care than Node-only libraries.
- Some DOM drag/drop behavior may need Tauri-specific configuration and testing on Windows.

## ADR-002: Frontend
Use React + TypeScript + Vite.

Reasoning:
- The UI has many stateful interactions: explorer grid, multi-select, drag reorder, inline rename, editor panel, preview simulator.
- React componentization helps map each feature ID to a component.
- Vite keeps development fast and simple.

## ADR-003: Styling and components
Use Tailwind CSS v4 and shadcn/ui.

Reasoning:
- Tailwind v4 CSS-first tokens are good for a custom Windows 11-inspired design system.
- shadcn/ui components are editable source, which helps prevent opaque UI behavior and dead menus.

## ADR-004: Persistence
Use SQLite in the app data directory.

Reasoning:
- The app must persist order, alt values, crop boxes, GIF loop settings, and asset metadata.
- SQLite is sufficient and local-first.

## ADR-005: Image generation workflow
Use Codex image generation only for visual references and small assets.

Reasoning:
- Generated UI images can omit requested controls or invent fake controls.
- The implementation must follow `FEATURE_INVENTORY.md`, not the generated image.
- Every generated design reference must be traced to components through `docs/UI_TRACE.md`.


## ADR-006: Codex workflow
Use the Codex Windows App as the primary development interface.

Reasoning:
- The user plans to proceed in the Windows desktop app rather than primarily through Codex CLI.
- Codex App threads, integrated terminal, Git diff/review pane, in-app browser, and image generation can all support this project workflow.
- `AGENTS.md` remains the repository-level instruction source; `WINDOWS_APP_THREAD_PROMPTS.md` provides paste-ready thread prompts.

Tradeoff:
- Some setup commands still run in PowerShell or the app's integrated terminal because Tauri scaffolding and build verification are local shell tasks.

## ADR-007: Untrusted image import limits
Apply the same hard resource limits to normal image imports, cover/source replacement, and sheet tools.

Reasoning:
- Compressed images and long GIFs can consume far more memory and CPU than their file size suggests.
- Browser-to-Tauri byte arrays amplify memory use, so ordinary folder imports are sent one file at a time.
- A rejected file must not prevent other valid files in the same folder from importing.

Limits:
- 64MB per input file.
- 12,000px maximum per dimension and 32 million pixels total.
- 500 GIF frames and 128 million cumulative frame pixels.
- 64MB aggregate for multi-file sheet requests that must remain atomic.

## ADR-008: npm dependency reproducibility
Use npm 11 with a committed `package-lock.json` as the only JavaScript package-manager lock.

Reasoning:
- `packageManager` already declares npm, and a mixed pnpm lock prevented reproducible `npm ci` setup.
- The unused shadcn CLI package was removed; shadcn/ui-style editable source and `components.json` remain in the repository.

## ADR-009: External editor references are UX research, not embedded code
Use official user documentation from mature sprite/image editors to study workflows,
but keep PMTCONCON Studio's editor and renderer independently implemented.

Reasoning:
- Aseprite's main application source and official binaries are governed by the Aseprite
  EULA, while some separately identified modules are MIT; PMTCONCON Studio does not copy
  either category as part of this reference work.
- Pixelorama, Piskel, miniPaint, TOAST UI Image Editor, and Filerobot can inform common
  feature categories, but embedding a full editor would duplicate the current Konva/Rust
  pipeline and would not automatically guarantee consistency with the Rust GIF
  preview/export path.
- Aseprite's offset, sprite size, padding, sheet type, rows/columns, empty-cell, and
  metadata concepts apply to both PMTCONCON Studio's existing static multi-emoticon
  sheet workflow and its implemented frame-sheet-to-GIF workflow; they are not treated as
  GIF-only research.
- PMTCONCON Studio needs a narrow emoticon workflow, not a layer/cel/paint application.

Constraints:
- Do not copy or bundle Aseprite code, binaries, CLI, icons, themes, screenshots, sample
  media, or UI assets.
- Do not reproduce another editor's UI pixel-for-pixel.
- Any future dependency or bundled effect asset still requires the normal license and
  notice guardrails.
- Rust rendering remains the source of truth; browser filters may only preview the same
  documented recipe.

## ADR-010: Canonical non-destructive transform semantics
Store icon transforms as quarter turns plus a canonical horizontal reflection, while
retaining separate horizontal/vertical controls in the UI.

Reasoning:
- Rotations and axis reflections form only eight distinct visual states. Canonicalizing
  equivalent button sequences keeps persistence and optimization hashes stable.
- Odd quarter turns atomically swap non-square cell width/height and
  `horizontal_double`/`vertical_double` shape.
- Piece IDs and alt text follow visual content; piece indexes and roles describe final
  output position.
- The authoritative render order is text overlay, source crop, pre-transform viewport
  resize, whole-viewport transform, then piece split. This prevents seams caused by
  transforming pieces independently.
- Static images and every GIF frame use the same recipe. Source replacement explicitly
  resets crop and transform rather than silently applying old geometry to new content.

## ADR-011: Motion effects are a required, bounded native stage
Motion effects are an implemented editor stage after deterministic static effects, not
an optional backlog candidate. The durable contract is a revisioned, normalized
`pmtcon-motion-v1` recipe per icon.

Reasoning:
- The editor exposes 16 reusable presets in four categories: spatial transforms,
  procedural displacement, animated color/opacity, and overlays. At most one effect per
  category is enabled; the canonical order is spatial, displacement, color/opacity,
  then overlay.
- Static effects render first on the combined viewport, motion renders second, and only
  then are multi-piece icons split. Preview, export, optimization, static sheets, and
  GIF frame sheets share that native order and recipe hash.
- Static inputs with enabled motion become GIFs. Existing GIFs derive phase from
  cumulative frame timestamps rather than frame index and retain effective source loop
  behavior. Normalized phase, integer cycles, and a persisted seed make loops and
  particle/jitter variation reproducible.
- Displacement uses bounded procedural inverse sampling. Bilinear sampling operates on
  premultiplied alpha, with nearest-neighbor available for pixel art and explicit
  transparent/clamp/mirror edge modes.
- Native measurement encodes the actual editor-preview GIF and returns total and
  per-piece bytes, frame count, duration, effective loop, clipping, and warnings.
  Measurement is invalidated when its revision or render signature is stale; final
  export bytes are still revalidated with the selected profile and optimizer.
- Heavy rendering snapshots DB inputs and releases the shared SQLite lock before
  encoding, then rechecks inputs in a short transaction before commit. Request previews
  and superseded saved artifacts are bounded and only deleted when no durable path
  references them.
- Static work sheets intentionally contain only the 0ms poster frame and an animation-
  loss warning. Their `render_recipe_hash` includes motion, so processed-output reimport
  skips a stale cell instead of writing or applying it. GIF frame sheets remain the full-
  frame, duration, and loop round-trip path.
- Icon and collection duplication preserve the recipe while keeping mutable preview
  ownership independent. The implementation adds no new runtime dependency.

Constraints:
- A profile that disallows GIF cannot export an enabled motion recipe.
- User-provided displacement maps, freeform liquify/warp, arbitrary shaders, executable
  effect plugins, unlimited motion stacks, and a general layer/brush editor remain out
  of scope.

## ADR-012: Frame-sheet GIF creation uses measured recipe commits

Create a GIF from a manifest-free frame sheet through two native operations that share
one deterministic renderer: measure, then commit.

Reasoning:
- The browser owns transient frame-strip interaction, but the backend recalculates grid
  cells from the source sheet and never accepts client-provided crop coordinates.
- Duplicate frames are repeated cell references rather than duplicated source pixels.
  Reverse and endpoint-safe ping-pong are materialized into the encoded sequence.
- GIF timing is persisted as per-frame milliseconds normalized to 10ms units. FPS is
  only a UI convenience.
- Measurement writes a managed temporary GIF and returns its actual bytes and render
  hash. Commit re-renders and rejects a stale hash instead of trusting a temp path.
- The source sheet and a versioned `pmtcon-frame-sheet-gif-v1` recipe provenance row are
  retained while the encoded GIF is registered as an ordinary animated source/icon.
- Partial alpha, palette quantization, collection byte limits, 500 final frames, and
  128 million cumulative frame pixels are checked or warned before commit.

## ADR-013: Static effects use revisioned ordered recipes and native exact rendering

Persist the curated static effect stack per icon as a revisioned
`pmtcon-effects-v1` ordered JSON recipe. Rust rendering is authoritative.

Reasoning:
- The first seven effect kinds are pixelate, color adjustment, grayscale/sepia tone,
  blur, sharpen, outline, and shadow. Stable step IDs, enabled flags, bounded
  parameters, and ordering are durable edit state.
- Optimistic revision checks prevent an older editor draft from silently replacing a
  newer recipe.
- Effects run on the combined viewport after crop/resize and whole-viewport transform,
  before piece splitting. The same recipe is applied to every GIF frame.
- Native exact preview, generated preview, final export, optimizer analysis, static
  sheets, and GIF frame-sheet source/render hashes use the same normalized recipe.
- Existing image/GIF code is sufficient for this bounded set, so no effect dependency
  or embedded third-party editor runtime is introduced.
