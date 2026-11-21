# Architecture

- `src/` contains the React shell and typed client for Tauri commands.
- `src-tauri/src/` contains the desktop runtime and Rust-owned data access. Commands that touch SQLite run off the UI thread.
- `src-tauri/migrations/` contains ordered, append-only SQLite migrations. `PRAGMA user_version` is updated in the same transaction as the schema. Kairos refuses to open a database with a newer schema version.

## Local storage

On Windows, Kairos uses Tauri’s application data directory for the stable application identifier `app.kairos.desktop`. The database file is `kairos.sqlite3`. The application does not store task data in the installation directory or the Git checkout.

Migration 001 creates `tasks` with a stable text ID, a required title and a UTC creation timestamp. Later migrations extend this foundation. Foreign-key checking is enabled on every connection.

Migration 002 adds `app_settings`. The `panel` key stores the selected theme and transparency, the preferred display, and a layout for each display. Window layouts use display-relative logical coordinates. Kairos clamps the restored geometry to the display work area and caps the panel height at 70% of that work area.

The local WebView capability grants the window operations used by the panel. The Rust backend owns the tray and applies Windows Acrylic. The UI supplies a readable tinted surface when Acrylic is unavailable.

Migration 003 adds task details, categories, and schedule history while preserving tasks from earlier versions. Scheduled dates and deadlines are stored as independent local calendar fields. A missing deadline time is interpreted as 23:59. The task board command applies each edit and records automatic reschedules in a single SQLite transaction. Task settings and manual order are persisted locally.
