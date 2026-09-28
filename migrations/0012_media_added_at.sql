-- Acquisition date per snapshot row: the oldest hint the instance reports for
-- the item. Sonarr's `series.added` folded with every episode file's
-- `dateAdded`, Lidarr's `artist.added` folded with its albums'. Existing rows
-- stay NULL until the next sync, which replaces snapshots wholesale.
--
-- `movie_snapshots.added_at` already exists (migration 0008); only its meaning
-- widens, from "Radarr added the movie" to "the oldest acquisition hint".
--
-- Every value here is re-derived from the source on each sync, so the
-- delete-then-insert write path cannot leave a stale date behind. Anything
-- folded in that the source cannot re-supply -- the optional filesystem probe's
-- file timestamps -- is recomputed on the same schedule for the same reason.
ALTER TABLE series_snapshots ADD COLUMN added_at TEXT;
ALTER TABLE series_episode_snapshots ADD COLUMN added_at TEXT;
ALTER TABLE artist_snapshots ADD COLUMN added_at TEXT;
ALTER TABLE artist_album_snapshots ADD COLUMN added_at TEXT;
