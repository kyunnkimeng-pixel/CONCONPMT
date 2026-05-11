# Public Repository Sync

This repository is published from a local development workspace. The public repository state is the remote-tracking branch `origin/main`.

## Current Public Baseline

Refresh the remote-tracking branch before comparing:

```powershell
git fetch origin main
git rev-parse --short origin/main
```

The returned commit is the latest public GitHub update point.

## Compare Local Work Against Public

Show branch and working tree state:

```powershell
git status -sb --ignored
```

Show tracked local changes relative to the latest public commit:

```powershell
git diff --stat origin/main
git diff --name-status origin/main
```

Show untracked files that are not ignored:

```powershell
git ls-files -o --exclude-standard
```

## Local-Only Files

The following files are intentionally kept in the local workspace and ignored by Git:

- `AGENTS.md`
- `CODEX_COMMANDS.md`
- `WINDOWS_APP_THREAD_PROMPTS.md`
- `INITIAL_CODEX_PROMPT.md`
- `docs/UI_IMAGE_PROMPT.md`
- `qa-artifacts/`
- `docs/QA_*.md`
- `dist/`
- `src-tauri/target/`
- `src-tauri/gen/`
- `node_modules/`

## Publishing Checklist

1. Refresh `origin/main`.
2. Review `git status -sb --ignored`.
3. Review `git diff --stat origin/main`.
4. Stage only the files intended for the public repository.
5. Verify no local-only files are tracked:

```powershell
git ls-files | Select-String -Pattern '(^AGENTS\.md$|^CODEX_COMMANDS\.md$|^WINDOWS_APP_THREAD_PROMPTS\.md$|^INITIAL_CODEX_PROMPT\.md$|^docs/UI_IMAGE_PROMPT\.md$|^qa-artifacts/|^docs/QA_|^dist/|^node_modules/|^src-tauri/target/|^src-tauri/gen/)'
```

6. Commit and push:

```powershell
git commit -m "<public update summary>"
git push origin main
```

7. Confirm the new public point:

```powershell
git ls-remote origin refs/heads/main
```
