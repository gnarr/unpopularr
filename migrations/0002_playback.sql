CREATE TABLE playback_sources (
    id TEXT PRIMARY KEY NOT NULL,
    provider TEXT NOT NULL CHECK (provider IN ('tautulli')),
    last_successful_sync_at TEXT
);

CREATE TABLE playback_sync_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id TEXT NOT NULL REFERENCES playback_sources(id) ON DELETE CASCADE,
    trigger TEXT NOT NULL CHECK (trigger IN ('startup', 'scheduled', 'manual')),
    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'partial', 'failed')),
    started_at TEXT NOT NULL,
    completed_at TEXT,
    matched_history_rows INTEGER NOT NULL DEFAULT 0 CHECK (matched_history_rows >= 0),
    unmatched_history_rows INTEGER NOT NULL DEFAULT 0 CHECK (unmatched_history_rows >= 0),
    error TEXT
);

CREATE TABLE playback_snapshots (
    source_id TEXT NOT NULL REFERENCES playback_sources(id) ON DELETE CASCADE,
    content_type TEXT NOT NULL CHECK (content_type IN ('movie', 'series', 'artist')),
    content_id TEXT NOT NULL,
    play_count INTEGER NOT NULL CHECK (play_count >= 0),
    play_duration_seconds INTEGER NOT NULL CHECK (play_duration_seconds >= 0),
    last_played_at TEXT,
    PRIMARY KEY (source_id, content_type, content_id)
);

CREATE INDEX idx_playback_sync_runs_started_at
    ON playback_sync_runs(started_at DESC);
