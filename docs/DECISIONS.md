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
