CREATE TABLE task_alert_preferences (
    task_id TEXT PRIMARY KEY NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    overrides INTEGER NOT NULL CHECK (overrides IN (0, 1)),
    allow_during_dnd INTEGER NOT NULL CHECK (allow_during_dnd IN (0, 1)),
    rules TEXT NOT NULL
) STRICT;
CREATE TABLE category_alert_preferences (
    category_id TEXT PRIMARY KEY NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
    offsets TEXT NOT NULL
) STRICT;
CREATE TABLE reminder_receipts (
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    token TEXT NOT NULL,
    due_at INTEGER NOT NULL,
    PRIMARY KEY (task_id, token, due_at)
) STRICT;
CREATE TABLE reminder_batches (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    first_due_at INTEGER NOT NULL,
    count INTEGER NOT NULL CHECK (count > 0),
    silent INTEGER NOT NULL CHECK (silent IN (0, 1)),
    native_status TEXT NOT NULL DEFAULT 'pending' CHECK (native_status IN ('pending','accepted','unavailable')),
    dismissed INTEGER NOT NULL DEFAULT 0 CHECK (dismissed IN (0, 1)),
    snoozed_for INTEGER
) STRICT;
CREATE INDEX reminder_inbox ON reminder_batches(dismissed, created_at);
