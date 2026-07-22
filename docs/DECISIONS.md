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
