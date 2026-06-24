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

    async fn store_snapshot(
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
            bail!("successful playback snapshot requires a completed status");
        }

        let mut transaction = self.pool.begin().await?;
        sqlx::query("DELETE FROM playback_snapshots WHERE source_id = ?")
            .bind(&source.id)
            .execute(&mut *transaction)
            .await?;

        for aggregate in &snapshot.aggregates {
            sqlx::query(
                r#"
                INSERT INTO playback_snapshots (
                    source_id, content_type, content_id, play_count,
                    play_duration_seconds, last_played_at
                )
                VALUES (?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&source.id)
            .bind(aggregate.key.content_type())
            .bind(aggregate.key.content_id())
            .bind(aggregate.play_count)
            .bind(aggregate.play_duration_seconds)
            .bind(aggregate.last_played_at)
            .execute(&mut *transaction)
            .await?;
        }

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
    use chrono::Utc;
    use url::Url;

    use crate::{
        database,
        playback::{
            ContentKey, PlaybackAggregate, PlaybackRepository, PlaybackSnapshot, PlaybackSource,
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

    #[tokio::test]
    async fn replaces_snapshots_atomically_and_removes_deleted_sources() {
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
        let snapshot = PlaybackSnapshot {
            aggregates: vec![PlaybackAggregate {
                key: ContentKey::Movie(1),
                play_count: 2,
                play_duration_seconds: 300,
                last_played_at: Some(Utc::now()),
            }],
            matched_history_rows: 1,
            unmatched_history_rows: 0,
        };
        let run = repository
            .store_snapshot(
                run.id,
                &source,
                &snapshot,
                PlaybackSyncStatus::Succeeded,
                Utc::now(),
            )
            .await
            .expect("store");
        assert_eq!(run.status, PlaybackSyncStatus::Succeeded);

        repository
            .reconcile_source(None)
            .await
            .expect("remove source");
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playback_snapshots")
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn failed_sync_preserves_previous_snapshot() {
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
            .store_snapshot(
                successful.id,
                &source,
                &PlaybackSnapshot {
                    aggregates: vec![PlaybackAggregate {
                        key: ContentKey::Series(2),
                        play_count: 1,
                        play_duration_seconds: 60,
                        last_played_at: None,
                    }],
                    matched_history_rows: 1,
                    unmatched_history_rows: 0,
                },
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

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playback_snapshots")
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(count, 1);
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
