-- Aggregate playback snapshots created before playback_events existed cannot be
-- decomposed into exact source history rows. Keep them as a baseline so the
-- first event-backed sync does not erase locally retained history that Tautulli
-- may have already purged.
CREATE TABLE playback_legacy_snapshots (
    source_id TEXT NOT NULL REFERENCES playback_sources(id) ON DELETE CASCADE,
    content_type TEXT NOT NULL CHECK (content_type IN ('movie', 'series', 'artist')),
    content_id TEXT NOT NULL,
    play_count INTEGER NOT NULL CHECK (play_count >= 0),
    play_duration_seconds INTEGER NOT NULL CHECK (play_duration_seconds >= 0),
    last_played_at TEXT,
    covered_until TEXT,
    PRIMARY KEY (source_id, content_type, content_id)
);

INSERT INTO playback_legacy_snapshots (
    source_id, content_type, content_id, play_count,
    play_duration_seconds, last_played_at, covered_until
)
SELECT snapshots.source_id,
       snapshots.content_type,
       snapshots.content_id,
       snapshots.play_count,
       snapshots.play_duration_seconds,
       snapshots.last_played_at,
       sources.last_successful_sync_at
FROM playback_snapshots AS snapshots
JOIN playback_sources AS sources ON sources.id = snapshots.source_id
WHERE NOT EXISTS (
    SELECT 1
    FROM playback_events AS events
    WHERE events.source_id = snapshots.source_id
);
