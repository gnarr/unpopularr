use std::collections::HashMap;

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{Row, SqlitePool};

use crate::catalog::{
    ArtistAlbumFile, ArtistDetailsSources, ArtistSource, CatalogPlayback, CatalogRepository,
    CatalogSources, DailyPlayback, InstanceReference, ItemPlayback, MovieDetailsSources,
    MovieSource, PlaybackMetrics, SeriesDetailsSources, SeriesEpisodeFile, SeriesEpisodePlayback,
    SeriesSeasonFiles, SeriesSource, UserPlayback,
};

/// Distinct UTC days with playback per item, the popularity score's input.
/// Events inside a legacy aggregate's covered window are excluded, like every
/// other playback read, because the legacy row already stands for them. In
/// their place a legacy aggregate with plays and a timestamp contributes one
/// synthetic day: its last play, the only date it reliably carries. `UNION`
/// (not `UNION ALL`) collapses same-day sessions, and a synthetic day that
/// coincides with a real one. A macro so the per-item query can extend it and
/// both stay `&'static str`.
macro_rules! active_days_sql {
    () => {
        r#"
        SELECT events.content_type, events.content_id,
               substr(events.played_at, 1, 10) AS day
        FROM playback_events AS events
        LEFT JOIN playback_legacy_snapshots AS legacy
            ON legacy.source_id = events.source_id
           AND legacy.content_type = events.content_type
           AND legacy.content_id = events.content_id
        WHERE legacy.source_id IS NULL OR events.played_at > legacy.covered_until
        UNION
        SELECT content_type, content_id, substr(last_played_at, 1, 10) AS day
        FROM playback_legacy_snapshots
        WHERE play_count > 0 AND last_played_at IS NOT NULL
        "#
    };
}

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

    /// Active playback days for one item; see `active_days_sql!`.
    async fn active_days(&self, content_type: &str, content_id: &str) -> Result<Vec<NaiveDate>> {
        sqlx::query(concat!(
            "SELECT day FROM (",
            active_days_sql!(),
            ") WHERE content_type = ?1 AND content_id = ?2 ORDER BY day"
        ))
        .bind(content_type)
        .bind(content_id)
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(parse_day)
        .collect()
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

    /// Per-user playback totals for one item, most plays first. Excludes
    /// events already counted in the legacy aggregate (like
    /// [`Self::daily_movie_playback`]) so the rows plus the unattributed
    /// remainder add up to the item's stored total. Grouping by user id alone
    /// assumes a single playback source (a config invariant today). The bare
    /// `user_name` column legally takes its value from the `MAX(played_at)`
    /// row — SQLite's single-min/max rule — so each user shows the display
    /// name of their most recent session.
    async fn user_playback(
        &self,
        content_type: &str,
        content_id: &str,
    ) -> Result<Vec<UserPlayback>> {
        sqlx::query(
            r#"
            SELECT events.user_id, events.user_name,
                   COUNT(*) AS play_count,
                   COALESCE(SUM(events.duration_seconds), 0) AS play_duration_seconds,
                   MAX(events.played_at) AS last_played_at
            FROM playback_events AS events
            LEFT JOIN playback_legacy_snapshots AS legacy
                ON legacy.source_id = events.source_id
               AND legacy.content_type = events.content_type
               AND legacy.content_id = events.content_id
            WHERE events.content_type = ?1 AND events.content_id = ?2
              AND events.user_id IS NOT NULL
              AND (
                  legacy.source_id IS NULL
                  OR events.played_at > legacy.covered_until
              )
            GROUP BY events.user_id
            ORDER BY play_count DESC, last_played_at DESC, events.user_id
            "#,
        )
        .bind(content_type)
        .bind(content_id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok(UserPlayback {
                user_id: row.try_get("user_id")?,
                user_name: row.try_get("user_name")?,
                playback: PlaybackMetrics {
                    play_count: row.try_get("play_count")?,
                    play_duration_seconds: row.try_get("play_duration_seconds")?,
                    last_played_at: row.try_get("last_played_at")?,
                },
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
            SELECT m.tmdb_id, m.title, m.title_slug, m.year, m.size_on_disk_bytes,
                   m.file_count, m.added_at,
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
                    title_slug: row.try_get("title_slug")?,
                    year: row.try_get("year")?,
                    size_on_disk_bytes: row.try_get("size_on_disk_bytes")?,
                    file_count: row.try_get("file_count")?,
                    added_at: row.try_get("added_at")?,
                    instance: instance_reference(&row)?,
                    config_order: row.try_get("config_order")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let series_rows = sqlx::query(
            r#"
            SELECT s.tvdb_id, s.title, s.title_slug, s.year, s.size_on_disk_bytes,
                   s.file_count, s.added_at,
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
                    added_at: row.try_get("added_at")?,
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
                   a.added_at,
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
                    added_at: row.try_get("added_at")?,
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
                item_playback(&mut playback, &row)?.metrics = Some(PlaybackMetrics {
                    play_count: row.try_get("play_count")?,
                    play_duration_seconds: row.try_get("play_duration_seconds")?,
                    last_played_at: row.try_get("last_played_at")?,
                });
            }

            // Playback history outlives catalog snapshots, so skip removed
            // items instead of materializing days nothing will score.
            let rows = sqlx::query(concat!(
                "SELECT content_type, content_id, day FROM (",
                active_days_sql!(),
                r#")
                WHERE (content_type = 'movie' AND content_id IN (
                          SELECT CAST(tmdb_id AS TEXT) FROM movie_snapshots))
                   OR (content_type = 'series' AND content_id IN (
                          SELECT CAST(tvdb_id AS TEXT) FROM series_snapshots))
                   OR (content_type = 'artist' AND content_id IN (
                          SELECT musicbrainz_id FROM artist_snapshots))
                "#
            ))
            .fetch_all(&self.pool)
            .await?;
            for row in rows {
                let day = parse_day(&row)?;
                item_playback(&mut playback, &row)?.active_days.push(day);
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
                   s.file_count, s.added_at,
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
                   e.has_file, e.size_on_disk_bytes, e.added_at
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
                    added_at: row.try_get("added_at")?,
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
        let user_playback = if playback_available {
            self.user_playback("series", &tvdb_id.to_string()).await?
        } else {
            Vec::new()
        };
        let active_days = if playback_available {
            self.active_days("series", &tvdb_id.to_string()).await?
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
                    added_at: row.try_get("added_at")?,
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
            user_playback,
            active_days,
            playback_available,
            playback,
        }))
    }

    async fn load_movie(&self, tmdb_id: i64) -> Result<Option<MovieDetailsSources>> {
        let movie_rows = sqlx::query(
            r#"
            SELECT m.title, m.title_slug, m.year, m.size_on_disk_bytes, m.file_count,
                   m.added_at,
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
                    title_slug: row.try_get("title_slug")?,
                    year: row.try_get("year")?,
                    size_on_disk_bytes: row.try_get("size_on_disk_bytes")?,
                    file_count: row.try_get("file_count")?,
                    added_at: row.try_get("added_at")?,
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
        let user_playback = if playback_available {
            self.user_playback("movie", &tmdb_id.to_string()).await?
        } else {
            Vec::new()
        };
        let active_days = if playback_available {
            self.active_days("movie", &tmdb_id.to_string()).await?
        } else {
            Vec::new()
        };

        Ok(Some(MovieDetailsSources {
            instances,
            daily_playback,
            user_playback,
            active_days,
            playback_available,
            playback,
        }))
    }

    async fn load_artist(&self, musicbrainz_id: &str) -> Result<Option<ArtistDetailsSources>> {
        let artist_rows = sqlx::query(
            r#"
            SELECT a.name, a.size_on_disk_bytes, a.file_count, a.added_at,
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
                    added_at: row.try_get("added_at")?,
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
        let user_playback = if playback_available {
            self.user_playback("artist", musicbrainz_id).await?
        } else {
            Vec::new()
        };
        let active_days = if playback_available {
            self.active_days("artist", musicbrainz_id).await?
        } else {
            Vec::new()
        };

        Ok(Some(ArtistDetailsSources {
            instances,
            albums,
            user_playback,
            active_days,
            playback_available,
            playback,
        }))
    }
}

/// The catalog entry a playback row belongs to, keyed by its typed content ID.
fn item_playback<'a>(
    playback: &'a mut CatalogPlayback,
    row: &sqlx::sqlite::SqliteRow,
) -> Result<&'a mut ItemPlayback> {
    let content_type: String = row.try_get("content_type")?;
    let content_id: String = row.try_get("content_id")?;
    let numeric_id = || {
        content_id
            .parse::<i64>()
            .with_context(|| format!("invalid {content_type} playback ID {content_id}"))
    };
    Ok(match content_type.as_str() {
        "movie" => playback.movies.entry(numeric_id()?).or_default(),
        "series" => playback.series.entry(numeric_id()?).or_default(),
        "artist" => playback.artists.entry(content_id).or_default(),
        _ => anyhow::bail!("unknown playback content type in database: {content_type}"),
    })
}

/// The `day` column of an active-days row: the `YYYY-MM-DD` prefix of a stored
/// UTC timestamp, so already the UTC calendar day.
fn parse_day(row: &sqlx::sqlite::SqliteRow) -> Result<NaiveDate> {
    let day: String = row.try_get("day")?;
    day.parse()
        .with_context(|| format!("invalid playback day {day}"))
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
        // The deep-link path depends on the item this reference is attached to;
        // the aggregation layer fills it in per (item, instance).
        deep_link_path: None,
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

    #[tokio::test]
    async fn user_playback_groups_by_user_and_excludes_covered_and_unattributed_events() {
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
        // Alice: one covered event (excluded) and two live ones carrying an old
        // and a new display name. Bob: one live event. Row 5 has no user.
        sqlx::query(
            r#"
            INSERT INTO playback_events (
                source_id, source_row_id, content_type, content_id,
                played_at, duration_seconds, user_id, user_name
            )
            VALUES ('plex', 1, 'movie', '42', '2024-01-10T00:00:00Z', 60, 7, 'Alice'),
                   ('plex', 2, 'movie', '42', '2024-01-20T00:00:00Z', 120, 7, 'Alice'),
                   ('plex', 3, 'movie', '42', '2024-01-22T00:00:00Z', 30, 7, 'Alicia'),
                   ('plex', 4, 'movie', '42', '2024-01-25T00:00:00Z', 90, 0, 'Local'),
                   ('plex', 5, 'movie', '42', '2024-01-26T00:00:00Z', 45, NULL, NULL)
            "#,
        )
        .execute(&pool)
        .await
        .expect("seed playback events");

        let users = repository
            .user_playback("movie", "42")
            .await
            .expect("load user playback");

        assert_eq!(users.len(), 2);
        // Alice leads with two live plays and reports her latest display name.
        assert_eq!(users[0].user_id, 7);
        assert_eq!(users[0].user_name.as_deref(), Some("Alicia"));
        assert_eq!(users[0].playback.play_count, 2);
        assert_eq!(users[0].playback.play_duration_seconds, 150);
        assert_eq!(
            users[0]
                .playback
                .last_played_at
                .map(|played| played.to_rfc3339()),
            Some("2024-01-22T00:00:00+00:00".to_owned())
        );
        assert_eq!(users[1].user_id, 0);
        assert_eq!(users[1].user_name.as_deref(), Some("Local"));
        assert_eq!(users[1].playback.play_count, 1);
    }

    async fn seed(pool: &SqlitePool, sql: &'static str) {
        sqlx::query(sql).execute(pool).await.expect("seed");
    }

    fn day(iso: &str) -> NaiveDate {
        iso.parse().expect("valid date")
    }

    fn sorted(mut days: Vec<NaiveDate>) -> Vec<NaiveDate> {
        days.sort_unstable();
        days
    }

    #[tokio::test]
    async fn load_sources_collects_distinct_utc_days_per_item() {
        let pool = database::test_pool().await;
        let repository = SqliteCatalogRepository::new(pool.clone());
        seed(
            &pool,
            r#"
            INSERT INTO playback_sources (id, provider, last_successful_sync_at)
            VALUES ('plex', 'tautulli', '2026-01-03T00:00:00Z')
            "#,
        )
        .await;
        seed(
            &pool,
            r#"
            INSERT INTO instances (id, name, kind, config_order, last_successful_sync_at)
            VALUES ('a', 'a', 'radarr', 0, '2026-01-03T00:00:00Z')
            "#,
        )
        .await;
        for sql in [
            r#"
            INSERT INTO movie_snapshots
                (instance_id, tmdb_id, title, year, size_on_disk_bytes, file_count)
            VALUES ('a', 42, 'Movie', 2020, 100, 1)
            "#,
            r#"
            INSERT INTO series_snapshots
                (instance_id, tvdb_id, title, year, size_on_disk_bytes, file_count)
            VALUES ('a', 42, 'Show', 2020, 100, 1)
            "#,
            r#"
            INSERT INTO artist_snapshots
                (instance_id, musicbrainz_id, name, size_on_disk_bytes, file_count)
            VALUES ('a', 'mbid', 'Artist', 100, 1)
            "#,
        ] {
            seed(&pool, sql).await;
        }
        // Movie 42: two sessions bracketing one UTC day, then the next day's
        // first second twice (separate Tautulli rows, same timestamp), once
        // without a user. The series shares an ID string with the movie,
        // proving rows are keyed per content type. Movie 99 has history but
        // has left the catalog.
        seed(
            &pool,
            r#"
            INSERT INTO playback_events (
                source_id, source_row_id, content_type, content_id,
                played_at, duration_seconds, user_id, user_name
            )
            VALUES ('plex', 1, 'movie', '42', '2026-01-01T00:00:00Z', 60, 7, 'Alice'),
                   ('plex', 2, 'movie', '42', '2026-01-01T23:59:59Z', 60, 7, 'Alice'),
                   ('plex', 3, 'movie', '42', '2026-01-02T00:00:00Z', 60, 7, 'Alice'),
                   ('plex', 4, 'movie', '42', '2026-01-02T00:00:00Z', 60, NULL, NULL),
                   ('plex', 5, 'series', '42', '2026-01-03T12:00:00Z', 60, 7, 'Alice'),
                   ('plex', 6, 'artist', 'mbid', '2025-12-31T23:59:59Z', 60, NULL, NULL),
                   ('plex', 7, 'movie', '99', '2026-01-01T12:00:00Z', 60, NULL, NULL)
            "#,
        )
        .await;

        let playback = repository.load_sources().await.expect("load").playback;

        assert!(playback.available);
        assert_eq!(
            sorted(playback.movies[&42].active_days.clone()),
            vec![day("2026-01-01"), day("2026-01-02")]
        );
        assert_eq!(playback.series[&42].active_days, vec![day("2026-01-03")]);
        assert_eq!(
            playback.artists["mbid"].active_days,
            vec![day("2025-12-31")]
        );
        assert!(!playback.movies.contains_key(&99));
    }

    #[tokio::test]
    async fn load_sources_skips_playback_before_the_first_successful_sync() {
        let pool = database::test_pool().await;
        let repository = SqliteCatalogRepository::new(pool.clone());
        seed(
            &pool,
            "INSERT INTO playback_sources (id, provider) VALUES ('plex', 'tautulli')",
        )
        .await;
        seed(
            &pool,
            r#"
            INSERT INTO playback_events (
                source_id, source_row_id, content_type, content_id,
                played_at, duration_seconds
            )
            VALUES ('plex', 1, 'movie', '42', '2026-01-01T00:00:00Z', 60)
            "#,
        )
        .await;

        let playback = repository.load_sources().await.expect("load").playback;

        assert!(!playback.available);
        assert!(playback.movies.is_empty());
    }

    #[tokio::test]
    async fn active_days_replace_covered_events_with_one_legacy_day() {
        let pool = database::test_pool().await;
        let repository = SqliteCatalogRepository::new(pool.clone());
        seed(
            &pool,
            "INSERT INTO playback_sources (id, provider) VALUES ('plex', 'tautulli')",
        )
        .await;
        // 42: covered history plus one later event. 43: the legacy day and a
        // post-cutoff event share a date. 44: no reliable timestamp. 45: no
        // plays, so its timestamp is not evidence of activity.
        seed(
            &pool,
            r#"
            INSERT INTO playback_legacy_snapshots (
                source_id, content_type, content_id, play_count,
                play_duration_seconds, last_played_at, covered_until
            )
            VALUES ('plex', 'movie', '42', 3, 600, '2024-01-12T10:00:00Z', '2024-01-15T00:00:00Z'),
                   ('plex', 'movie', '43', 1, 60, '2024-02-01T08:00:00Z', '2024-02-01T09:00:00Z'),
                   ('plex', 'movie', '44', 2, 120, NULL, '2024-01-15T00:00:00Z'),
                   ('plex', 'movie', '45', 0, 0, '2024-01-12T10:00:00Z', '2024-01-15T00:00:00Z')
            "#,
        )
        .await;
        seed(
            &pool,
            r#"
            INSERT INTO playback_events (
                source_id, source_row_id, content_type, content_id,
                played_at, duration_seconds
            )
            VALUES ('plex', 1, 'movie', '42', '2024-01-10T00:00:00Z', 60),
                   ('plex', 2, 'movie', '42', '2024-01-12T20:00:00Z', 60),
                   ('plex', 3, 'movie', '42', '2024-01-20T00:00:00Z', 60),
                   ('plex', 4, 'movie', '43', '2024-02-01T21:00:00Z', 60)
            "#,
        )
        .await;

        let days = |id: &'static str| {
            let repository = repository.clone();
            async move { repository.active_days("movie", id).await.expect("days") }
        };
        assert_eq!(days("42").await, vec![day("2024-01-12"), day("2024-01-20")]);
        assert_eq!(days("43").await, vec![day("2024-02-01")]);
        assert!(days("44").await.is_empty());
        assert!(days("45").await.is_empty());
    }
}
