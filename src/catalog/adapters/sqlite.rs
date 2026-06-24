use std::collections::HashMap;

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};

use crate::catalog::{
    ArtistSource, CatalogPlayback, CatalogRepository, CatalogSources, InstanceReference,
    MovieSource, PlaybackMetrics, SeriesSource,
};

#[derive(Clone)]
pub struct SqliteCatalogRepository {
    pool: SqlitePool,
}

impl SqliteCatalogRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CatalogRepository for SqliteCatalogRepository {
    async fn load_sources(&self) -> Result<CatalogSources> {
        let season_rows = sqlx::query(
            r#"
            SELECT instance_id, tvdb_id, season_number
            FROM series_season_snapshots
            ORDER BY season_number
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        let mut seasons = HashMap::<(String, i64), Vec<i64>>::new();
        for row in season_rows {
            seasons
                .entry((row.try_get("instance_id")?, row.try_get("tvdb_id")?))
                .or_default()
                .push(row.try_get("season_number")?);
        }

        let album_rows = sqlx::query(
            r#"
            SELECT instance_id, artist_musicbrainz_id, album_musicbrainz_id
            FROM artist_album_snapshots
            ORDER BY album_musicbrainz_id
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        let mut albums = HashMap::<(String, String), Vec<String>>::new();
        for row in album_rows {
            albums
                .entry((
                    row.try_get("instance_id")?,
                    row.try_get("artist_musicbrainz_id")?,
                ))
                .or_default()
                .push(row.try_get("album_musicbrainz_id")?);
        }

        let movie_rows = sqlx::query(
            r#"
            SELECT m.tmdb_id, m.title, m.year, m.size_on_disk_bytes, m.file_count,
                   i.id AS instance_id, i.name AS instance_name, i.config_order,
                   i.last_successful_sync_at
            FROM movie_snapshots m
            JOIN instances i ON i.id = m.instance_id
            ORDER BY i.config_order, m.tmdb_id
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        let movies = movie_rows
            .into_iter()
            .map(|row| {
                Ok(MovieSource {
                    tmdb_id: row.try_get("tmdb_id")?,
                    title: row.try_get("title")?,
                    year: row.try_get("year")?,
                    size_on_disk_bytes: row.try_get("size_on_disk_bytes")?,
                    file_count: row.try_get("file_count")?,
                    instance: instance_reference(&row)?,
                    config_order: row.try_get("config_order")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let series_rows = sqlx::query(
            r#"
            SELECT s.tvdb_id, s.title, s.year, s.size_on_disk_bytes, s.file_count,
                   i.id AS instance_id, i.name AS instance_name, i.config_order,
                   i.last_successful_sync_at
            FROM series_snapshots s
            JOIN instances i ON i.id = s.instance_id
            ORDER BY i.config_order, s.tvdb_id
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        let series = series_rows
            .into_iter()
            .map(|row| {
                let instance_id: String = row.try_get("instance_id")?;
                let tvdb_id = row.try_get("tvdb_id")?;
                Ok(SeriesSource {
                    tvdb_id,
                    title: row.try_get("title")?,
                    year: row.try_get("year")?,
                    size_on_disk_bytes: row.try_get("size_on_disk_bytes")?,
                    file_count: row.try_get("file_count")?,
                    season_numbers: seasons
                        .remove(&(instance_id.clone(), tvdb_id))
                        .unwrap_or_default(),
                    instance: instance_reference_with_id(&row, instance_id)?,
                    config_order: row.try_get("config_order")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let artist_rows = sqlx::query(
            r#"
            SELECT a.musicbrainz_id, a.name, a.size_on_disk_bytes, a.file_count,
                   i.id AS instance_id, i.name AS instance_name, i.config_order,
                   i.last_successful_sync_at
            FROM artist_snapshots a
            JOIN instances i ON i.id = a.instance_id
            ORDER BY i.config_order, a.musicbrainz_id
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        let artists = artist_rows
            .into_iter()
            .map(|row| {
                let instance_id: String = row.try_get("instance_id")?;
                let musicbrainz_id: String = row.try_get("musicbrainz_id")?;
                Ok(ArtistSource {
                    musicbrainz_id: musicbrainz_id.clone(),
                    name: row.try_get("name")?,
                    size_on_disk_bytes: row.try_get("size_on_disk_bytes")?,
                    file_count: row.try_get("file_count")?,
                    album_musicbrainz_ids: albums
                        .remove(&(instance_id.clone(), musicbrainz_id))
                        .unwrap_or_default(),
                    instance: instance_reference_with_id(&row, instance_id)?,
                    config_order: row.try_get("config_order")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let playback_available = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM playback_sources
                WHERE last_successful_sync_at IS NOT NULL
            )
            "#,
        )
        .fetch_one(&self.pool)
        .await?;
        let mut playback = CatalogPlayback {
            available: playback_available,
            ..CatalogPlayback::default()
        };
        if playback_available {
            let rows = sqlx::query(
                r#"
                SELECT content_type, content_id, play_count, play_duration_seconds,
                       last_played_at
                FROM playback_snapshots
                "#,
            )
            .fetch_all(&self.pool)
            .await?;
            for row in rows {
                let metrics = PlaybackMetrics {
                    play_count: row.try_get("play_count")?,
                    play_duration_seconds: row.try_get("play_duration_seconds")?,
                    last_played_at: row.try_get("last_played_at")?,
                };
                let content_id: String = row.try_get("content_id")?;
                match row.try_get::<String, _>("content_type")?.as_str() {
                    "movie" => {
                        playback.movies.insert(
                            content_id.parse().with_context(|| {
                                format!("invalid movie playback ID {content_id}")
                            })?,
                            metrics,
                        );
                    }
                    "series" => {
                        playback.series.insert(
                            content_id.parse().with_context(|| {
                                format!("invalid series playback ID {content_id}")
                            })?,
                            metrics,
                        );
                    }
                    "artist" => {
                        playback.artists.insert(content_id, metrics);
                    }
                    content_type => {
                        anyhow::bail!("unknown playback content type in database: {content_type}");
                    }
                }
            }
        }

        Ok(CatalogSources {
            movies,
            series,
            artists,
            playback,
        })
    }
}

fn instance_reference(row: &sqlx::sqlite::SqliteRow) -> Result<InstanceReference> {
    instance_reference_with_id(row, row.try_get("instance_id")?)
}

fn instance_reference_with_id(
    row: &sqlx::sqlite::SqliteRow,
    id: String,
) -> Result<InstanceReference> {
    let last_successful_sync_at = row
        .try_get::<Option<DateTime<Utc>>, _>("last_successful_sync_at")?
        .context("snapshot instance is missing last_successful_sync_at")?;
    Ok(InstanceReference {
        id,
        name: row.try_get("instance_name")?,
        last_successful_sync_at,
    })
}
