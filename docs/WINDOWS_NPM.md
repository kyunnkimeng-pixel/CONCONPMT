# Windows npm Command Guide

PMTCONCON Studio uses npm on this machine.

PowerShell may block `npm.ps1` because of ExecutionPolicy. Use the command shim instead:

```powershell
npm.cmd run tauri -- dev
npm.cmd run lint
npm.cmd run build
```

Equivalent `cmd.exe` form:

```powershell
cmd /c "npm run tauri -- dev"
```

`pnpm-workspace.yaml` was removed because this is not a pnpm workspace and the file only carried pnpm-specific build approvals.

`package-lock.json` is the only JavaScript dependency lockfile. It was generated with npm 11 and verified with a clean `npm.cmd ci --ignore-scripts --workspaces=false` install. Use `npm.cmd ci` for reproducible setup; do not add a pnpm lockfile unless the project explicitly changes package managers.
