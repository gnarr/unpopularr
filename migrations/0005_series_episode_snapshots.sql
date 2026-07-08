-- Per-episode snapshots collected from Sonarr. Replaced wholesale on each
-- successful instance sync via the ON DELETE CASCADE from series_snapshots
-- (the collection adapter deletes series_snapshots per instance).
-- Season 0 (specials) is excluded, matching series_season_snapshots.
CREATE TABLE series_episode_snapshots (
    instance_id TEXT NOT NULL,
    tvdb_id INTEGER NOT NULL,
    season_number INTEGER NOT NULL CHECK (season_number > 0),
    episode_number INTEGER NOT NULL CHECK (episode_number >= 0),
    title TEXT NOT NULL,
    air_date_utc TEXT,
    has_file INTEGER NOT NULL CHECK (has_file IN (0, 1)),
    size_on_disk_bytes INTEGER NOT NULL CHECK (size_on_disk_bytes >= 0),
    PRIMARY KEY (instance_id, tvdb_id, season_number, episode_number),
    FOREIGN KEY (instance_id, tvdb_id)
        REFERENCES series_snapshots(instance_id, tvdb_id)
        ON DELETE CASCADE
);

-- Serves the series details read: all instances' episodes for one series.
CREATE INDEX idx_series_episode_snapshots_tvdb_id
    ON series_episode_snapshots(tvdb_id);
