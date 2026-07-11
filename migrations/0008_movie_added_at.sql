-- When Radarr first added the movie to its library, used as the left edge of
-- the movie details "minutes played per month" plot. Nullable: existing rows
-- (and any Radarr response lacking the field) stay NULL until the next sync,
-- which replaces movie snapshots wholesale.
ALTER TABLE movie_snapshots ADD COLUMN added_at TEXT;
