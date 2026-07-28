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

## ADR-014: AI separates immutable provider candidates from icon-scoped versions

AI support is provider-neutral and candidate-first. It must not overwrite
`icons.source_file_id`, original bytes, crop data, or deterministic edit recipes. Preview
artifacts are newly rendered to icon/activation-owned durable paths and switched by DB pointer.

Decision:
- Persist each provider execution as a mutable status/retention record in `ai_requests`
  and its immutable raw outputs in `ai_candidates`. Register authoritative bytes in the
  existing content-addressed `source_files`; request input/handoff folders are temporary
  copies, not source of truth.
- Bind a candidate to a specific icon and original lineage through `icon_ai_versions`.
  Use `icons.original_lineage_id` plus monotonic `original_lineage_generation`
  independently from content-addressed source identity so same-byte replacement cannot
  revive old history and legacy manifests can detect any replacement. Store normalization/
  effective source on the version and a revisioned nullable pointer in `icon_ai_state`.
  `NULL` selects the original; selecting an old version or `NULL` is a local rollback.
- Backfill every existing icon with a distinct lineage, generation 0 and original-only AI
  state. DB lineage defaults, an atomic state trigger and one guarded repository helper
  cover every import, placeholder, duplicate, sheet and clone insert path. Enforce nullable
  `SET NULL` request origins, restricted candidate/source provenance, a lineage-scoped
  parent FK, and an icon-scoped active-version FK plus lineage CAS. Store safely decoded
  `pmtcon-alpha-v1` metadata on `source_files`: true means an actual non-opaque pixel in any
  displayed frame, not merely an alpha channel. Never infer it from an extension.
- Resolve one `EffectiveVisualSource` at the start of every preview, export, optimizer,
  GIF-FPS, static-sheet and GIF-sheet operation. Sheet manifests retain original identity
  and separately record lineage/generation plus effective render source/hash in versioned
  static/GIF v2 schemas; legacy manifests require inactive AI and generation 0. AI changes
  only the base source; native edit stages remain durable.
- Broken/missing state, source files, SHA or decode metadata fail closed and block
  render/export rather than silently selecting the original. The editor exposes separate
  original and effective-render sources. `processed_asset_variants` records the effective
  source ID/hash, while non-render ownership queries remain on a documented direct-original
  allowlist.
  Reconcile legacy nullable variant source IDs only on unambiguous owning-original ID/SHA
  matches. Bounded-check legacy artifacts and backfill `output_sha256`; otherwise deactivate
  them and replace promoted previews with native effective-source renders. Require all new
  variants to carry matching non-null source ID/SHA and output digest provenance.
- Default to adding a candidate as a new working icon with empty alt. Preserve provider-raw
  and deterministic normalized sources. Activate only compatible `base_source` versions on
  the current icon; rendered-viewport and GIF-poster results create new full-canvas icons
  so existing recipes are not applied twice.
  A base-source new-icon transaction includes its candidate child/active state before final
  effective-source resolution, variants and previews; failure compensates the entire icon
  instead of leaving a committed clone awaiting a second activation.
- Keep the editor entry compact and perform import/generation, large candidate comparison,
  normalization, activation and history in one large in-app AI workspace dialog. Preserve
  arbitrary-size raw JPG/PNG candidates and create a separate deterministic
  `contain_pad`/`cover_crop` source for the effective base canvas. Default to
  contain + transparent padding, show raw/normalized/final output before commit, and provide
  explicit post-create reveal/open actions. Provider controls appear only when their full
  flow is implemented.
- Request-time payload, request-recipe and activation-recipe signatures are distinct:
  base-source payload signatures exclude downstream recipes, rendered-viewport signatures
  include them, and the activation signature always covers the latest full native recipe.
- Activation and rollback use prepare snapshot, staging render, full-recipe CAS and one
  short transaction. After CAS, staging files are atomically renamed to preallocated
  icon/activation-owned durable paths and those paths plus version/pointer/revision are committed;
  failures use DB rollback, file compensation and reference-aware crash-orphan cleanup.
  Network, manual web waits and rendering never hold the transaction.
- A normal image replacement stages the new source/preview and atomically updates the
  original, always issues a new lineage ID, increments lineage generation, resets edit
  geometry, clears active AI state, increments activation revision and marks old-signature
  requests superseded. Old versions remain history but cannot be activated on the new
  lineage, including an `A → B → A` replacement.
- Keep pixelation, color adjustment, transforms and motion in the deterministic native
  renderer; use AI for semantic style, character, background and composition changes.
- Keep PMTCONCON Studio free/MIT and do not operate a shared provider account, shared
  credential, metered proxy or paid AI service. Each user's provider subscription, credits,
  charges and compliance remain on that user's account.
- Use NovelAI Image API as the first automated adapter and conditional primary. Accept only
  a user-issued Persistent API Token, session-first, and initially allow only exact
  `https://image.novelai.net:443/ai/generate-image`. Do not accept account login credentials
  or call primary login/token APIs. Require one human action for one request/sample and
  prohibit background batches, chained generation, automatic retries and provider fallback.
  Release requires official support clarification and a user-approved small live pilot
  because the public schema leaves model/action, rate and API Anlas details incomplete.
  A newly issued PAT invalidates the old PAT and is only shown once, so clear frontend token
  input after the invoke handoff, never auto-read the clipboard, and handle `401` without
  retry by giving rotation/re-entry guidance.
- Treat NovelAI text-to-image, img2img and inpaint as the first capability set. General Opus
  zero-Anlas conditions exclude base-image generation. The official web Inpaint guide
  separately documents zero-Anlas Focused Inpainting on large-image regions for Opus, but
  the public Image API does not document Focused payload support or charging parity. Treat
  API inpaint cost as provider-confirmation-required until support clarification and the
  pilot. Do not convert subscription Anlas to USD actual/billed or assume alpha/animation.
- Preserve manual website handoff as a token-free fallback. It may package the image/mask/
  references and prompt, open an official website, and import a user-selected result. It
  records service surface, account context and typed policy references, may represent
  model/cost as unverified, and must not automate login, cookies, DOM upload, scraping, or
  downloads.
- Gemini is not the default: current image generation has no free tier and requires a
  separate age/audience, professional/business, region and paid-service distribution gate.
  OpenAI Image API remains a separately reviewed optional paid-cloud adapter. A generic
  local adapter is a separate literal-loopback-only security stage with redirects denied.
- Use BYOK only. Secrets never enter SQLite, durable settings, request history or logs.
  A Tauri desktop backend remains a client: OS credential storage protects at rest but
  does not remove runtime theft risk. Start with session keys, code-owned exact HTTPS
  origins, strict CSP and narrow Rust commands; persistent keys require credential-store/
  license review.
- Persist only provider-qualified, versioned, bounded canonical allowlist snapshots for
  adapter contract/capability/data-tier/retention/consent and prompt/options, not binary
  payloads, complete requests/responses, secret references or secrets.
- Foundation and the first cloud-adapter stage persist `credential_mode_snapshot` only. They
  create no credential binding table/column/FK and reject `os_vault_ref`.
- Persistent credentials require a separate later Stage Gate. It must introduce a secret-
  free `ai_credential_bindings` parent, nullable request FK `ON DELETE SET NULL`, and
  adapter/provider consistency guards while keeping tokens only in the OS vault. Deletion
  first marks the binding `deleting`, then removes the vault entry and DB row; interruption
  is a retryable repair state. Its migration tests must prove no cascade into AI history.
- Provider changes require a new request and consent. Disabling an adapter, clearing a
  session token or deleting a future persistent binding never deletes candidates/versions or
  changes the active pointer, so local rollback remains available.
- Do not bundle local AI runtimes, model weights, workflows, or copyleft/unclear
  dependencies. ComfyUI may only be considered as a separately installed user endpoint
  reached by independently written bounded HTTP integration. Disclose that its workflow
  may itself call external paid Partner Nodes, so only the app-to-endpoint hop is local.
- Keep GIF frame batches and sprite-sheet generation experimental. A larger sheet is not
  assumed to be cheaper, and static poster replacement is never silently treated as a
  full animated edit.
- The repository's MIT license covers PMTCONCON Studio code, not provider/model/workflow
  terms, attribution duties or generated-output rights. Store typed references and show
  this boundary during consent/export instead of promising rights the app cannot grant.
- Icon and collection clones share mutable request execution provenance, immutable
  candidates and bytes, but map every distinct historical lineage one-to-one to a new ID,
  preserve generation, and receive new version/state IDs and independent previews. The fixed
  order is durable recipes, the complete AI DAG/state, target effective source, compatible
  variants, then preview paths. Copy a variant only when source/crop hashes, format and an
  ID/path-independent output-profile compatibility hash match the final target. Otherwise
  skip its bytes/row and promoted-preview remap and render natively; never relabel old bytes
  with the new effective-source hash. Every failure rolls back DB state and compensates
  files. A pending request's late candidate is
  not auto-attached to a clone. Usage/cost is counted once per distinct request. Cleanup
  protects referenced sources and expires temporary sensitive input/handoff/staging copies
  under an explicit retention policy.
- Preserve original and version source bytes through ordinary cleanup so rollback remains
  local and available until the user explicitly confirms permanent AI-history deletion
  after seeing affected rollback points, descendants and clones.

Reasoning:
- Generative output is not reproducible enough for “call the model again” to be a
  rollback mechanism. Persisted bytes and a local pointer make rollback immediate,
  restart-safe, provider-independent and free of extra API charges.
- Separating execution candidates from per-icon versions avoids duplicating provider
  identity, usage and cost when a result becomes a new icon or a collection is cloned,
  while preserving independent rollback trees.
- Separating AI base sources from export optimization variants prevents provider
  provenance from being mixed with profile/piece-specific derivatives.
- Keeping deterministic PMTCONCON Studio recipes outside the AI version lets users
  remove AI while retaining later crop, text, effect and motion edits.
- NovelAI's documented third-party PAT flow fits a free desktop BYOT product and its
  anime/img2img/inpaint tools fit emoticon work, while the pilot gate contains its broad
  credential and incomplete billing/schema contract. Manual handoff remains a reusable
  no-token fallback because both routes share the same candidate model.

Tradeoffs:
- All render consumers must be migrated to the shared effective-source resolver.
- Current-icon activation initially requires a compatible canvas; rendered-viewport
  edits are added as new icons to avoid double-applying existing recipes.
- Candidate/version history consumes storage, and external-transfer copies are sensitive;
  both need explicit reference-aware cleanup instead of automatic orphan deletion or
  indefinite retention.

Detailed contracts: `docs/AI_INTEGRATION_DESIGN.md` and
`docs/AI_WORKSPACE_UX_DESIGN.md`.

## ADR-015: AI-UX completion uses atomic mutation snapshots and direct-create provenance

Decision:
- Current-icon AI activation and rollback return `AiReviewState` and `IconEditorState`
  from the same immediate transaction. Callers adopt that response and do not issue a
  mutation-followup read that could observe a different revision.
- Migration `016_ai_icon_root_creations` records only successful explicit
  `create_ai_icon_root` operations. The user-facing count means icons directly added
  with this candidate in this collection; ordinary icon duplicates and collection clones
  are deliberately excluded even though their AI version DAG keeps shared execution
  provenance. Existing installations receive no guessed historical backfill.
- Counts and latest links ignore soft-deleted icons. The latest link follows durable
  creation order, falls back to the previous non-deleted direct result, and becomes empty
  when none remain. Target icon deletion cascades its direct-create row, source icon
  deletion nulls the optional source identity, and candidate deletion is restricted.
- After a successful create, the AI workspace remains open and offers explicit open,
  reveal and continue-comparing actions. Reveal requests select, scroll and focus the exact
  tile; opening also crosses the existing unsaved-editor confirmation boundary.

Reasoning:
- A same-transaction response prevents an avoidable race between a committed AI source and
  the crop/editor state rendered immediately afterward.
- A dedicated direct-create record makes duplicate-use messaging exact without interpreting
  cloned `new_icon_root` version rows as additional user actions.

Tradeoffs:
- Pre-016 direct creates are not counted. This is more honest than presenting inferred
  history as exact, and future explicit creates become precise immediately.
