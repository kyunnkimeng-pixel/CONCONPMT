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

`pnpm-lock.yaml` is preserved for now. Do not delete it until the npm workflow has been confirmed stable and a maintainer intentionally removes the old pnpm lockfile.
