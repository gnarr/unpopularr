-- Season/episode position of series playback events, for the per-episode
-- watch matrix. Nullable: only meaningful for episode rows (null for movies
-- and tracks). Rows stored before this migration are backfilled by the next
-- playback sync, which re-fetches the full history and upserts every row.
ALTER TABLE playback_events ADD COLUMN season_number INTEGER;
ALTER TABLE playback_events ADD COLUMN episode_number INTEGER;

-- Serves the per-series episode aggregation, which reads across all sources
-- (idx_playback_events_content leads with source_id and cannot help there).
CREATE INDEX idx_playback_events_content_item
    ON playback_events(content_type, content_id);
