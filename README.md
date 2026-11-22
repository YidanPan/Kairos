# Kairos

**At your own pace.**

Kairos is a minimalist, local-first personal task manager for Windows 11. It is designed as a compact desktop panel for planning tasks, tracking deadlines, and keeping routines.

## Windows controls

Close hides Kairos to the system tray so reminders can keep running. Use the tray to reopen the panel, restore mouse interaction after enabling click-through, or exit. **Settings → Behavior** contains launch at login, window modes, and shortcut registration status.

| Global shortcut | Action |
| --- | --- |
| `Ctrl+Alt+Shift+T` | Show or hide Kairos |
| `Ctrl+Alt+Shift+N` | Quick Add |
| `Ctrl+Alt+Shift+F` | Search |
| `Ctrl+Alt+Shift+S` | Open Settings |
| `Ctrl+Alt+Shift+R` | Restore mouse interaction |

## Backup and recovery

Open **Settings → Data and backups** to make a snapshot, choose its folder, export it, or import an existing snapshot. Kairos makes automatic daily backups and keeps up to 30 snapshots in the active backup folder. Restore shows the impact before you confirm and saves the current data first. **Merge** adds missing tasks and keeps local edits. **Replace** restores the database contents and app settings from the selected backup.

Build the Windows x64 installer with `npm run tauri build`. The NSIS executable is written to `src-tauri/target/release/bundle/nsis/`. Uninstallation preserves local data by default. Select the data removal checkbox only when you intend to delete it.

## Project layout

| Path | Purpose |
| --- | --- |
| `src/` | React and TypeScript interface |
| `src-tauri/` | Rust backend, SQLite migrations, Windows integration, and installer configuration |
| `docs/` | Architecture, development notes, and verification results |

## Development

Use Windows 11 with Node.js 22.13+ LTS, the stable Rust MSVC toolchain, Microsoft C++ Build Tools with the Desktop development with C++ workload, and the WebView2 runtime.

```powershell
npm ci
npm run tauri dev
```

```powershell
npm run lint
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build -- --no-bundle
```

See the [architecture](docs/architecture.md) for database details, the [progress log](docs/progress.md) for verification results, the [product requirements](PRD.md) for the roadmap, and the [development rules](DEVELOPMENT_RULES.md) for Git and quality gates.
