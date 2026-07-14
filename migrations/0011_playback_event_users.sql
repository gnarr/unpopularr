-- Watching user of a playback event (Tautulli user id plus display name).
-- Nullable: rows stored before this migration are backfilled by the next
-- playback sync, which re-fetches the full history and upserts every row.
-- Rows Tautulli has since purged stay NULL and are reported on detail pages
-- as unattributed plays.
ALTER TABLE playback_events ADD COLUMN user_id INTEGER;
ALTER TABLE playback_events ADD COLUMN user_name TEXT;
