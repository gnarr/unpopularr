CREATE TABLE instances (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('sonarr', 'radarr', 'lidarr')),
    config_order INTEGER NOT NULL,
    last_successful_sync_at TEXT
);

CREATE TABLE sync_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    trigger TEXT NOT NULL CHECK (trigger IN ('startup', 'scheduled', 'manual')),
    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'partial', 'failed')),
    started_at TEXT NOT NULL,
    completed_at TEXT,
    imported_items INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE sync_instance_results (
    sync_run_id INTEGER NOT NULL REFERENCES sync_runs(id) ON DELETE CASCADE,
    instance_id TEXT NOT NULL,
    instance_name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('sonarr', 'radarr', 'lidarr')),
    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed')),
    imported_items INTEGER NOT NULL DEFAULT 0,
    error TEXT,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    PRIMARY KEY (sync_run_id, instance_id)
);

CREATE TABLE movie_snapshots (
    instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    tmdb_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    year INTEGER NOT NULL,
    size_on_disk_bytes INTEGER NOT NULL CHECK (size_on_disk_bytes >= 0),
    file_count INTEGER NOT NULL CHECK (file_count >= 0),
    PRIMARY KEY (instance_id, tmdb_id)
);

CREATE TABLE series_snapshots (
    instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    tvdb_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    year INTEGER NOT NULL,
    size_on_disk_bytes INTEGER NOT NULL CHECK (size_on_disk_bytes >= 0),
    file_count INTEGER NOT NULL CHECK (file_count >= 0),
    PRIMARY KEY (instance_id, tvdb_id)
);

CREATE TABLE series_season_snapshots (
    instance_id TEXT NOT NULL,
    tvdb_id INTEGER NOT NULL,
    season_number INTEGER NOT NULL CHECK (season_number > 0),
    file_count INTEGER NOT NULL CHECK (file_count > 0),
    PRIMARY KEY (instance_id, tvdb_id, season_number),
    FOREIGN KEY (instance_id, tvdb_id)
        REFERENCES series_snapshots(instance_id, tvdb_id)
        ON DELETE CASCADE
);

CREATE TABLE artist_snapshots (
    instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    musicbrainz_id TEXT NOT NULL,
    name TEXT NOT NULL,
    size_on_disk_bytes INTEGER NOT NULL CHECK (size_on_disk_bytes >= 0),
    file_count INTEGER NOT NULL CHECK (file_count >= 0),
    PRIMARY KEY (instance_id, musicbrainz_id)
);

CREATE TABLE artist_album_snapshots (
    instance_id TEXT NOT NULL,
    artist_musicbrainz_id TEXT NOT NULL,
    album_musicbrainz_id TEXT NOT NULL,
    file_count INTEGER NOT NULL CHECK (file_count > 0),
    PRIMARY KEY (instance_id, artist_musicbrainz_id, album_musicbrainz_id),
    FOREIGN KEY (instance_id, artist_musicbrainz_id)
        REFERENCES artist_snapshots(instance_id, musicbrainz_id)
        ON DELETE CASCADE
);

CREATE INDEX idx_movie_snapshots_tmdb_id ON movie_snapshots(tmdb_id);
CREATE INDEX idx_series_snapshots_tvdb_id ON series_snapshots(tvdb_id);
CREATE INDEX idx_artist_snapshots_musicbrainz_id ON artist_snapshots(musicbrainz_id);
CREATE INDEX idx_sync_runs_started_at ON sync_runs(started_at DESC);

