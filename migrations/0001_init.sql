-- Esquema inicial de Diario-IA.
-- created_at se almacena como TEXT en RFC3339 UTC ("2026-07-14T20:30:00.123+00:00").
-- Los filtros por fecha usan substr(created_at, 1, 10) = 'YYYY-MM-DD'.

CREATE TABLE IF NOT EXISTS application (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    slug        TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    description TEXT,
    created_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS entry (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    application_id    INTEGER NOT NULL REFERENCES application(id) ON DELETE CASCADE,
    agent_name        TEXT NOT NULL,
    model             TEXT,
    title             TEXT NOT NULL,
    prompt            TEXT NOT NULL,
    task_summary      TEXT,
    response_markdown TEXT NOT NULL,
    response_html     TEXT NOT NULL,
    status            TEXT,
    tokens_input      INTEGER,
    tokens_output     INTEGER,
    duration_ms       INTEGER,
    metadata_json     TEXT,
    created_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_entry_app_created ON entry(application_id, created_at);
CREATE INDEX IF NOT EXISTS idx_entry_created ON entry(created_at);

CREATE TABLE IF NOT EXISTS attachment (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    entry_id         INTEGER NOT NULL REFERENCES entry(id) ON DELETE CASCADE,
    filename         TEXT NOT NULL,
    kind             TEXT,
    content_markdown TEXT NOT NULL,
    content_html     TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_attachment_entry ON attachment(entry_id);

CREATE TABLE IF NOT EXISTS entry_tag (
    entry_id INTEGER NOT NULL REFERENCES entry(id) ON DELETE CASCADE,
    tag      TEXT NOT NULL,
    PRIMARY KEY (entry_id, tag)
);
CREATE INDEX IF NOT EXISTS idx_entry_tag_tag ON entry_tag(tag);

CREATE TABLE IF NOT EXISTS api_key (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    name         TEXT NOT NULL,
    key_hash     TEXT NOT NULL UNIQUE,
    scope        TEXT NOT NULL DEFAULT 'write',
    active       INTEGER NOT NULL DEFAULT 1,
    created_at   TEXT NOT NULL,
    last_used_at TEXT
);
