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

Migration 004 adds one-level subtasks, recurring series, and completion events. The series stores the template, rule, and original occurrence date so carrying a task forward leaves its recurrence cadence intact. Completion, skipping, and creating a successor use one transaction. A partial unique index limits each series to one active occurrence. If restoration would conflict with an active occurrence or restart a stopped series, the recovered task keeps its source-series reference and opens independently. Monthly rules skip months without the selected day. Completion events remain available for statistics after a task is moved to Trash or permanently deleted.

Migration 005 stores reminder preferences, per-category offsets, durable receipts, and an inbox. The worker checks active tasks every ten seconds. It groups newly due reminders by task and records receipts and inbox rows in one SQLite transaction. Suppressed reminders stay pending until quiet mode ends. Repeating tasks inherit deadline offsets and the DND exception, while one-time dates stay with the original occurrence.

On Windows, the installed identifier names the WinRT toast sender. The inbox preserves each reminder when Windows rejects presentation. The installer registers an app-specific URI for toast actions. Migration 008 stores expiring single-use tokens for those actions. Backups omit the tokens, and Replace discards them. The existing instance receives the action if Kairos is running. On startup, the application queues open and snooze actions until the task interface is ready. Clicking the notification opens the options for that reminder. Custom WAV playback uses the Windows audio device and mixer.

Migration 006 gives completion records a stable parent ID and archives past planned dates when a task is permanently deleted. The archive stores IDs and dates. The task board calculates daily and Monday–Sunday counts from active completion records in local time. The daily total pairs those completions with the remaining tasks in Today.

Streaks start on the first recorded activity date. Missed plans remain on their original days after automatic rescheduling. A day with only skipped occurrences and the selected weekly rest days are neutral. An unfinished plan breaks the streak after local midnight. Changing rest days recalculates streaks from the same completion history.

The `behavior` setting stores the window mode, startup page, and Smart mode executable whitelist. Smart mode checks the foreground window against its monitor bounds and updates the panel's topmost state when it changes. Outside fullscreen, an empty whitelist admits all foreground applications. A configured whitelist matches executable filenames without regard to case.

The Windows runtime uses Tauri 2 plugins for global shortcuts, per-user login registration, and single-instance handling. Tray restoration and the global recovery shortcut clear native click-through. Click-through also resets whenever Kairos starts. Hiding the panel leaves the reminder worker running.

Migration 007 stores custom reminder audio in SQLite so backups include it. Upgrades import the existing `notification.wav` during migration. Playback uses a regenerated local file for Windows audio.

Kairos uses SQLite's Online Backup API to write snapshots to a temporary file and publishes the file after integrity checks. The backup folder keeps the newest 30 managed files. Restore stages and validates the selected snapshot, makes a safety backup of the current database, and applies changes in a transaction. Replace restores records and app settings while retaining the local backup-folder location and Windows startup registration. Merge keeps current IDs, settings and sound. It adds missing tasks, maps colliding category names, and remaps imported reminder batch IDs and snooze receipts.

The NSIS installer registers a fresh installation for Windows sign-in and backs up an existing local database before copying new binaries. On uninstall, Windows removes the executable, shortcuts and startup entry. The default uninstaller preserves the application data and backups.
