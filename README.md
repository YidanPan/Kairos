# Kairos

**At your own pace.**

Kairos is a minimalist, local-first personal task manager for Windows 11. It is designed as a compact desktop panel for planning tasks, tracking deadlines, and keeping routines.

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
