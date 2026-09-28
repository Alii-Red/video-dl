-- Initial schema for Video DL. See ADR-0001 Partie 9.
-- Timestamps are RFC3339 strings; ids are UUID strings.

-- downloads: the "what"
CREATE TABLE IF NOT EXISTS downloads (
    id          TEXT PRIMARY KEY NOT NULL,
    source_url  TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    title       TEXT,
    format      TEXT NOT NULL,
    output_path TEXT,
    created_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_downloads_provider ON downloads(provider_id);

-- tasks: runtime state, 1-1 with downloads ("where it's at")
CREATE TABLE IF NOT EXISTS tasks (
    id               TEXT PRIMARY KEY NOT NULL,
    download_id      TEXT NOT NULL REFERENCES downloads(id) ON DELETE CASCADE,
    state            TEXT NOT NULL,
    progress_percent REAL NOT NULL DEFAULT 0,
    speed_bps        INTEGER,
    eta_secs         INTEGER,
    error_code       TEXT,
    error_message    TEXT,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL
);
-- The scheduler filters tasks by state on every dispatch loop.
CREATE INDEX IF NOT EXISTS idx_tasks_state ON tasks(state);
CREATE INDEX IF NOT EXISTS idx_tasks_download ON tasks(download_id);

-- queue: execution order for active tasks
CREATE TABLE IF NOT EXISTS queue (
    task_id     TEXT PRIMARY KEY NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    priority    INTEGER NOT NULL DEFAULT 0,
    position    INTEGER NOT NULL DEFAULT 0,
    enqueued_at TEXT NOT NULL
);
-- Sorted at each dispatch: highest priority first, then insertion order.
CREATE INDEX IF NOT EXISTS idx_queue_order ON queue(priority DESC, position ASC);

-- history: archive of completed downloads
CREATE TABLE IF NOT EXISTS history (
    id           TEXT PRIMARY KEY NOT NULL,
    download_id  TEXT NOT NULL REFERENCES downloads(id) ON DELETE CASCADE,
    completed_at TEXT NOT NULL,
    final_path   TEXT,
    duration_ms  INTEGER,
    bytes        INTEGER
);
CREATE INDEX IF NOT EXISTS idx_history_completed ON history(completed_at DESC);

-- providers: enable / per-provider config
CREATE TABLE IF NOT EXISTS providers (
    id          TEXT PRIMARY KEY NOT NULL,
    enabled     INTEGER NOT NULL DEFAULT 1,
    config_json TEXT
);

-- settings: simple key/value
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

-- playlists and their ordered items
CREATE TABLE IF NOT EXISTS playlists (
    id         TEXT PRIMARY KEY NOT NULL,
    title      TEXT NOT NULL,
    source_url TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS playlist_items (
    playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    download_id TEXT NOT NULL REFERENCES downloads(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    PRIMARY KEY (playlist_id, download_id)
);

-- logs: application logs surfaced in the UI (verbose logs stay in files)
CREATE TABLE IF NOT EXISTS logs (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    level        TEXT NOT NULL,
    message      TEXT NOT NULL,
    context_json TEXT,
    created_at   TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_logs_created ON logs(created_at DESC);
