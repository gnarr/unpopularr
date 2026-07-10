-- Album title and size for the artist details page. Existing rows get
-- placeholder values ('' / 0); each Lidarr sync replaces album snapshots
-- wholesale (via the artist_snapshots cascade), so real values arrive with
-- the next sync.
ALTER TABLE artist_album_snapshots ADD COLUMN title TEXT NOT NULL DEFAULT '';
ALTER TABLE artist_album_snapshots
    ADD COLUMN size_on_disk_bytes INTEGER NOT NULL DEFAULT 0
    CHECK (size_on_disk_bytes >= 0);

-- Serves the artist details read: all instances' albums for one artist
-- (the PK leads with instance_id and cannot help there).
CREATE INDEX idx_artist_album_snapshots_artist_musicbrainz_id
    ON artist_album_snapshots(artist_musicbrainz_id);
