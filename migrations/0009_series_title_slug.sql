-- Sonarr's title slug, used to build "open in Sonarr" deep links
-- ({external_url}/series/{titleSlug}). Existing rows get a placeholder ('');
-- each Sonarr sync replaces series snapshots wholesale, so real slugs arrive
-- with the next sync and links stay hidden until then.
ALTER TABLE series_snapshots ADD COLUMN title_slug TEXT NOT NULL DEFAULT '';
