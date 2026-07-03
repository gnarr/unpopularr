use std::collections::HashSet;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};

use crate::playback::{
    PlaybackRepository, PlaybackSnapshot, PlaybackSource, PlaybackSyncRun, PlaybackSyncStatus,
    PlaybackSyncTrigger,
};

#[derive(Clone)]
pub struct SqlitePlaybackRepository {
    pool: SqlitePool,
}

impl SqlitePlaybackRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PlaybackRepository for SqlitePlaybackRepository {
    async fn reconcile_source(&self, source: Option<&PlaybackSource>) -> Result<()> {
        let mut transaction = self.pool.begin().await?;
        let configured_ids = source
            .map(|source| HashSet::from([source.id.as_str()]))
            .unwrap_or_default();
        let existing_ids = sqlx::query("SELECT id FROM playback_sources")
            .fetch_all(&mut *transaction)
            .await?
            .into_iter()
            .map(|row| row.try_get::<String, _>("id"))
            .collect::<Result<Vec<_>, _>>()?;

        for id in existing_ids {
            if !configured_ids.contains(id.as_str()) {
                sqlx::query("DELETE FROM playback_sources WHERE id = ?")
                    .bind(id)
                    .execute(&mut *transaction)
                    .await?;
            }
        }

        if let Some(source) = source {
            sqlx::query(
                r#"
                INSERT INTO playback_sources (id, provider)
                VALUES (?, ?)
                ON CONFLICT(id) DO UPDATE SET provider = excluded.provider
                "#,
            )
            .bind(&source.id)
            .bind(source.provider.as_str())
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;
        Ok(())
    }

    async fn recover_interrupted_syncs(&self, completed_at: DateTime<Utc>) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE playback_sync_runs
            SET status = 'failed',
                completed_at = ?,
                error = 'sync interrupted by service restart'
            WHERE status = 'running'
            "#,
        )
        .bind(completed_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn create_sync_run(
        &self,
        source: &PlaybackSource,
        trigger: PlaybackSyncTrigger,
        started_at: DateTime<Utc>,
    ) -> Result<PlaybackSyncRun> {
        let result = sqlx::query(
            r#"
            INSERT INTO playback_sync_runs (source_id, trigger, status, started_at)
            VALUES (?, ?, 'running', ?)
            "#,
        )
        .bind(&source.id)
        .bind(trigger.as_str())
        .bind(started_at)
        .execute(&self.pool)
        .await?;

        load_sync_run(&self.pool, result.last_insert_rowid())
            .await?
            .context("created playback sync run was not found")
    }

    async fn store_events(
        &self,
        sync_run_id: i64,
        source: &PlaybackSource,
        snapshot: &PlaybackSnapshot,
        status: PlaybackSyncStatus,
        completed_at: DateTime<Utc>,
    ) -> Result<PlaybackSyncRun> {
        if !matches!(
            status,
            PlaybackSyncStatus::Succeeded | PlaybackSyncStatus::Partial
        ) {
            bail!("successful playback sync requires a completed status");
        }

        let mut transaction = self.pool.begin().await?;

        sqlx::query(
            r#"
            UPDATE playback_legacy_snapshots
            SET covered_until = COALESCE(
                covered_until,
                (
                    SELECT last_successful_sync_at
                    FROM playback_sources
                    WHERE id = playback_legacy_snapshots.source_id
                ),
                ?
            )
            WHERE source_id = ?
            "#,
        )
        .bind(completed_at)
        .bind(&source.id)
        .execute(&mut *transaction)
        .await?;

        // Accumulate individual sessions. Re-syncing a Tautulli row that we have
        // already stored is idempotent, so history survives Tautulli purges.
        for event in &snapshot.events {
            sqlx::query(
                r#"
                INSERT INTO playback_events (
                    source_id, source_row_id, content_type, content_id,
                    played_at, duration_seconds
                )
                VALUES (?, ?, ?, ?, ?, ?)
                ON CONFLICT(source_id, source_row_id) DO UPDATE SET
                    content_type = excluded.content_type,
                    content_id = excluded.content_id,
                    played_at = excluded.played_at,
                    duration_seconds = excluded.duration_seconds
                "#,
            )
            .bind(&source.id)
            .bind(event.source_row_id)
            .bind(event.key.content_type())
            .bind(event.key.content_id())
            .bind(event.played_at)
            .bind(event.duration_seconds)
            .execute(&mut *transaction)
            .await?;
        }

        // Recompute the materialized aggregate that the catalog reads from.
        // Legacy aggregate rows preserve pre-events history, while source events
        // after the legacy cutoff accumulate on every successful sync.
        sqlx::query("DELETE FROM playback_snapshots WHERE source_id = ?")
            .bind(&source.id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            r#"
            WITH legacy AS (
                SELECT source_id, content_type, content_id, play_count,
                       play_duration_seconds, last_played_at, covered_until
                FROM playback_legacy_snapshots
                WHERE source_id = ?
            ),
            event_snapshots AS (
                SELECT events.source_id,
                       events.content_type,
                       events.content_id,
                       COUNT(*) AS play_count,
                       COALESCE(SUM(events.duration_seconds), 0) AS play_duration_seconds,
                       MAX(events.played_at) AS last_played_at
                FROM playback_events AS events
                LEFT JOIN legacy
                    ON legacy.source_id = events.source_id
                   AND legacy.content_type = events.content_type
                   AND legacy.content_id = events.content_id
                WHERE events.source_id = ?
                  AND (
                      legacy.source_id IS NULL
                      OR events.played_at > legacy.covered_until
                  )
                GROUP BY events.source_id, events.content_type, events.content_id
            ),
            combined AS (
                SELECT source_id, content_type, content_id, play_count,
                       play_duration_seconds, last_played_at
                FROM legacy
                UNION ALL
                SELECT source_id, content_type, content_id, play_count,
                       play_duration_seconds, last_played_at
                FROM event_snapshots
            )
            INSERT INTO playback_snapshots (
                source_id, content_type, content_id, play_count,
                play_duration_seconds, last_played_at
            )
            SELECT source_id, content_type, content_id,
                   SUM(play_count),
                   COALESCE(SUM(play_duration_seconds), 0),
                   MAX(last_played_at)
            FROM combined
            GROUP BY source_id, content_type, content_id
            HAVING SUM(play_count) > 0 OR SUM(play_duration_seconds) > 0
            "#,
        )
        .bind(&source.id)
        .bind(&source.id)
        .execute(&mut *transaction)
        .await?;

        sqlx::query("UPDATE playback_sources SET last_successful_sync_at = ? WHERE id = ?")
            .bind(completed_at)
            .bind(&source.id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            r#"
            UPDATE playback_sync_runs
            SET status = ?,
                completed_at = ?,
                matched_history_rows = ?,
                unmatched_history_rows = ?,
                error = NULL
            WHERE id = ?
            "#,
        )
        .bind(status.as_str())
        .bind(completed_at)
        .bind(snapshot.matched_history_rows)
        .bind(snapshot.unmatched_history_rows)
        .bind(sync_run_id)
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;
        load_sync_run(&self.pool, sync_run_id)
            .await?
            .context("completed playback sync run was not found")
    }

    async fn mark_failed(
        &self,
        sync_run_id: i64,
        matched_history_rows: i64,
        unmatched_history_rows: i64,
        error: &str,
        completed_at: DateTime<Utc>,
    ) -> Result<PlaybackSyncRun> {
        sqlx::query(
            r#"
            UPDATE playback_sync_runs
            SET status = 'failed',
                completed_at = ?,
                matched_history_rows = ?,
                unmatched_history_rows = ?,
                error = ?
            WHERE id = ?
            "#,
        )
        .bind(completed_at)
        .bind(matched_history_rows)
        .bind(unmatched_history_rows)
        .bind(error)
        .bind(sync_run_id)
        .execute(&self.pool)
        .await?;

        load_sync_run(&self.pool, sync_run_id)
            .await?
            .context("failed playback sync run was not found")
    }

    async fn active_or_latest_sync(&self) -> Result<Option<PlaybackSyncRun>> {
        let id = sqlx::query(
            r#"
            SELECT id
            FROM playback_sync_runs
            ORDER BY (status = 'running') DESC, id DESC
            LIMIT 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await?
        .map(|row| row.try_get::<i64, _>("id"))
        .transpose()?;

        match id {
            Some(id) => load_sync_run(&self.pool, id).await,
            None => Ok(None),
        }
    }
}

async fn load_sync_run(pool: &SqlitePool, id: i64) -> Result<Option<PlaybackSyncRun>> {
    let Some(row) = sqlx::query(
        r#"
        SELECT id, source_id, trigger, status, started_at, completed_at,
               matched_history_rows, unmatched_history_rows, error
        FROM playback_sync_runs
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    Ok(Some(PlaybackSyncRun {
        id: row.try_get("id")?,
        source_id: row.try_get("source_id")?,
        trigger: parse_trigger(&row.try_get::<String, _>("trigger")?)?,
        status: parse_status(&row.try_get::<String, _>("status")?)?,
        started_at: row.try_get("started_at")?,
        completed_at: row.try_get("completed_at")?,
        matched_history_rows: row.try_get("matched_history_rows")?,
        unmatched_history_rows: row.try_get("unmatched_history_rows")?,
        error: row.try_get("error")?,
    }))
}

fn parse_trigger(value: &str) -> Result<PlaybackSyncTrigger> {
    match value {
        "startup" => Ok(PlaybackSyncTrigger::Startup),
        "scheduled" => Ok(PlaybackSyncTrigger::Scheduled),
        "manual" => Ok(PlaybackSyncTrigger::Manual),
        _ => bail!("unknown playback sync trigger in database: {value}"),
    }
}

fn parse_status(value: &str) -> Result<PlaybackSyncStatus> {
    match value {
        "running" => Ok(PlaybackSyncStatus::Running),
        "succeeded" => Ok(PlaybackSyncStatus::Succeeded),
        "partial" => Ok(PlaybackSyncStatus::Partial),
        "failed" => Ok(PlaybackSyncStatus::Failed),
        _ => bail!("unknown playback sync status in database: {value}"),
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use url::Url;

    use crate::{
        database,
        playback::{
            ContentKey, PlaybackEvent, PlaybackRepository, PlaybackSnapshot, PlaybackSource,
            PlaybackSyncStatus, PlaybackSyncTrigger,
        },
    };

    use super::SqlitePlaybackRepository;

    fn source(id: &str) -> PlaybackSource {
        PlaybackSource {
            id: id.to_owned(),
            provider: crate::playback::PlaybackProvider::Tautulli,
            base_url: Url::parse("http://localhost/").expect("URL"),
            api_key: "secret".to_owned(),
        }
    }

    fn event(
        source_row_id: i64,
        key: ContentKey,
        played_at_secs: i64,
        duration_seconds: i64,
    ) -> PlaybackEvent {
        PlaybackEvent {
            key,
            source_row_id,
            played_at: DateTime::from_timestamp(played_at_secs, 0).expect("timestamp"),
            duration_seconds,
        }
    }

    fn snapshot(events: Vec<PlaybackEvent>) -> PlaybackSnapshot {
        PlaybackSnapshot {
            matched_history_rows: events.len() as i64,
            unmatched_history_rows: 0,
            events,
        }
    }

    #[tokio::test]
    async fn accumulates_events_and_removes_deleted_sources() {
        let pool = database::test_pool().await;
        let repository = SqlitePlaybackRepository::new(pool.clone());
        let source = source("plex");
        repository
            .reconcile_source(Some(&source))
            .await
            .expect("reconcile");
        let run = repository
            .create_sync_run(&source, PlaybackSyncTrigger::Manual, Utc::now())
            .await
            .expect("create run");
        let run = repository
            .store_events(
                run.id,
                &source,
                &snapshot(vec![
                    event(1, ContentKey::Movie(1), 100, 120),
                    event(2, ContentKey::Movie(1), 200, 60),
                ]),
                PlaybackSyncStatus::Succeeded,
                Utc::now(),
            )
            .await
            .expect("store");
        assert_eq!(run.status, PlaybackSyncStatus::Succeeded);

        let event_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playback_events")
            .fetch_one(&pool)
            .await
            .expect("count events");
        assert_eq!(event_count, 2);

        // The materialized snapshot aggregates both sessions of the movie.
        let (play_count, duration): (i64, i64) = sqlx::query_as(
            "SELECT play_count, play_duration_seconds FROM playback_snapshots \
             WHERE content_type = 'movie' AND content_id = '1'",
        )
        .fetch_one(&pool)
        .await
        .expect("snapshot row");
        assert_eq!(play_count, 2);
        assert_eq!(duration, 180);

        // Removing the source cascades both the events and the derived snapshot.
        repository
            .reconcile_source(None)
            .await
            .expect("remove source");
        let event_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playback_events")
            .fetch_one(&pool)
            .await
            .expect("count events");
        assert_eq!(event_count, 0);
        let snapshot_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playback_snapshots")
            .fetch_one(&pool)
            .await
            .expect("count snapshots");
        assert_eq!(snapshot_count, 0);
    }

    #[tokio::test]
    async fn re_syncing_the_same_row_is_idempotent_and_accumulates_new_rows() {
        let pool = database::test_pool().await;
        let repository = SqlitePlaybackRepository::new(pool.clone());
        let source = source("plex");
        repository
            .reconcile_source(Some(&source))
            .await
            .expect("reconcile");

        let first = repository
            .create_sync_run(&source, PlaybackSyncTrigger::Manual, Utc::now())
            .await
            .expect("first run");
        repository
            .store_events(
                first.id,
                &source,
                &snapshot(vec![
                    event(1, ContentKey::Movie(1), 100, 120),
                    event(2, ContentKey::Movie(1), 200, 60),
                    event(3, ContentKey::Series(2), 300, 30),
                ]),
                PlaybackSyncStatus::Succeeded,
                Utc::now(),
            )
            .await
            .expect("first store");

        // Second sync re-sends row 1 with an updated duration and adds row 4.
        // Rows 2 and 3 are absent (purged from Tautulli) but must be retained.
        let second = repository
            .create_sync_run(&source, PlaybackSyncTrigger::Manual, Utc::now())
            .await
            .expect("second run");
        repository
            .store_events(
                second.id,
                &source,
                &snapshot(vec![
                    event(1, ContentKey::Movie(1), 100, 999),
                    event(4, ContentKey::Series(2), 400, 45),
                ]),
                PlaybackSyncStatus::Succeeded,
                Utc::now(),
            )
            .await
            .expect("second store");

        let event_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playback_events")
            .fetch_one(&pool)
            .await
            .expect("count events");
        assert_eq!(event_count, 4);

        // Row 1 was updated in place, not duplicated.
        let duration: i64 = sqlx::query_scalar(
            "SELECT duration_seconds FROM playback_events WHERE source_row_id = 1",
        )
        .fetch_one(&pool)
        .await
        .expect("row 1");
        assert_eq!(duration, 999);

        // The recomputed snapshot reflects every accumulated session.
        let (movie_count, movie_duration): (i64, i64) = sqlx::query_as(
            "SELECT play_count, play_duration_seconds FROM playback_snapshots \
             WHERE content_type = 'movie' AND content_id = '1'",
        )
        .fetch_one(&pool)
        .await
        .expect("movie snapshot");
        assert_eq!(movie_count, 2); // rows 1 and 2
        assert_eq!(movie_duration, 1059); // 999 + 60

        let last_played: Option<DateTime<Utc>> = sqlx::query_scalar(
            "SELECT last_played_at FROM playback_snapshots \
             WHERE content_type = 'series' AND content_id = '2'",
        )
        .fetch_one(&pool)
        .await
        .expect("series snapshot");
        assert_eq!(last_played.map(|played| played.timestamp()), Some(400));
    }

    #[tokio::test]
    async fn first_event_sync_preserves_legacy_aggregate_without_double_counting() {
        let pool = database::test_pool().await;
        let repository = SqlitePlaybackRepository::new(pool.clone());
        let source = source("plex");
        let cutoff = DateTime::from_timestamp(250, 0).expect("timestamp");
        let legacy_last_played = DateTime::from_timestamp(200, 0).expect("timestamp");

        repository
            .reconcile_source(Some(&source))
            .await
            .expect("reconcile");
        sqlx::query("UPDATE playback_sources SET last_successful_sync_at = ? WHERE id = ?")
            .bind(cutoff)
            .bind(&source.id)
            .execute(&pool)
            .await
            .expect("mark previous sync");
        sqlx::query(
            r#"
            INSERT INTO playback_snapshots (
                source_id, content_type, content_id, play_count,
                play_duration_seconds, last_played_at
            )
            VALUES (?, 'movie', '1', 2, 180, ?)
            "#,
        )
        .bind(&source.id)
        .bind(legacy_last_played)
        .execute(&pool)
        .await
        .expect("seed legacy snapshot");
        sqlx::query(
            r#"
            INSERT INTO playback_legacy_snapshots (
                source_id, content_type, content_id, play_count,
                play_duration_seconds, last_played_at, covered_until
            )
            VALUES (?, 'movie', '1', 2, 180, ?, ?)
            "#,
        )
        .bind(&source.id)
        .bind(legacy_last_played)
        .bind(cutoff)
        .execute(&pool)
        .await
        .expect("seed legacy baseline");

        let run = repository
            .create_sync_run(&source, PlaybackSyncTrigger::Manual, Utc::now())
            .await
            .expect("create run");
        repository
            .store_events(
                run.id,
                &source,
                &snapshot(vec![
                    event(1, ContentKey::Movie(1), 100, 120),
                    event(2, ContentKey::Movie(1), 300, 60),
                ]),
                PlaybackSyncStatus::Succeeded,
                Utc::now(),
            )
            .await
            .expect("store");

        let event_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playback_events")
            .fetch_one(&pool)
            .await
            .expect("count events");
        assert_eq!(event_count, 2);

        let (play_count, duration, last_played): (i64, i64, Option<DateTime<Utc>>) =
            sqlx::query_as(
                "SELECT play_count, play_duration_seconds, last_played_at \
                 FROM playback_snapshots \
                 WHERE content_type = 'movie' AND content_id = '1'",
            )
            .fetch_one(&pool)
            .await
            .expect("snapshot row");
        assert_eq!(play_count, 3);
        assert_eq!(duration, 240);
        assert_eq!(last_played.map(|played| played.timestamp()), Some(300));
    }

    #[tokio::test]
    async fn failed_sync_preserves_previous_events() {
        let pool = database::test_pool().await;
        let repository = SqlitePlaybackRepository::new(pool.clone());
        let source = source("plex");
        repository
            .reconcile_source(Some(&source))
            .await
            .expect("reconcile");
        let successful = repository
            .create_sync_run(&source, PlaybackSyncTrigger::Manual, Utc::now())
            .await
            .expect("create successful run");
        repository
            .store_events(
                successful.id,
                &source,
                &snapshot(vec![event(1, ContentKey::Series(2), 100, 60)]),
                PlaybackSyncStatus::Succeeded,
                Utc::now(),
            )
            .await
            .expect("store");

        let failed = repository
            .create_sync_run(&source, PlaybackSyncTrigger::Manual, Utc::now())
            .await
            .expect("create failed run");
        repository
            .mark_failed(failed.id, 0, 1, "failed", Utc::now())
            .await
            .expect("mark failed");

        let event_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playback_events")
            .fetch_one(&pool)
            .await
            .expect("count events");
        assert_eq!(event_count, 1);
        let snapshot_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playback_snapshots")
            .fetch_one(&pool)
            .await
            .expect("count snapshots");
        assert_eq!(snapshot_count, 1);
    }

    #[tokio::test]
    async fn recovers_interrupted_runs_as_failed() {
        let pool = database::test_pool().await;
        let repository = SqlitePlaybackRepository::new(pool);
        let source = source("plex");
        repository
            .reconcile_source(Some(&source))
            .await
            .expect("reconcile");
        repository
            .create_sync_run(&source, PlaybackSyncTrigger::Startup, Utc::now())
            .await
            .expect("create run");

        repository
            .recover_interrupted_syncs(Utc::now())
            .await
            .expect("recover");
        let run = repository
            .active_or_latest_sync()
            .await
            .expect("load run")
            .expect("run");
        assert_eq!(run.status, PlaybackSyncStatus::Failed);
        assert_eq!(
            run.error.as_deref(),
            Some("sync interrupted by service restart")
        );
    }
}
