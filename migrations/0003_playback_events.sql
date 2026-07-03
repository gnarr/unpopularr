-- Individual playback sessions from Tautulli, the durable source of truth for
-- playback history. Deduplicated by the Tautulli history row id so re-syncs are
-- idempotent and events accumulate even after Tautulli purges its own history.
-- playback_snapshots (0002) is kept as a materialized aggregate recomputed from
-- this table on every successful sync.
CREATE TABLE playback_events (
    source_id TEXT NOT NULL REFERENCES playback_sources(id) ON DELETE CASCADE,
    source_row_id INTEGER NOT NULL,
    content_type TEXT NOT NULL CHECK (content_type IN ('movie', 'series', 'artist')),
    content_id TEXT NOT NULL,
    played_at TEXT NOT NULL,
    duration_seconds INTEGER NOT NULL CHECK (duration_seconds >= 0),
    PRIMARY KEY (source_id, source_row_id)
);

-- Serves the per-sync recompute (GROUP BY content_type, content_id WHERE source_id = ?).
CREATE INDEX idx_playback_events_content
    ON playback_events (source_id, content_type, content_id);

-- Serves the future minutes-per-week/month graph (time-bucketed range scans).
CREATE INDEX idx_playback_events_played_at
    ON playback_events (source_id, played_at);
