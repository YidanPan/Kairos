CREATE TABLE reminder_activation_tokens (
    batch_id INTEGER PRIMARY KEY REFERENCES reminder_batches(id) ON DELETE CASCADE,
    token TEXT NOT NULL UNIQUE,
    expires_at INTEGER NOT NULL
) STRICT;
