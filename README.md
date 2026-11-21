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

Development starts with a runnable Tauri 2 application and versioned SQLite migrations. See the [product requirements](PRD.md) for the roadmap and the [development rules](DEVELOPMENT_RULES.md) for Git and quality gates.
