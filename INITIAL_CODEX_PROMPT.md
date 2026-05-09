You are building PMTCONCON Studio. The app name is fixed by the user and must not be renamed. It is a Windows Explorer-like desktop app for organizing, editing, previewing, validating, and exporting DCInside-style icon packs and custom emoticon packs.

First read these files in order:
1. AGENTS.md
2. docs/PRODUCT_SPEC.md
3. docs/FEATURE_INVENTORY.md
4. docs/IMPLEMENTATION_PLAN.md
5. docs/DECISIONS.md
6. docs/UI_IMAGE_PROMPT.md

Non-negotiable rules:
- The written feature inventory is the source of truth. Generated UI images are visual references only.
- Do not add dead menus or fake features.
- Do not omit a required feature just because it is missing from a generated image.
- Preserve original imported images/GIFs and store crop metadata separately.
- Persist collection names, icon names, alt values, order, crop boxes, representative images, profile settings, and GIF loop settings.
- Implement DCInside export validation: 10–200 output images, 200×200 default output cells, jpg/png/gif only, max 2MB per file, unique alt values, alt length 1–3 Korean grapheme characters, allowed specials *, ^, !, ~, +.
- Support custom target cell sizes for non-DC emoticon workflows.

Recommended stack:
- Tauri 2 + Rust backend/native commands
- React 19 + TypeScript + Vite frontend
- Tailwind CSS v4 + shadcn/ui
- TanStack Router, Zustand, dnd-kit, react-konva
- SQLite through Rust, preferably rusqlite for a local-first app
- Rust image processing with image/gif/fast_image_resize where appropriate

Codex workflow note:
- This project is being developed through the Codex Windows App. Use the integrated terminal, review/diff pane, and in-app browser where useful.
- Follow AGENTS.md and WINDOWS_APP_THREAD_PROMPTS.md for thread-level workflow.

Work approach:
1. Inspect the current scaffold.
2. Update docs/IMPLEMENTATION_PLAN.md with any concrete project-specific details.
3. Implement the app in vertical slices. Start with Phase 0 and continue through Phase 6 if feasible.
4. Keep docs/FEATURE_INVENTORY.md statuses updated.
5. Create docs/UI_TRACE.md mapping feature IDs to UI components/routes/commands.
6. Add automated tests where practical.
7. Run lint/test/build. If a check cannot run because the scaffold lacks a script, add the script or explain why.
8. Finish with a concise summary of implemented features, remaining gaps, and exact commands run.

When creating or using UI reference images:
- Use the prompts in docs/UI_IMAGE_PROMPT.md.
- Save generated references under docs/ui-references/.
- Never implement controls that appear only because the image generator invented them.
- If the generated image lacks a required feature, still implement that feature and update the UI accordingly.

Begin by planning briefly, then implement Phase 0. Continue to the next phases only after the app still builds.
