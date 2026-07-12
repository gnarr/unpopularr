use std::collections::HashMap;

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};

use crate::catalog::{
    ArtistAlbumFile, ArtistDetailsSources, ArtistSource, CatalogPlayback, CatalogRepository,
    CatalogSources, DailyPlayback, InstanceReference, MovieDetailsSources, MovieSource,
    PlaybackMetrics, SeriesDetailsSources, SeriesEpisodeFile, SeriesEpisodePlayback,
    SeriesSeasonFiles, SeriesSource,
};

#[derive(Clone)]
pub struct SqliteCatalogRepository {
    pool: SqlitePool,
}

impl SqliteCatalogRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    async fn playback_available(&self) -> Result<bool> {
        Ok(sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM playback_sources
                WHERE last_successful_sync_at IS NOT NULL
            )
            "#,
        )
        .fetch_one(&self.pool)
        .await?)
    }

    /// The stored aggregate for one item, assuming a single playback source
    /// (a config invariant today).
    async fn playback_snapshot(
        &self,
        content_type: &str,
        content_id: &str,
    ) -> Result<Option<PlaybackMetrics>> {
        sqlx::query(
            r#"
            SELECT play_count, play_duration_seconds, last_played_at
            FROM playback_snapshots
            WHERE content_type = ?1 AND content_id = ?2
            "#,
        )
        .bind(content_type)
        .bind(content_id)
        .fetch_optional(&self.pool)
        .await?
        .map(|row| {
            Ok(PlaybackMetrics {
                play_count: row.try_get("play_count")?,
                play_duration_seconds: row.try_get("play_duration_seconds")?,
                last_played_at: row.try_get("last_played_at")?,
            })
        })
        .transpose()
    }

    /// Per-calendar-day playback totals for one movie, ascending by day.
    /// `substr(played_at, 1, 10)` takes the `YYYY-MM-DD` prefix of the stored
    /// UTC RFC3339 timestamp — events are always UTC, so this is the UTC day.
    /// Only days that had playback appear; the frontend fills the gaps and
    /// re-buckets to the resolution the user picks.
    async fn daily_movie_playback(&self, tmdb_id: i64) -> Result<Vec<DailyPlayback>> {
        sqlx::query(
            r#"
            SELECT substr(played_at, 1, 10) AS date,
                   COUNT(*) AS play_count,
                   COALESCE(SUM(duration_seconds), 0) AS play_duration_seconds
            FROM playback_events AS events
            LEFT JOIN playback_legacy_snapshots AS legacy
                ON legacy.source_id = events.source_id
               AND legacy.content_type = events.content_type
               AND legacy.content_id = events.content_id
            WHERE events.content_type = 'movie' AND events.content_id = ?1
              AND (
                  legacy.source_id IS NULL
                  OR events.played_at > legacy.covered_until
              )
            GROUP BY date
            ORDER BY date
            "#,
        )
        .bind(tmdb_id.to_string())
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok(DailyPlayback {
                date: row.try_get("date")?,
                play_count: row.try_get("play_count")?,
                play_duration_seconds: row.try_get("play_duration_seconds")?,
            })
        })
        .collect()
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
                    // Availability drives the details plot only; the list query
                    // doesn't fetch it.
                    available_at: None,
                    instance: instance_reference(&row)?,
                    config_order: row.try_get("config_order")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let series_rows = sqlx::query(
            r#"
            SELECT s.tvdb_id, s.title, s.title_slug, s.year, s.size_on_disk_bytes,
                   s.file_count,
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
                    title_slug: row.try_get("title_slug")?,
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

        let playback_available = self.playback_available().await?;
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

    async fn load_series(&self, tvdb_id: i64) -> Result<Option<SeriesDetailsSources>> {
        let series_rows = sqlx::query(
            r#"
            SELECT s.tvdb_id, s.title, s.title_slug, s.year, s.size_on_disk_bytes,
                   s.file_count,
                   i.id AS instance_id, i.name AS instance_name, i.config_order,
                   i.last_successful_sync_at
            FROM series_snapshots s
            JOIN instances i ON i.id = s.instance_id
            WHERE s.tvdb_id = ?1
            ORDER BY i.config_order
            "#,
        )
        .bind(tvdb_id)
        .fetch_all(&self.pool)
        .await?;
        if series_rows.is_empty() {
            return Ok(None);
        }

        // Unlike load_sources, this query keeps file_count — the per-season
        // breakdown is exactly what the details page exists to expose.
        let season_rows = sqlx::query(
            r#"
            SELECT instance_id, season_number, file_count
            FROM series_season_snapshots
            WHERE tvdb_id = ?1
            ORDER BY season_number
            "#,
        )
        .bind(tvdb_id)
        .fetch_all(&self.pool)
        .await?;
        let mut seasons = Vec::with_capacity(season_rows.len());
        let mut season_numbers_by_instance = HashMap::<String, Vec<i64>>::new();
        for row in season_rows {
            let instance_id: String = row.try_get("instance_id")?;
            let season_number: i64 = row.try_get("season_number")?;
            seasons.push(SeriesSeasonFiles {
                season_number,
                file_count: row.try_get("file_count")?,
            });
            season_numbers_by_instance
                .entry(instance_id)
                .or_default()
                .push(season_number);
        }

        // Ordered by config_order so aggregate_series' first-wins merge picks
        // the same instance that wins the display metadata.
        let episode_rows = sqlx::query(
            r#"
            SELECT e.season_number, e.episode_number, e.title, e.air_date_utc,
                   e.has_file, e.size_on_disk_bytes
            FROM series_episode_snapshots e
            JOIN instances i ON i.id = e.instance_id
            WHERE e.tvdb_id = ?1
            ORDER BY i.config_order, e.season_number, e.episode_number
            "#,
        )
        .bind(tvdb_id)
        .fetch_all(&self.pool)
        .await?;
        let episodes = episode_rows
            .into_iter()
            .map(|row| {
                Ok::<_, anyhow::Error>(SeriesEpisodeFile {
                    season_number: row.try_get("season_number")?,
                    episode_number: row.try_get("episode_number")?,
                    title: row.try_get("title")?,
                    air_date_utc: row.try_get("air_date_utc")?,
                    has_file: row.try_get("has_file")?,
                    size_on_disk_bytes: row.try_get("size_on_disk_bytes")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let playback_available = self.playback_available().await?;
        let playback = if playback_available {
            self.playback_snapshot("series", &tvdb_id.to_string())
                .await?
        } else {
            None
        };
        let episode_playback = if playback_available {
            // Aggregated at read time from the raw events; only rows that carry
            // episode positions can be attributed to a matrix cell.
            sqlx::query(
                r#"
                SELECT season_number, episode_number,
                       COUNT(*) AS play_count,
                       COALESCE(SUM(duration_seconds), 0) AS play_duration_seconds,
                       MAX(played_at) AS last_played_at
                FROM playback_events
                WHERE content_type = 'series' AND content_id = ?1
                  AND season_number IS NOT NULL AND episode_number IS NOT NULL
                GROUP BY season_number, episode_number
                "#,
            )
            .bind(tvdb_id.to_string())
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|row| {
                Ok::<_, anyhow::Error>(SeriesEpisodePlayback {
                    season_number: row.try_get("season_number")?,
                    episode_number: row.try_get("episode_number")?,
                    metrics: PlaybackMetrics {
                        play_count: row.try_get("play_count")?,
                        play_duration_seconds: row.try_get("play_duration_seconds")?,
                        last_played_at: row.try_get("last_played_at")?,
                    },
                })
            })
            .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };

        let instances = series_rows
            .into_iter()
            .map(|row| {
                let instance_id: String = row.try_get("instance_id")?;
                Ok(SeriesSource {
                    tvdb_id: row.try_get("tvdb_id")?,
                    title: row.try_get("title")?,
                    title_slug: row.try_get("title_slug")?,
                    year: row.try_get("year")?,
                    size_on_disk_bytes: row.try_get("size_on_disk_bytes")?,
                    file_count: row.try_get("file_count")?,
                    season_numbers: season_numbers_by_instance
                        .remove(&instance_id)
                        .unwrap_or_default(),
                    instance: instance_reference_with_id(&row, instance_id)?,
                    config_order: row.try_get("config_order")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Some(SeriesDetailsSources {
            instances,
            seasons,
            episodes,
            episode_playback,
            playback_available,
            playback,
        }))
    }

    async fn load_movie(&self, tmdb_id: i64) -> Result<Option<MovieDetailsSources>> {
        let movie_rows = sqlx::query(
            r#"
            SELECT m.title, m.year, m.size_on_disk_bytes, m.file_count, m.added_at,
                   i.id AS instance_id, i.name AS instance_name, i.config_order,
                   i.last_successful_sync_at
            FROM movie_snapshots m
            JOIN instances i ON i.id = m.instance_id
            WHERE m.tmdb_id = ?1
            ORDER BY i.config_order
            "#,
        )
        .bind(tmdb_id)
        .fetch_all(&self.pool)
        .await?;
        if movie_rows.is_empty() {
            return Ok(None);
        }

        let instances = movie_rows
            .into_iter()
            .map(|row| {
                Ok(MovieSource {
                    tmdb_id,
                    title: row.try_get("title")?,
                    year: row.try_get("year")?,
                    size_on_disk_bytes: row.try_get("size_on_disk_bytes")?,
                    file_count: row.try_get("file_count")?,
                    available_at: row.try_get("added_at")?,
                    instance: instance_reference(&row)?,
                    config_order: row.try_get("config_order")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let playback_available = self.playback_available().await?;
        let playback = if playback_available {
            self.playback_snapshot("movie", &tmdb_id.to_string())
                .await?
        } else {
            None
        };
        let daily_playback = if playback_available {
            self.daily_movie_playback(tmdb_id).await?
        } else {
            Vec::new()
        };

        Ok(Some(MovieDetailsSources {
            instances,
            daily_playback,
            playback_available,
            playback,
        }))
    }

    async fn load_artist(&self, musicbrainz_id: &str) -> Result<Option<ArtistDetailsSources>> {
        let artist_rows = sqlx::query(
            r#"
            SELECT a.name, a.size_on_disk_bytes, a.file_count,
                   i.id AS instance_id, i.name AS instance_name, i.config_order,
                   i.last_successful_sync_at
            FROM artist_snapshots a
            JOIN instances i ON i.id = a.instance_id
            WHERE a.musicbrainz_id = ?1
            ORDER BY i.config_order
            "#,
        )
        .bind(musicbrainz_id)
        .fetch_all(&self.pool)
        .await?;
        if artist_rows.is_empty() {
            return Ok(None);
        }

        // Ordered by config_order so aggregate_artist's first-wins title merge
        // picks the same instance that wins the display metadata.
        let album_rows = sqlx::query(
            r#"
            SELECT al.instance_id, al.album_musicbrainz_id, al.title,
                   al.size_on_disk_bytes, al.file_count
            FROM artist_album_snapshots al
            JOIN instances i ON i.id = al.instance_id
            WHERE al.artist_musicbrainz_id = ?1
            ORDER BY i.config_order, al.album_musicbrainz_id
            "#,
        )
        .bind(musicbrainz_id)
        .fetch_all(&self.pool)
        .await?;
        let mut albums = Vec::with_capacity(album_rows.len());
        let mut album_ids_by_instance = HashMap::<String, Vec<String>>::new();
        for row in album_rows {
            let instance_id: String = row.try_get("instance_id")?;
            let album_musicbrainz_id: String = row.try_get("album_musicbrainz_id")?;
            albums.push(ArtistAlbumFile {
                album_musicbrainz_id: album_musicbrainz_id.clone(),
                title: row.try_get("title")?,
                size_on_disk_bytes: row.try_get("size_on_disk_bytes")?,
                file_count: row.try_get("file_count")?,
            });
            album_ids_by_instance
                .entry(instance_id)
                .or_default()
                .push(album_musicbrainz_id);
        }

        let instances = artist_rows
            .into_iter()
            .map(|row| {
                let instance_id: String = row.try_get("instance_id")?;
                Ok(ArtistSource {
                    musicbrainz_id: musicbrainz_id.to_owned(),
                    name: row.try_get("name")?,
                    size_on_disk_bytes: row.try_get("size_on_disk_bytes")?,
                    file_count: row.try_get("file_count")?,
                    album_musicbrainz_ids: album_ids_by_instance
                        .remove(&instance_id)
                        .unwrap_or_default(),
                    instance: instance_reference_with_id(&row, instance_id)?,
                    config_order: row.try_get("config_order")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let playback_available = self.playback_available().await?;
        let playback = if playback_available {
            self.playback_snapshot("artist", musicbrainz_id).await?
        } else {
            None
        };

        Ok(Some(ArtistDetailsSources {
            instances,
            albums,
            playback_available,
            playback,
        }))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database;

    #[tokio::test]
    async fn daily_movie_playback_excludes_events_covered_by_legacy_snapshot() {
        let pool = database::test_pool().await;
        let repository = SqliteCatalogRepository::new(pool.clone());

        sqlx::query("INSERT INTO playback_sources (id, provider) VALUES ('plex', 'tautulli')")
            .execute(&pool)
            .await
            .expect("seed playback source");
        sqlx::query(
            r#"
            INSERT INTO playback_legacy_snapshots (
                source_id, content_type, content_id, play_count,
                play_duration_seconds, covered_until
            )
            VALUES ('plex', 'movie', '42', 1, 60, '2024-01-15T00:00:00Z')
            "#,
        )
        .execute(&pool)
        .await
        .expect("seed legacy snapshot");
        sqlx::query(
            r#"
            INSERT INTO playback_events (
                source_id, source_row_id, content_type, content_id,
                played_at, duration_seconds
            )
            VALUES ('plex', 1, 'movie', '42', '2024-01-10T00:00:00Z', 60),
                   ('plex', 2, 'movie', '42', '2024-01-20T00:00:00Z', 120)
            "#,
        )
        .execute(&pool)
        .await
        .expect("seed playback events");

        let daily = repository
            .daily_movie_playback(42)
            .await
            .expect("load daily playback");

        assert_eq!(daily.len(), 1);
        assert_eq!(daily[0].date, "2024-01-20");
        assert_eq!(daily[0].play_count, 1);
        assert_eq!(daily[0].play_duration_seconds, 120);
    }
}
