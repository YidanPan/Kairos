# Architecture

- `src/` contains the React shell and typed client for Tauri commands.
- `src-tauri/src/` contains the desktop runtime and Rust-owned data access. Commands that touch SQLite run off the UI thread.
- `src-tauri/migrations/` contains ordered, append-only SQLite migrations. `PRAGMA user_version` is updated in the same transaction as the schema. Kairos refuses to open a database with a newer schema version.

## Local storage

On Windows, Kairos uses Tauri’s application data directory for the stable application identifier `app.kairos.desktop`. The database file is `kairos.sqlite3`. The application does not store task data in the installation directory or the Git checkout.

Migration 001 creates `tasks` with a stable text ID, a required title and a UTC creation timestamp. Later migrations extend this foundation. Foreign-key checking is enabled on every connection.
