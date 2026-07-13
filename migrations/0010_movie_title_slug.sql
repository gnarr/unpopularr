-- Radarr's title slug, used to build "open in Radarr" deep links
-- ({external_url}/movie/{titleSlug}, e.g. movie/inception-27205 — the bare TMDB
-- id does not match Radarr's route). Existing rows get a placeholder ('');
-- each Radarr sync replaces movie snapshots wholesale, so real slugs arrive
-- with the next sync and links stay hidden until then.
ALTER TABLE movie_snapshots ADD COLUMN title_slug TEXT NOT NULL DEFAULT '';
