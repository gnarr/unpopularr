use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};

use crate::{
    collection::{
        CollectionRepository, InstanceSyncResult, Snapshot, SyncRun, SyncStatus, SyncTrigger,
        application::final_status,
    },
    instances::{Instance, InstanceKind},
};

#[derive(Clone)]
pub struct SqliteCollectionRepository {
    pool: SqlitePool,
}

impl SqliteCollectionRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CollectionRepository for SqliteCollectionRepository {
    async fn reconcile_instances(&self, instances: &[Instance]) -> Result<()> {
        let mut transaction = self.pool.begin().await?;
        let existing = sqlx::query("SELECT id, kind FROM instances")
            .fetch_all(&mut *transaction)
            .await?
            .into_iter()
            .map(|row| {
                Ok((
                    row.try_get::<String, _>("id")?,
                    row.try_get::<String, _>("kind")?,
                ))
            })
            .collect::<Result<HashMap<_, _>, sqlx::Error>>()?;
        let configured_ids = instances
            .iter()
            .map(|instance| instance.id.as_str())
            .collect::<HashSet<_>>();

        for (id, kind) in &existing {
            if !configured_ids.contains(id.as_str())
                || instances
                    .iter()
                    .any(|instance| instance.id == *id && instance.kind.as_str() != kind.as_str())
            {
                sqlx::query("DELETE FROM instances WHERE id = ?")
                    .bind(id)
                    .execute(&mut *transaction)
                    .await?;
            }
        }

        for instance in instances {
            sqlx::query(
                r#"
                INSERT INTO instances (id, name, kind, config_order)
                VALUES (?, ?, ?, ?)
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    kind = excluded.kind,
                    config_order = excluded.config_order
                "#,
            )
            .bind(&instance.id)
            .bind(&instance.name)
            .bind(instance.kind.as_str())
            .bind(instance.config_order)
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;
        Ok(())
    }

    async fn recover_interrupted_syncs(&self, completed_at: DateTime<Utc>) -> Result<()> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            r#"
            UPDATE sync_instance_results
            SET status = 'failed',
                error = 'sync interrupted by service restart',
                completed_at = ?
            WHERE status = 'running'
            "#,
        )
        .bind(completed_at)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            r#"
            UPDATE sync_runs
            SET status = 'failed', completed_at = ?
            WHERE status = 'running'
            "#,
        )
        .bind(completed_at)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    async fn create_sync_run(
        &self,
        trigger: SyncTrigger,
        instances: &[Instance],
        started_at: DateTime<Utc>,
    ) -> Result<SyncRun> {
        let mut transaction = self.pool.begin().await?;
        let result = sqlx::query(
            r#"
            INSERT INTO sync_runs (trigger, status, started_at)
            VALUES (?, 'running', ?)
            "#,
        )
        .bind(trigger.as_str())
        .bind(started_at)
        .execute(&mut *transaction)
        .await?;
        let run_id = result.last_insert_rowid();

        for instance in instances {
            sqlx::query(
                r#"
                INSERT INTO sync_instance_results (
                    sync_run_id, instance_id, instance_name, kind, status, started_at
                )
                VALUES (?, ?, ?, ?, 'running', ?)
                "#,
            )
            .bind(run_id)
            .bind(&instance.id)
            .bind(&instance.name)
            .bind(instance.kind.as_str())
            .bind(started_at)
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;
        load_sync_run(&self.pool, run_id)
            .await?
            .context("created sync run was not found")
    }

    async fn store_successful_snapshot(
        &self,
        sync_run_id: i64,
        instance: &Instance,
        snapshot: &Snapshot,
        completed_at: DateTime<Utc>,
    ) -> Result<()> {
        ensure_snapshot_kind(instance.kind, snapshot)?;
        let imported_items =
            i64::try_from(snapshot.item_count()).context("snapshot item count exceeds i64")?;
        let mut transaction = self.pool.begin().await?;

        match snapshot {
            Snapshot::Movies(movies) => {
                sqlx::query("DELETE FROM movie_snapshots WHERE instance_id = ?")
                    .bind(&instance.id)
                    .execute(&mut *transaction)
                    .await?;
                for movie in movies {
                    sqlx::query(
                        r#"
                        INSERT INTO movie_snapshots (
                            instance_id, tmdb_id, title, title_slug, year,
                            size_on_disk_bytes, file_count, added_at
                        )
                        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                        "#,
                    )
                    .bind(&instance.id)
                    .bind(movie.tmdb_id)
                    .bind(&movie.title)
                    .bind(&movie.title_slug)
                    .bind(movie.year)
                    .bind(movie.size_on_disk_bytes)
                    .bind(movie.file_count)
                    .bind(movie.added_at)
                    .execute(&mut *transaction)
                    .await?;
                }
            }
            Snapshot::Series(series_items) => {
                sqlx::query("DELETE FROM series_snapshots WHERE instance_id = ?")
                    .bind(&instance.id)
                    .execute(&mut *transaction)
                    .await?;
                for series in series_items {
                    sqlx::query(
                        r#"
                        INSERT INTO series_snapshots (
                            instance_id, tvdb_id, title, title_slug, year,
                            size_on_disk_bytes, file_count
                        )
                        VALUES (?, ?, ?, ?, ?, ?, ?)
                        "#,
                    )
                    .bind(&instance.id)
                    .bind(series.tvdb_id)
                    .bind(&series.title)
                    .bind(&series.title_slug)
                    .bind(series.year)
                    .bind(series.size_on_disk_bytes)
                    .bind(series.file_count)
                    .execute(&mut *transaction)
                    .await?;

                    for season in &series.seasons {
                        sqlx::query(
                            r#"
                            INSERT INTO series_season_snapshots (
                                instance_id, tvdb_id, season_number, file_count
                            )
                            VALUES (?, ?, ?, ?)
                            "#,
                        )
                        .bind(&instance.id)
                        .bind(series.tvdb_id)
                        .bind(season.season_number)
                        .bind(season.file_count)
                        .execute(&mut *transaction)
                        .await?;
                    }

                    for episode in &series.episodes {
                        sqlx::query(
                            r#"
                            INSERT INTO series_episode_snapshots (
                                instance_id, tvdb_id, season_number, episode_number,
                                title, air_date_utc, has_file, size_on_disk_bytes
                            )
                            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                            "#,
                        )
                        .bind(&instance.id)
                        .bind(series.tvdb_id)
                        .bind(episode.season_number)
                        .bind(episode.episode_number)
                        .bind(&episode.title)
                        .bind(episode.air_date_utc)
                        .bind(episode.has_file)
                        .bind(episode.size_on_disk_bytes)
                        .execute(&mut *transaction)
                        .await?;
                    }
                }
            }
            Snapshot::Artists(artists) => {
                sqlx::query("DELETE FROM artist_snapshots WHERE instance_id = ?")
                    .bind(&instance.id)
                    .execute(&mut *transaction)
                    .await?;
                for artist in artists {
                    sqlx::query(
                        r#"
                        INSERT INTO artist_snapshots (
                            instance_id, musicbrainz_id, name, size_on_disk_bytes, file_count
                        )
                        VALUES (?, ?, ?, ?, ?)
                        "#,
                    )
                    .bind(&instance.id)
                    .bind(&artist.musicbrainz_id)
                    .bind(&artist.name)
                    .bind(artist.size_on_disk_bytes)
                    .bind(artist.file_count)
                    .execute(&mut *transaction)
                    .await?;

                    for album in &artist.albums {
                        sqlx::query(
                            r#"
                            INSERT INTO artist_album_snapshots (
                                instance_id, artist_musicbrainz_id, album_musicbrainz_id,
                                title, size_on_disk_bytes, file_count
                            )
                            VALUES (?, ?, ?, ?, ?, ?)
                            "#,
                        )
                        .bind(&instance.id)
                        .bind(&artist.musicbrainz_id)
                        .bind(&album.musicbrainz_id)
                        .bind(&album.title)
                        .bind(album.size_on_disk_bytes)
                        .bind(album.file_count)
                        .execute(&mut *transaction)
                        .await?;
                    }
                }
            }
        }

        sqlx::query("UPDATE instances SET last_successful_sync_at = ? WHERE id = ?")
            .bind(completed_at)
            .bind(&instance.id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            r#"
            UPDATE sync_instance_results
            SET status = 'succeeded',
                imported_items = ?,
                error = NULL,
                completed_at = ?
            WHERE sync_run_id = ? AND instance_id = ?
            "#,
        )
        .bind(imported_items)
        .bind(completed_at)
        .bind(sync_run_id)
        .bind(&instance.id)
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;
        Ok(())
    }

    async fn mark_instance_failed(
        &self,
        sync_run_id: i64,
        instance_id: &str,
        error: &str,
        completed_at: DateTime<Utc>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE sync_instance_results
            SET status = 'failed', error = ?, completed_at = ?
            WHERE sync_run_id = ? AND instance_id = ?
            "#,
        )
        .bind(error)
        .bind(completed_at)
        .bind(sync_run_id)
        .bind(instance_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn finish_sync_run(
        &self,
        sync_run_id: i64,
        completed_at: DateTime<Utc>,
    ) -> Result<SyncRun> {
        let rows = sqlx::query(
            r#"
            SELECT status, COUNT(*) AS count, COALESCE(SUM(imported_items), 0) AS imported_items
            FROM sync_instance_results
            WHERE sync_run_id = ?
            GROUP BY status
            "#,
        )
        .bind(sync_run_id)
        .fetch_all(&self.pool)
        .await?;

        let mut successful = 0_usize;
        let mut failed = 0_usize;
        let mut imported_items = 0_i64;
        for row in rows {
            let count = usize::try_from(row.try_get::<i64, _>("count")?)
                .context("invalid sync result count")?;
            imported_items += row.try_get::<i64, _>("imported_items")?;
            match row.try_get::<String, _>("status")?.as_str() {
                "succeeded" => successful += count,
                "failed" | "running" => failed += count,
                status => bail!("unknown instance sync status in database: {status}"),
            }
        }
        let status = final_status(successful, failed);

        sqlx::query(
            r#"
            UPDATE sync_runs
            SET status = ?, completed_at = ?, imported_items = ?
            WHERE id = ?
            "#,
        )
        .bind(status.as_str())
        .bind(completed_at)
        .bind(imported_items)
        .bind(sync_run_id)
        .execute(&self.pool)
        .await?;

        load_sync_run(&self.pool, sync_run_id)
            .await?
            .context("finished sync run was not found")
    }

    async fn active_or_latest_sync(&self) -> Result<Option<SyncRun>> {
        let id = sqlx::query(
            r#"
            SELECT id
            FROM sync_runs
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

fn ensure_snapshot_kind(kind: InstanceKind, snapshot: &Snapshot) -> Result<()> {
    let matches = matches!(
        (kind, snapshot),
        (InstanceKind::Radarr, Snapshot::Movies(_))
            | (InstanceKind::Sonarr, Snapshot::Series(_))
            | (InstanceKind::Lidarr, Snapshot::Artists(_))
    );
    if !matches {
        bail!("snapshot type does not match instance kind {kind}");
    }
    Ok(())
}

async fn load_sync_run(pool: &SqlitePool, id: i64) -> Result<Option<SyncRun>> {
    let Some(row) = sqlx::query(
        r#"
        SELECT id, trigger, status, started_at, completed_at, imported_items
        FROM sync_runs
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    let result_rows = sqlx::query(
        r#"
        SELECT instance_id, instance_name, kind, status, imported_items, error,
               started_at, completed_at
        FROM sync_instance_results
        WHERE sync_run_id = ?
        ORDER BY instance_id
        "#,
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    let instances = result_rows
        .into_iter()
        .map(|row| {
            Ok(InstanceSyncResult {
                id: row.try_get("instance_id")?,
                name: row.try_get("instance_name")?,
                kind: parse_kind(&row.try_get::<String, _>("kind")?)?,
                status: parse_status(&row.try_get::<String, _>("status")?)?,
                imported_items: row.try_get("imported_items")?,
                error: row.try_get("error")?,
                started_at: row.try_get("started_at")?,
                completed_at: row.try_get("completed_at")?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(Some(SyncRun {
        id: row.try_get("id")?,
        trigger: parse_trigger(&row.try_get::<String, _>("trigger")?)?,
        status: parse_status(&row.try_get::<String, _>("status")?)?,
        started_at: row.try_get("started_at")?,
        completed_at: row.try_get("completed_at")?,
        imported_items: row.try_get("imported_items")?,
        instances,
    }))
}

fn parse_kind(value: &str) -> Result<InstanceKind> {
    match value {
        "sonarr" => Ok(InstanceKind::Sonarr),
        "radarr" => Ok(InstanceKind::Radarr),
        "lidarr" => Ok(InstanceKind::Lidarr),
        _ => bail!("unknown instance kind in database: {value}"),
    }
}

fn parse_trigger(value: &str) -> Result<SyncTrigger> {
    match value {
        "startup" => Ok(SyncTrigger::Startup),
        "scheduled" => Ok(SyncTrigger::Scheduled),
        "manual" => Ok(SyncTrigger::Manual),
        _ => bail!("unknown sync trigger in database: {value}"),
    }
}

fn parse_status(value: &str) -> Result<SyncStatus> {
    match value {
        "running" => Ok(SyncStatus::Running),
        "succeeded" => Ok(SyncStatus::Succeeded),
        "partial" => Ok(SyncStatus::Partial),
        "failed" => Ok(SyncStatus::Failed),
        _ => bail!("unknown sync status in database: {value}"),
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use url::Url;

    use crate::{
        collection::{
            ArtistAlbumSnapshot, ArtistSnapshot, CollectionRepository, MovieSnapshot,
            SeriesEpisodeSnapshot, SeriesSeasonSnapshot, SeriesSnapshot, Snapshot, SyncStatus,
            SyncTrigger,
        },
        database,
        instances::{Instance, InstanceKind},
    };

    use super::SqliteCollectionRepository;

    fn instance(id: &str) -> Instance {
        instance_of_kind(id, InstanceKind::Radarr)
    }

    fn instance_of_kind(id: &str, kind: InstanceKind) -> Instance {
        Instance {
            id: id.to_owned(),
            name: id.to_owned(),
            kind,
            base_url: Url::parse("http://localhost/").expect("URL"),
            external_url: None,
            api_key: "secret".to_owned(),
            config_order: 0,
        }
    }

    fn series_snapshot(episodes: Vec<SeriesEpisodeSnapshot>) -> Snapshot {
        Snapshot::Series(vec![SeriesSnapshot {
            tvdb_id: 7,
            title: "Series".to_owned(),
            title_slug: "series".to_owned(),
            year: 2020,
            size_on_disk_bytes: 512,
            file_count: 1,
            seasons: vec![SeriesSeasonSnapshot {
                season_number: 1,
                file_count: 1,
            }],
            episodes,
        }])
    }

    fn episode(season_number: i64, episode_number: i64) -> SeriesEpisodeSnapshot {
        SeriesEpisodeSnapshot {
            season_number,
            episode_number,
            title: format!("S{season_number}E{episode_number}"),
            air_date_utc: None,
            has_file: true,
            size_on_disk_bytes: 512,
        }
    }

    #[tokio::test]
    async fn records_successful_sync_and_removes_deleted_instances() {
        let pool = database::test_pool().await;
        let repository = SqliteCollectionRepository::new(pool.clone());
        let instance = instance("radarr");
        repository
            .reconcile_instances(std::slice::from_ref(&instance))
            .await
            .expect("reconcile");
        let run = repository
            .create_sync_run(
                SyncTrigger::Manual,
                std::slice::from_ref(&instance),
                Utc::now(),
            )
            .await
            .expect("create run");
        repository
            .store_successful_snapshot(
                run.id,
                &instance,
                &Snapshot::Movies(vec![MovieSnapshot {
                    tmdb_id: 1,
                    title: "Movie".to_owned(),
                    title_slug: "movie-1".to_owned(),
                    year: 2024,
                    size_on_disk_bytes: 100,
                    file_count: 1,
                    added_at: None,
                }]),
                Utc::now(),
            )
            .await
            .expect("store snapshot");
        let run = repository
            .finish_sync_run(run.id, Utc::now())
            .await
            .expect("finish run");
        assert_eq!(run.status, SyncStatus::Succeeded);

        repository
            .reconcile_instances(&[])
            .await
            .expect("remove instance");
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM movie_snapshots")
            .fetch_one(&pool)
            .await
            .expect("count snapshots");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn replaces_episode_snapshots_on_resync_and_cascades_on_removal() {
        let pool = database::test_pool().await;
        let repository = SqliteCollectionRepository::new(pool.clone());
        let instance = instance_of_kind("sonarr", InstanceKind::Sonarr);
        repository
            .reconcile_instances(std::slice::from_ref(&instance))
            .await
            .expect("reconcile");

        let run = repository
            .create_sync_run(
                SyncTrigger::Manual,
                std::slice::from_ref(&instance),
                Utc::now(),
            )
            .await
            .expect("create run");
        repository
            .store_successful_snapshot(
                run.id,
                &instance,
                &series_snapshot(vec![episode(1, 1), episode(1, 2)]),
                Utc::now(),
            )
            .await
            .expect("store first snapshot");
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM series_episode_snapshots")
            .fetch_one(&pool)
            .await
            .expect("count episodes");
        assert_eq!(count, 2);

        // A re-sync replaces the instance's episode rows wholesale.
        let run = repository
            .create_sync_run(
                SyncTrigger::Manual,
                std::slice::from_ref(&instance),
                Utc::now(),
            )
            .await
            .expect("create second run");
        repository
            .store_successful_snapshot(
                run.id,
                &instance,
                &series_snapshot(vec![episode(1, 1)]),
                Utc::now(),
            )
            .await
            .expect("store second snapshot");
        let episodes: Vec<(i64, i64)> =
            sqlx::query_as("SELECT season_number, episode_number FROM series_episode_snapshots")
                .fetch_all(&pool)
                .await
                .expect("episode rows");
        assert_eq!(episodes, vec![(1, 1)]);

        repository
            .reconcile_instances(&[])
            .await
            .expect("remove instance");
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM series_episode_snapshots")
            .fetch_one(&pool)
            .await
            .expect("count after removal");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn failed_sync_preserves_the_previous_snapshot() {
        let pool = database::test_pool().await;
        let repository = SqliteCollectionRepository::new(pool.clone());
        let instance = instance("radarr");
        repository
            .reconcile_instances(std::slice::from_ref(&instance))
            .await
            .expect("reconcile");

        let successful_run = repository
            .create_sync_run(
                SyncTrigger::Manual,
                std::slice::from_ref(&instance),
                Utc::now(),
            )
            .await
            .expect("create successful run");
        repository
            .store_successful_snapshot(
                successful_run.id,
                &instance,
                &Snapshot::Movies(vec![MovieSnapshot {
                    tmdb_id: 1,
                    title: "Preserved".to_owned(),
                    title_slug: "preserved-1".to_owned(),
                    year: 2024,
                    size_on_disk_bytes: 100,
                    file_count: 1,
                    added_at: None,
                }]),
                Utc::now(),
            )
            .await
            .expect("store snapshot");
        repository
            .finish_sync_run(successful_run.id, Utc::now())
            .await
            .expect("finish successful run");

        let failed_run = repository
            .create_sync_run(
                SyncTrigger::Manual,
                std::slice::from_ref(&instance),
                Utc::now(),
            )
            .await
            .expect("create failed run");
        repository
            .mark_instance_failed(failed_run.id, &instance.id, "unavailable", Utc::now())
            .await
            .expect("mark failed");
        let failed_run = repository
            .finish_sync_run(failed_run.id, Utc::now())
            .await
            .expect("finish failed run");
        assert_eq!(failed_run.status, SyncStatus::Failed);

        let title: String =
            sqlx::query_scalar("SELECT title FROM movie_snapshots WHERE tmdb_id = 1")
                .fetch_one(&pool)
                .await
                .expect("preserved snapshot");
        assert_eq!(title, "Preserved");
    }

    #[tokio::test]
    async fn stores_artist_album_snapshots_with_title_and_size() {
        let pool = database::test_pool().await;
        let repository = SqliteCollectionRepository::new(pool.clone());
        let instance = instance_of_kind("lidarr", InstanceKind::Lidarr);
        repository
            .reconcile_instances(std::slice::from_ref(&instance))
            .await
            .expect("reconcile");

        let run = repository
            .create_sync_run(
                SyncTrigger::Manual,
                std::slice::from_ref(&instance),
                Utc::now(),
            )
            .await
            .expect("create run");
        repository
            .store_successful_snapshot(
                run.id,
                &instance,
                &Snapshot::Artists(vec![ArtistSnapshot {
                    musicbrainz_id: "artist-1".to_owned(),
                    name: "Artist".to_owned(),
                    size_on_disk_bytes: 800,
                    file_count: 5,
                    albums: vec![ArtistAlbumSnapshot {
                        musicbrainz_id: "album-1".to_owned(),
                        title: "Album One".to_owned(),
                        size_on_disk_bytes: 800,
                        file_count: 5,
                    }],
                }]),
                Utc::now(),
            )
            .await
            .expect("store snapshot");

        let album: (String, i64, i64) = sqlx::query_as(
            "SELECT title, size_on_disk_bytes, file_count FROM artist_album_snapshots",
        )
        .fetch_one(&pool)
        .await
        .expect("album row");
        assert_eq!(album, ("Album One".to_owned(), 800, 5));
    }
}
