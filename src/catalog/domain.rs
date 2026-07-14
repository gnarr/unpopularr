use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceReference {
    pub id: String,
    pub name: String,
    pub last_successful_sync_at: DateTime<Utc>,
    /// Path into this instance's *arr web UI for the item this reference is
    /// attached to (e.g. `movie/inception-27205`, `series/breaking-bad`,
    /// `artist/{mbid}`), relative to the instance's external URL. `None` when
    /// the routing slug isn't available yet (e.g. a snapshot synced before the
    /// slug column existed). Always item-scoped: an `InstanceReference` only
    /// ever appears within a specific content item.
    pub deep_link_path: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct CatalogSources {
    pub movies: Vec<MovieSource>,
    pub series: Vec<SeriesSource>,
    pub artists: Vec<ArtistSource>,
    pub playback: CatalogPlayback,
}

#[derive(Clone, Debug, Default)]
pub struct CatalogPlayback {
    pub available: bool,
    pub movies: BTreeMap<i64, PlaybackMetrics>,
    pub series: BTreeMap<i64, PlaybackMetrics>,
    pub artists: BTreeMap<String, PlaybackMetrics>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackMetrics {
    pub play_count: i64,
    pub play_duration_seconds: i64,
    pub last_played_at: Option<DateTime<Utc>>,
}

impl PlaybackMetrics {
    fn never_played() -> Self {
        Self {
            play_count: 0,
            play_duration_seconds: 0,
            last_played_at: None,
        }
    }
}

/// Read-time playback aggregate for one calendar day of a movie, computed from
/// `playback_events` grouped by day. This is the finest grain the movie plot
/// offers; the frontend re-buckets it to the resolution the user picks
/// (day/week/month/year). Only days that had playback are present; the frontend
/// fills the gaps to a continuous axis.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyPlayback {
    /// Calendar day as `YYYY-MM-DD` in UTC.
    pub date: String,
    pub play_count: i64,
    pub play_duration_seconds: i64,
}

/// Read-time playback aggregate for one watching user of one item, computed
/// from `playback_events` grouped by user. Rows arrive ordered by play count,
/// then recency. Plays without a user (events stored before user tracking and
/// since purged from Tautulli, plus legacy aggregates) are reported via
/// `unknown_user_play_count` on the details structs instead.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPlayback {
    /// Tautulli's stable per-user id (0 is the local Plex user).
    pub user_id: i64,
    /// Display name from the user's most recent event; `None` when the source
    /// never reported one.
    pub user_name: Option<String>,
    pub playback: PlaybackMetrics,
}

#[derive(Clone, Debug)]
pub struct MovieSource {
    pub tmdb_id: i64,
    pub title: String,
    pub title_slug: String,
    pub year: i64,
    pub size_on_disk_bytes: i64,
    pub file_count: i64,
    /// When this instance's Radarr added the movie. `None` before a re-sync
    /// populates the column, or when Radarr omits it. Details-only; the flat
    /// catalog list leaves it `None`.
    pub available_at: Option<DateTime<Utc>>,
    pub instance: InstanceReference,
    pub config_order: i64,
}

#[derive(Clone, Debug)]
pub struct SeriesSource {
    pub tvdb_id: i64,
    pub title: String,
    pub title_slug: String,
    pub year: i64,
    pub size_on_disk_bytes: i64,
    pub file_count: i64,
    pub season_numbers: Vec<i64>,
    pub instance: InstanceReference,
    pub config_order: i64,
}

#[derive(Clone, Debug)]
pub struct ArtistSource {
    pub musicbrainz_id: String,
    pub name: String,
    pub size_on_disk_bytes: i64,
    pub file_count: i64,
    pub album_musicbrainz_ids: Vec<String>,
    pub instance: InstanceReference,
    pub config_order: i64,
}

/// A season's file count for a single instance, as read from
/// `series_season_snapshots`. Multiple instances may report the same
/// `season_number`; [`aggregate_series`] sums them.
#[derive(Clone, Debug)]
pub struct SeriesSeasonFiles {
    pub season_number: i64,
    pub file_count: i64,
}

/// One episode's on-disk state for a single instance, as read from
/// `series_episode_snapshots`. Rows arrive ordered by instance `config_order`;
/// [`aggregate_series`] merges duplicates (first instance wins metadata).
#[derive(Clone, Debug)]
pub struct SeriesEpisodeFile {
    pub season_number: i64,
    pub episode_number: i64,
    pub title: String,
    pub air_date_utc: Option<DateTime<Utc>>,
    pub has_file: bool,
    pub size_on_disk_bytes: i64,
}

/// Read-time playback aggregate for one (season, episode) of a series,
/// computed from `playback_events` rows that carry episode positions.
#[derive(Clone, Debug)]
pub struct SeriesEpisodePlayback {
    pub season_number: i64,
    pub episode_number: i64,
    pub metrics: PlaybackMetrics,
}

/// Raw per-instance material for a single series, straight from the repository
/// before aggregation into [`SeriesDetails`].
#[derive(Clone, Debug, Default)]
pub struct SeriesDetailsSources {
    pub instances: Vec<SeriesSource>,
    pub seasons: Vec<SeriesSeasonFiles>,
    pub episodes: Vec<SeriesEpisodeFile>,
    pub episode_playback: Vec<SeriesEpisodePlayback>,
    pub user_playback: Vec<UserPlayback>,
    pub playback_available: bool,
    pub playback: Option<PlaybackMetrics>,
}

/// Raw per-instance material for a single movie, straight from the repository
/// before aggregation into [`MovieDetails`].
#[derive(Clone, Debug, Default)]
pub struct MovieDetailsSources {
    pub instances: Vec<MovieSource>,
    /// Per-day playback totals, ascending by day. Empty when playback is
    /// unavailable or the movie has never been played.
    pub daily_playback: Vec<DailyPlayback>,
    pub user_playback: Vec<UserPlayback>,
    pub playback_available: bool,
    pub playback: Option<PlaybackMetrics>,
}

/// One album's on-disk state for a single instance, as read from
/// `artist_album_snapshots`. Rows arrive ordered by instance `config_order`;
/// [`aggregate_artist`] merges duplicates (first non-empty title wins,
/// sizes and file counts are summed).
#[derive(Clone, Debug)]
pub struct ArtistAlbumFile {
    pub album_musicbrainz_id: String,
    pub title: String,
    pub size_on_disk_bytes: i64,
    pub file_count: i64,
}

/// Raw per-instance material for a single artist, straight from the repository
/// before aggregation into [`ArtistDetails`].
#[derive(Clone, Debug, Default)]
pub struct ArtistDetailsSources {
    pub instances: Vec<ArtistSource>,
    pub albums: Vec<ArtistAlbumFile>,
    pub user_playback: Vec<UserPlayback>,
    pub playback_available: bool,
    pub playback: Option<PlaybackMetrics>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesEpisodeDetail {
    pub episode_number: i64,
    pub title: String,
    pub air_date_utc: Option<DateTime<Utc>>,
    pub has_file: bool,
    pub size_on_disk_bytes: i64,
    pub playback: Option<PlaybackMetrics>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesSeasonDetail {
    pub season_number: i64,
    pub file_count: i64,
    /// Distinct episodes known to Sonarr, aired or not. 0 when the episode
    /// snapshot predates episode collection.
    pub episode_count: i64,
    pub episodes_with_files: i64,
    pub size_on_disk_bytes: i64,
    pub playback: Option<PlaybackMetrics>,
    pub episodes: Vec<SeriesEpisodeDetail>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesInstanceDetail {
    pub instance: InstanceReference,
    pub size_on_disk_bytes: i64,
    pub file_count: i64,
    pub season_numbers: Vec<i64>,
}

/// A single series aggregated across every instance that holds it, plus the
/// per-season and per-instance breakdowns that the flat catalog list discards.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesDetails {
    pub display_name: String,
    pub tvdb_id: i64,
    pub year: i64,
    pub size_on_disk_bytes: i64,
    pub file_count: i64,
    pub instances: Vec<InstanceReference>,
    pub seasons: Vec<SeriesSeasonDetail>,
    pub instance_details: Vec<SeriesInstanceDetail>,
    pub playback: Option<PlaybackMetrics>,
    /// Series-level plays not attributable to an episode cell: legacy
    /// aggregates, events without episode positions, and specials. `None` when
    /// playback is unavailable.
    pub unattributed_play_count: Option<i64>,
    /// Per-user playback, most plays first. Empty when playback is unavailable.
    pub user_playback: Vec<UserPlayback>,
    /// Plays not attributable to a user: legacy aggregates and events stored
    /// before user tracking. `None` when playback is unavailable.
    pub unknown_user_play_count: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovieInstanceDetail {
    pub instance: InstanceReference,
    pub size_on_disk_bytes: i64,
    pub file_count: i64,
}

/// A single movie aggregated across every instance that holds it, plus the
/// per-instance breakdown that the flat catalog list discards.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovieDetails {
    pub display_name: String,
    pub tmdb_id: i64,
    pub year: i64,
    pub size_on_disk_bytes: i64,
    pub file_count: i64,
    pub instances: Vec<InstanceReference>,
    pub instance_details: Vec<MovieInstanceDetail>,
    pub playback: Option<PlaybackMetrics>,
    /// Earliest Radarr "added" date across instances — the plot's left edge.
    /// `None` until a re-sync populates it.
    pub available_at: Option<DateTime<Utc>>,
    /// Per-day playback totals, ascending by day.
    pub daily_playback: Vec<DailyPlayback>,
    /// Per-user playback, most plays first. Empty when playback is unavailable.
    pub user_playback: Vec<UserPlayback>,
    /// Plays not attributable to a user: legacy aggregates and events stored
    /// before user tracking. `None` when playback is unavailable.
    pub unknown_user_play_count: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistAlbumDetail {
    pub music_brainz_id: String,
    /// Empty for rows synced before the title column existed; the next Lidarr
    /// sync fills it. The UI renders a fallback label for empty titles.
    pub title: String,
    pub size_on_disk_bytes: i64,
    pub file_count: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistInstanceDetail {
    pub instance: InstanceReference,
    pub size_on_disk_bytes: i64,
    pub file_count: i64,
    pub album_count: i64,
}

/// A single artist aggregated across every instance that holds it, plus the
/// per-album and per-instance breakdowns that the flat catalog list discards.
/// Artists carry no year — Lidarr does not model one.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistDetails {
    pub display_name: String,
    pub music_brainz_id: String,
    pub size_on_disk_bytes: i64,
    pub file_count: i64,
    pub instances: Vec<InstanceReference>,
    pub albums: Vec<ArtistAlbumDetail>,
    pub instance_details: Vec<ArtistInstanceDetail>,
    pub playback: Option<PlaybackMetrics>,
    /// Per-user playback, most plays first. Empty when playback is unavailable.
    pub user_playback: Vec<UserPlayback>,
    /// Plays not attributable to a user: legacy aggregates and events stored
    /// before user tracking. `None` when playback is unavailable.
    pub unknown_user_play_count: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(
    tag = "contentType",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ContentItem {
    Movie {
        display_name: String,
        size_on_disk_bytes: i64,
        file_count: i64,
        instances: Vec<InstanceReference>,
        tmdb_id: i64,
        year: i64,
        playback: Option<PlaybackMetrics>,
    },
    Series {
        display_name: String,
        size_on_disk_bytes: i64,
        file_count: i64,
        instances: Vec<InstanceReference>,
        tvdb_id: i64,
        year: i64,
        seasons_with_files: i64,
        playback: Option<PlaybackMetrics>,
    },
    Artist {
        display_name: String,
        size_on_disk_bytes: i64,
        file_count: i64,
        instances: Vec<InstanceReference>,
        music_brainz_id: String,
        albums_with_files: i64,
        playback: Option<PlaybackMetrics>,
    },
}

impl ContentItem {
    fn type_name(&self) -> &'static str {
        match self {
            Self::Artist { .. } => "artist",
            Self::Movie { .. } => "movie",
            Self::Series { .. } => "series",
        }
    }

    fn display_name(&self) -> &str {
        match self {
            Self::Artist { display_name, .. }
            | Self::Movie { display_name, .. }
            | Self::Series { display_name, .. } => display_name,
        }
    }

    fn stable_id(&self) -> String {
        match self {
            Self::Artist {
                music_brainz_id, ..
            } => music_brainz_id.clone(),
            Self::Movie { tmdb_id, .. } => tmdb_id.to_string(),
            Self::Series { tvdb_id, .. } => tvdb_id.to_string(),
        }
    }
}

/// Builds the *arr web-UI path for an item on one instance, or `None` when the
/// routing slug is empty (unavailable until the next sync). `route_base` is the
/// UI route prefix: `movie` (Radarr), `series` (Sonarr), or `artist` (Lidarr).
fn deep_link_path(route_base: &str, slug: &str) -> Option<String> {
    (!slug.is_empty()).then(|| format!("{route_base}/{slug}"))
}

pub fn aggregate(mut sources: CatalogSources) -> Vec<ContentItem> {
    sources.movies.sort_by_key(|source| source.config_order);
    sources.series.sort_by_key(|source| source.config_order);
    sources.artists.sort_by_key(|source| source.config_order);
    let playback = sources.playback;

    let mut movies = BTreeMap::<i64, MovieAggregate>::new();
    for source in sources.movies {
        let aggregate = movies
            .entry(source.tmdb_id)
            .or_insert_with(|| MovieAggregate {
                title: source.title,
                year: source.year,
                size_on_disk_bytes: 0,
                file_count: 0,
                instances: Vec::new(),
            });
        aggregate.size_on_disk_bytes += source.size_on_disk_bytes;
        aggregate.file_count += source.file_count;
        if source.file_count > 0 {
            let mut instance = source.instance;
            instance.deep_link_path = deep_link_path("movie", &source.title_slug);
            aggregate.instances.push(instance);
        }
    }

    let mut series = BTreeMap::<i64, SeriesAggregate>::new();
    for source in sources.series {
        let aggregate = series
            .entry(source.tvdb_id)
            .or_insert_with(|| SeriesAggregate {
                title: source.title,
                year: source.year,
                size_on_disk_bytes: 0,
                file_count: 0,
                season_numbers: BTreeSet::new(),
                instances: Vec::new(),
            });
        aggregate.size_on_disk_bytes += source.size_on_disk_bytes;
        aggregate.file_count += source.file_count;
        aggregate.season_numbers.extend(source.season_numbers);
        if source.file_count > 0 {
            let mut instance = source.instance;
            instance.deep_link_path = deep_link_path("series", &source.title_slug);
            aggregate.instances.push(instance);
        }
    }

    let mut artists = BTreeMap::<String, ArtistAggregate>::new();
    for source in sources.artists {
        let deep_link = deep_link_path("artist", &source.musicbrainz_id);
        let aggregate = artists
            .entry(source.musicbrainz_id)
            .or_insert_with(|| ArtistAggregate {
                name: source.name,
                size_on_disk_bytes: 0,
                file_count: 0,
                album_musicbrainz_ids: BTreeSet::new(),
                instances: Vec::new(),
            });
        aggregate.size_on_disk_bytes += source.size_on_disk_bytes;
        aggregate.file_count += source.file_count;
        aggregate
            .album_musicbrainz_ids
            .extend(source.album_musicbrainz_ids);
        if source.file_count > 0 {
            let mut instance = source.instance;
            instance.deep_link_path = deep_link;
            aggregate.instances.push(instance);
        }
    }

    let mut content = Vec::with_capacity(movies.len() + series.len() + artists.len());
    content.extend(movies.into_iter().filter_map(|(tmdb_id, movie)| {
        (movie.file_count > 0).then_some(ContentItem::Movie {
            display_name: movie.title,
            size_on_disk_bytes: movie.size_on_disk_bytes,
            file_count: movie.file_count,
            instances: movie.instances,
            tmdb_id,
            year: movie.year,
            playback: playback_metrics(playback.available, playback.movies.get(&tmdb_id)),
        })
    }));
    content.extend(series.into_iter().filter_map(|(tvdb_id, series)| {
        (series.file_count > 0).then_some(ContentItem::Series {
            display_name: series.title,
            size_on_disk_bytes: series.size_on_disk_bytes,
            file_count: series.file_count,
            instances: series.instances,
            tvdb_id,
            year: series.year,
            seasons_with_files: i64::try_from(series.season_numbers.len()).unwrap_or(i64::MAX),
            playback: playback_metrics(playback.available, playback.series.get(&tvdb_id)),
        })
    }));
    content.extend(artists.into_iter().filter_map(|(music_brainz_id, artist)| {
        let metrics = playback_metrics(playback.available, playback.artists.get(&music_brainz_id));
        (artist.file_count > 0).then_some(ContentItem::Artist {
            display_name: artist.name,
            size_on_disk_bytes: artist.size_on_disk_bytes,
            file_count: artist.file_count,
            instances: artist.instances,
            music_brainz_id,
            albums_with_files: i64::try_from(artist.album_musicbrainz_ids.len())
                .unwrap_or(i64::MAX),
            playback: metrics,
        })
    }));

    content.sort_by_cached_key(|item| {
        (
            item.type_name(),
            item.display_name().to_lowercase(),
            item.stable_id(),
        )
    });
    content
}

/// Fold the raw per-instance rows for one series into the serialized
/// [`SeriesDetails`]. Mirrors [`aggregate`]: lowest `config_order` wins for the
/// display metadata, sizes/file counts are summed, and only instances that hold
/// files appear in the header `instances` list. `instance_details` retains every
/// instance (including empty ones) and per-season file counts are summed across
/// instances.
pub fn aggregate_series(mut sources: SeriesDetailsSources) -> Option<SeriesDetails> {
    sources.instances.sort_by_key(|source| source.config_order);
    let first = sources.instances.first()?;
    let tvdb_id = first.tvdb_id;
    let display_name = first.title.clone();
    let year = first.year;

    let mut size_on_disk_bytes = 0;
    let mut file_count = 0;
    let mut instances = Vec::new();
    let mut instance_details = Vec::with_capacity(sources.instances.len());
    for source in sources.instances {
        size_on_disk_bytes += source.size_on_disk_bytes;
        file_count += source.file_count;
        let mut instance = source.instance;
        instance.deep_link_path = deep_link_path("series", &source.title_slug);
        if source.file_count > 0 {
            instances.push(instance.clone());
        }
        let mut season_numbers = source.season_numbers;
        season_numbers.sort_unstable();
        instance_details.push(SeriesInstanceDetail {
            instance,
            size_on_disk_bytes: source.size_on_disk_bytes,
            file_count: source.file_count,
            season_numbers,
        });
    }

    let mut season_files = BTreeMap::<i64, i64>::new();
    for season in sources.seasons {
        *season_files.entry(season.season_number).or_default() += season.file_count;
    }

    // First instance wins title/air date (rows arrive ordered by config_order);
    // file presence is OR'd and sizes are summed, like the series totals.
    let mut episode_map = BTreeMap::<i64, BTreeMap<i64, EpisodeAggregate>>::new();
    for episode in sources.episodes {
        match episode_map
            .entry(episode.season_number)
            .or_default()
            .entry(episode.episode_number)
        {
            Entry::Vacant(slot) => {
                slot.insert(EpisodeAggregate {
                    title: episode.title,
                    air_date_utc: episode.air_date_utc,
                    has_file: episode.has_file,
                    size_on_disk_bytes: episode.size_on_disk_bytes,
                });
            }
            Entry::Occupied(mut slot) => {
                let aggregate = slot.get_mut();
                aggregate.has_file |= episode.has_file;
                aggregate.size_on_disk_bytes += episode.size_on_disk_bytes;
            }
        }
    }

    let episode_playback = sources
        .episode_playback
        .into_iter()
        .map(|playback| {
            (
                (playback.season_number, playback.episode_number),
                playback.metrics,
            )
        })
        .collect::<BTreeMap<_, _>>();

    // Union with the season file counts so a database synced before episode
    // collection existed still lists its seasons (with an empty episode list).
    let season_numbers = season_files
        .keys()
        .chain(episode_map.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    let mut attributed_play_count = 0;
    let seasons = season_numbers
        .into_iter()
        .map(|season_number| {
            let mut episodes_with_files = 0;
            let mut season_size_bytes = 0;
            let mut season_playback = PlaybackMetrics::never_played();
            let episodes = episode_map
                .remove(&season_number)
                .unwrap_or_default()
                .into_iter()
                .map(|(episode_number, aggregate)| {
                    let metrics = episode_playback.get(&(season_number, episode_number));
                    if let Some(metrics) = metrics {
                        attributed_play_count += metrics.play_count;
                        season_playback.play_count += metrics.play_count;
                        season_playback.play_duration_seconds += metrics.play_duration_seconds;
                        season_playback.last_played_at =
                            season_playback.last_played_at.max(metrics.last_played_at);
                    }
                    episodes_with_files += i64::from(aggregate.has_file);
                    season_size_bytes += aggregate.size_on_disk_bytes;
                    SeriesEpisodeDetail {
                        episode_number,
                        title: aggregate.title,
                        air_date_utc: aggregate.air_date_utc,
                        has_file: aggregate.has_file,
                        size_on_disk_bytes: aggregate.size_on_disk_bytes,
                        playback: playback_metrics(sources.playback_available, metrics),
                    }
                })
                .collect::<Vec<_>>();

            SeriesSeasonDetail {
                season_number,
                file_count: season_files.get(&season_number).copied().unwrap_or(0),
                episode_count: i64::try_from(episodes.len()).unwrap_or(i64::MAX),
                episodes_with_files,
                size_on_disk_bytes: season_size_bytes,
                playback: playback_metrics(sources.playback_available, Some(&season_playback)),
                episodes,
            }
        })
        .collect();

    let playback = playback_metrics(sources.playback_available, sources.playback.as_ref());
    // Clamped: re-synced events inside the legacy aggregate's covered window can
    // be counted both per episode and in the series total's legacy share.
    let unattributed_play_count = playback
        .as_ref()
        .map(|metrics| (metrics.play_count - attributed_play_count).max(0));
    let unknown_user_play_count =
        unknown_user_play_count(playback.as_ref(), &sources.user_playback);

    Some(SeriesDetails {
        display_name,
        tvdb_id,
        year,
        size_on_disk_bytes,
        file_count,
        instances,
        seasons,
        instance_details,
        playback,
        unattributed_play_count,
        user_playback: sources.user_playback,
        unknown_user_play_count,
    })
}

struct EpisodeAggregate {
    title: String,
    air_date_utc: Option<DateTime<Utc>>,
    has_file: bool,
    size_on_disk_bytes: i64,
}

/// Fold the raw per-instance rows for one movie into the serialized
/// [`MovieDetails`]. Mirrors [`aggregate_series`], minus the per-season data
/// movies don't have.
pub fn aggregate_movie(mut sources: MovieDetailsSources) -> Option<MovieDetails> {
    sources.instances.sort_by_key(|source| source.config_order);
    let first = sources.instances.first()?;
    let tmdb_id = first.tmdb_id;
    let display_name = first.title.clone();
    let year = first.year;

    let mut size_on_disk_bytes = 0;
    let mut file_count = 0;
    let mut available_at: Option<DateTime<Utc>> = None;
    let mut instances = Vec::new();
    let mut instance_details = Vec::with_capacity(sources.instances.len());
    for source in sources.instances {
        size_on_disk_bytes += source.size_on_disk_bytes;
        file_count += source.file_count;
        if let Some(added) = source.available_at {
            available_at = Some(available_at.map_or(added, |current| current.min(added)));
        }
        let mut instance = source.instance;
        instance.deep_link_path = deep_link_path("movie", &source.title_slug);
        if source.file_count > 0 {
            instances.push(instance.clone());
        }
        instance_details.push(MovieInstanceDetail {
            instance,
            size_on_disk_bytes: source.size_on_disk_bytes,
            file_count: source.file_count,
        });
    }

    let playback = playback_metrics(sources.playback_available, sources.playback.as_ref());
    let unknown_user_play_count =
        unknown_user_play_count(playback.as_ref(), &sources.user_playback);

    Some(MovieDetails {
        display_name,
        tmdb_id,
        year,
        size_on_disk_bytes,
        file_count,
        instances,
        instance_details,
        playback,
        available_at,
        daily_playback: sources.daily_playback,
        user_playback: sources.user_playback,
        unknown_user_play_count,
    })
}

/// Fold the raw per-instance rows for one artist into the serialized
/// [`ArtistDetails`]. Mirrors [`aggregate_series`], with albums in the role
/// seasons play there: merged across instances and summed.
pub fn aggregate_artist(mut sources: ArtistDetailsSources) -> Option<ArtistDetails> {
    sources.instances.sort_by_key(|source| source.config_order);
    let first = sources.instances.first()?;
    let music_brainz_id = first.musicbrainz_id.clone();
    let display_name = first.name.clone();
    // The MBID is global (identical on every Lidarr instance), so every
    // instance's deep link shares the same path.
    let artist_link = deep_link_path("artist", &music_brainz_id);

    let mut size_on_disk_bytes = 0;
    let mut file_count = 0;
    let mut instances = Vec::new();
    let mut instance_details = Vec::with_capacity(sources.instances.len());
    for source in sources.instances {
        size_on_disk_bytes += source.size_on_disk_bytes;
        file_count += source.file_count;
        let mut instance = source.instance;
        instance.deep_link_path = artist_link.clone();
        if source.file_count > 0 {
            instances.push(instance.clone());
        }
        instance_details.push(ArtistInstanceDetail {
            instance,
            size_on_disk_bytes: source.size_on_disk_bytes,
            file_count: source.file_count,
            album_count: i64::try_from(source.album_musicbrainz_ids.len()).unwrap_or(i64::MAX),
        });
    }

    // First non-empty title wins (rows arrive ordered by config_order), so a
    // pre-0007 placeholder row on one instance can't blank a known title.
    let mut album_map = BTreeMap::<String, AlbumAggregate>::new();
    for album in sources.albums {
        match album_map.entry(album.album_musicbrainz_id) {
            Entry::Vacant(slot) => {
                slot.insert(AlbumAggregate {
                    title: album.title,
                    size_on_disk_bytes: album.size_on_disk_bytes,
                    file_count: album.file_count,
                });
            }
            Entry::Occupied(mut slot) => {
                let aggregate = slot.get_mut();
                if aggregate.title.is_empty() {
                    aggregate.title = album.title;
                }
                aggregate.size_on_disk_bytes += album.size_on_disk_bytes;
                aggregate.file_count += album.file_count;
            }
        }
    }
    let mut albums = album_map
        .into_iter()
        .map(|(music_brainz_id, aggregate)| ArtistAlbumDetail {
            music_brainz_id,
            title: aggregate.title,
            size_on_disk_bytes: aggregate.size_on_disk_bytes,
            file_count: aggregate.file_count,
        })
        .collect::<Vec<_>>();
    albums.sort_by_cached_key(|album| (album.title.to_lowercase(), album.music_brainz_id.clone()));

    let playback = playback_metrics(sources.playback_available, sources.playback.as_ref());
    let unknown_user_play_count =
        unknown_user_play_count(playback.as_ref(), &sources.user_playback);

    Some(ArtistDetails {
        display_name,
        music_brainz_id,
        size_on_disk_bytes,
        file_count,
        instances,
        albums,
        instance_details,
        playback,
        user_playback: sources.user_playback,
        unknown_user_play_count,
    })
}

struct AlbumAggregate {
    title: String,
    size_on_disk_bytes: i64,
    file_count: i64,
}

fn playback_metrics(available: bool, metrics: Option<&PlaybackMetrics>) -> Option<PlaybackMetrics> {
    available.then(|| {
        metrics
            .cloned()
            .unwrap_or_else(PlaybackMetrics::never_played)
    })
}

/// Plays in the item total that no user row accounts for: legacy aggregates
/// and events stored before user tracking. The per-user query excludes
/// legacy-covered events, so the subtraction is exact; the clamp only guards
/// against inconsistent stored aggregates.
fn unknown_user_play_count(
    playback: Option<&PlaybackMetrics>,
    user_playback: &[UserPlayback],
) -> Option<i64> {
    playback.map(|metrics| {
        let attributed: i64 = user_playback
            .iter()
            .map(|user| user.playback.play_count)
            .sum();
        (metrics.play_count - attributed).max(0)
    })
}

struct MovieAggregate {
    title: String,
    year: i64,
    size_on_disk_bytes: i64,
    file_count: i64,
    instances: Vec<InstanceReference>,
}

struct SeriesAggregate {
    title: String,
    year: i64,
    size_on_disk_bytes: i64,
    file_count: i64,
    season_numbers: BTreeSet<i64>,
    instances: Vec<InstanceReference>,
}

struct ArtistAggregate {
    name: String,
    size_on_disk_bytes: i64,
    file_count: i64,
    album_musicbrainz_ids: BTreeSet<String>,
    instances: Vec<InstanceReference>,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use chrono::Utc;

    use super::{
        ArtistAlbumFile, ArtistDetailsSources, ArtistSource, CatalogPlayback, CatalogSources,
        ContentItem, DailyPlayback, InstanceReference, MovieDetailsSources, MovieSource,
        PlaybackMetrics, SeriesDetailsSources, SeriesEpisodeFile, SeriesEpisodePlayback,
        SeriesSeasonDetail, SeriesSeasonFiles, SeriesSource, UserPlayback, aggregate,
        aggregate_artist, aggregate_movie, aggregate_series,
    };

    fn instance(id: &str, name: &str) -> InstanceReference {
        InstanceReference {
            id: id.to_owned(),
            name: name.to_owned(),
            last_successful_sync_at: Utc::now(),
            deep_link_path: None,
        }
    }

    #[test]
    fn combines_instances_and_uses_first_configured_metadata() {
        let content = aggregate(CatalogSources {
            movies: vec![
                MovieSource {
                    tmdb_id: 10,
                    title: "Preferred".to_owned(),
                    title_slug: "preferred-10".to_owned(),
                    year: 2020,
                    size_on_disk_bytes: 100,
                    file_count: 1,
                    available_at: None,
                    instance: instance("hd", "HD"),
                    config_order: 0,
                },
                MovieSource {
                    tmdb_id: 10,
                    title: "Other".to_owned(),
                    title_slug: "other-10".to_owned(),
                    year: 2021,
                    size_on_disk_bytes: 400,
                    file_count: 1,
                    available_at: None,
                    instance: instance("uhd", "4K"),
                    config_order: 1,
                },
            ],
            ..CatalogSources::default()
        });

        let ContentItem::Movie {
            display_name,
            size_on_disk_bytes,
            file_count,
            instances,
            ..
        } = &content[0]
        else {
            panic!("expected movie");
        };
        assert_eq!(display_name, "Preferred");
        assert_eq!(*size_on_disk_bytes, 500);
        assert_eq!(*file_count, 2);
        assert_eq!(instances.len(), 2);
        // Each instance carries its own Radarr deep-link path (built from that
        // instance's titleSlug), not a single globally-shared one.
        assert_eq!(
            instances[0].deep_link_path.as_deref(),
            Some("movie/preferred-10")
        );
        assert_eq!(
            instances[1].deep_link_path.as_deref(),
            Some("movie/other-10")
        );
    }

    #[test]
    fn counts_unique_seasons_and_albums_and_filters_empty_content() {
        let shared_instance = instance("one", "One");
        let content = aggregate(CatalogSources {
            movies: vec![MovieSource {
                tmdb_id: 99,
                title: "Empty".to_owned(),
                title_slug: "empty-99".to_owned(),
                year: 2022,
                size_on_disk_bytes: 0,
                file_count: 0,
                available_at: None,
                instance: shared_instance.clone(),
                config_order: 0,
            }],
            series: vec![
                SeriesSource {
                    tvdb_id: 1,
                    title: "Show".to_owned(),
                    title_slug: "show".to_owned(),
                    year: 2020,
                    size_on_disk_bytes: 100,
                    file_count: 2,
                    season_numbers: vec![1, 2],
                    instance: shared_instance.clone(),
                    config_order: 0,
                },
                SeriesSource {
                    tvdb_id: 1,
                    title: "Show".to_owned(),
                    title_slug: "show".to_owned(),
                    year: 2020,
                    size_on_disk_bytes: 200,
                    file_count: 2,
                    season_numbers: vec![2, 3],
                    instance: instance("two", "Two"),
                    config_order: 1,
                },
            ],
            artists: vec![
                ArtistSource {
                    musicbrainz_id: "artist".to_owned(),
                    name: "Artist".to_owned(),
                    size_on_disk_bytes: 100,
                    file_count: 2,
                    album_musicbrainz_ids: vec!["a".to_owned(), "b".to_owned()],
                    instance: shared_instance,
                    config_order: 0,
                },
                ArtistSource {
                    musicbrainz_id: "artist".to_owned(),
                    name: "Artist".to_owned(),
                    size_on_disk_bytes: 200,
                    file_count: 3,
                    album_musicbrainz_ids: vec!["b".to_owned(), "c".to_owned()],
                    instance: instance("two", "Two"),
                    config_order: 1,
                },
            ],
            playback: CatalogPlayback::default(),
        });

        assert_eq!(content.len(), 2);
        assert!(matches!(
            &content[0],
            ContentItem::Artist {
                albums_with_files: 3,
                ..
            }
        ));
        assert!(matches!(
            &content[1],
            ContentItem::Series {
                seasons_with_files: 3,
                ..
            }
        ));
    }

    #[test]
    fn distinguishes_unavailable_never_played_and_played_content() {
        let movie = MovieSource {
            tmdb_id: 10,
            title: "Movie".to_owned(),
            title_slug: "movie-10".to_owned(),
            year: 2024,
            size_on_disk_bytes: 100,
            file_count: 1,
            available_at: None,
            instance: instance("one", "One"),
            config_order: 0,
        };
        let unavailable = aggregate(CatalogSources {
            movies: vec![movie.clone()],
            ..CatalogSources::default()
        });
        assert!(matches!(
            &unavailable[0],
            ContentItem::Movie { playback: None, .. }
        ));

        let never_played = aggregate(CatalogSources {
            movies: vec![movie.clone()],
            playback: CatalogPlayback {
                available: true,
                ..CatalogPlayback::default()
            },
            ..CatalogSources::default()
        });
        assert!(matches!(
            &never_played[0],
            ContentItem::Movie {
                playback: Some(PlaybackMetrics { play_count: 0, .. }),
                ..
            }
        ));

        let played = aggregate(CatalogSources {
            movies: vec![movie],
            playback: CatalogPlayback {
                available: true,
                movies: BTreeMap::from([(
                    10,
                    PlaybackMetrics {
                        play_count: 3,
                        play_duration_seconds: 600,
                        last_played_at: None,
                    },
                )]),
                ..CatalogPlayback::default()
            },
            ..CatalogSources::default()
        });
        assert!(matches!(
            &played[0],
            ContentItem::Movie {
                playback: Some(PlaybackMetrics { play_count: 3, .. }),
                ..
            }
        ));
    }

    fn series_source(
        title: &str,
        size_on_disk_bytes: i64,
        file_count: i64,
        season_numbers: Vec<i64>,
        instance: InstanceReference,
        config_order: i64,
    ) -> SeriesSource {
        SeriesSource {
            tvdb_id: 1,
            title: title.to_owned(),
            title_slug: title.to_lowercase(),
            year: 2020,
            size_on_disk_bytes,
            file_count,
            season_numbers,
            instance,
            config_order,
        }
    }

    #[test]
    fn series_details_sums_sizes_and_per_season_files_across_instances() {
        let details = aggregate_series(SeriesDetailsSources {
            instances: vec![
                // Deliberately out of config order to prove the sort.
                series_source("Other", 200, 2, vec![2, 3], instance("two", "Two"), 1),
                series_source("Show", 100, 3, vec![1, 2], instance("one", "One"), 0),
            ],
            seasons: vec![
                SeriesSeasonFiles {
                    season_number: 1,
                    file_count: 2,
                },
                SeriesSeasonFiles {
                    season_number: 2,
                    file_count: 1,
                },
                SeriesSeasonFiles {
                    season_number: 2,
                    file_count: 1,
                },
                SeriesSeasonFiles {
                    season_number: 3,
                    file_count: 4,
                },
            ],
            ..SeriesDetailsSources::default()
        })
        .expect("series details");

        assert_eq!(details.display_name, "Show"); // lowest config_order wins
        assert_eq!(details.size_on_disk_bytes, 300);
        assert_eq!(details.file_count, 5);
        assert_eq!(details.instances.len(), 2);
        // Each instance keeps its own Sonarr slug rather than sharing one.
        assert_eq!(
            details.instances[0].deep_link_path.as_deref(),
            Some("series/show")
        );
        assert_eq!(
            details.instances[1].deep_link_path.as_deref(),
            Some("series/other")
        );
        // The per-instance breakdown carries the same per-instance links.
        assert_eq!(
            details.instance_details[0]
                .instance
                .deep_link_path
                .as_deref(),
            Some("series/show")
        );
        assert_eq!(
            details.instance_details[1]
                .instance
                .deep_link_path
                .as_deref(),
            Some("series/other")
        );
        let empty_season = |season_number, file_count| SeriesSeasonDetail {
            season_number,
            file_count,
            episode_count: 0,
            episodes_with_files: 0,
            size_on_disk_bytes: 0,
            playback: None,
            episodes: Vec::new(),
        };
        assert_eq!(
            details.seasons,
            vec![
                empty_season(1, 2),
                empty_season(2, 2), // summed across both instances
                empty_season(3, 4),
            ]
        );
        assert_eq!(details.instance_details.len(), 2);
        assert_eq!(details.instance_details[0].instance.id, "one");
        assert_eq!(details.instance_details[0].season_numbers, vec![1, 2]);
        assert!(details.playback.is_none());
    }

    #[test]
    fn series_details_excludes_empty_instance_from_header_but_keeps_it_in_details() {
        let details = aggregate_series(SeriesDetailsSources {
            instances: vec![
                series_source("Show", 100, 2, vec![1], instance("full", "Full"), 0),
                series_source("Show", 0, 0, vec![], instance("empty", "Empty"), 1),
            ],
            seasons: vec![SeriesSeasonFiles {
                season_number: 1,
                file_count: 2,
            }],
            ..SeriesDetailsSources::default()
        })
        .expect("series details");

        assert_eq!(details.instances.len(), 1);
        assert_eq!(details.instances[0].id, "full");
        assert_eq!(details.instance_details.len(), 2);
        assert!(
            details
                .instance_details
                .iter()
                .any(|detail| detail.instance.id == "empty" && detail.file_count == 0)
        );
    }

    #[test]
    fn series_details_distinguishes_playback_availability() {
        let build = |playback_available, playback| {
            aggregate_series(SeriesDetailsSources {
                instances: vec![series_source(
                    "Show",
                    100,
                    2,
                    vec![1],
                    instance("one", "One"),
                    0,
                )],
                seasons: Vec::new(),
                playback_available,
                playback,
                ..SeriesDetailsSources::default()
            })
            .expect("series details")
        };

        assert!(build(false, None).playback.is_none());
        assert!(matches!(
            build(true, None).playback,
            Some(PlaybackMetrics { play_count: 0, .. })
        ));
        assert!(matches!(
            build(
                true,
                Some(PlaybackMetrics {
                    play_count: 7,
                    play_duration_seconds: 900,
                    last_played_at: None,
                })
            )
            .playback,
            Some(PlaybackMetrics { play_count: 7, .. })
        ));
    }

    #[test]
    fn series_details_returns_none_without_instances() {
        assert!(aggregate_series(SeriesDetailsSources::default()).is_none());
    }

    fn episode_file(
        season_number: i64,
        episode_number: i64,
        title: &str,
        has_file: bool,
        size_on_disk_bytes: i64,
    ) -> SeriesEpisodeFile {
        SeriesEpisodeFile {
            season_number,
            episode_number,
            title: title.to_owned(),
            air_date_utc: None,
            has_file,
            size_on_disk_bytes,
        }
    }

    #[test]
    fn series_details_merges_episodes_across_instances() {
        let details = aggregate_series(SeriesDetailsSources {
            instances: vec![
                series_source("Show", 100, 2, vec![1], instance("one", "One"), 0),
                series_source("Show", 480, 2, vec![1], instance("two", "Two"), 1),
            ],
            seasons: vec![SeriesSeasonFiles {
                season_number: 1,
                file_count: 3,
            }],
            // Rows arrive ordered by config_order; the first title wins, file
            // presence is OR'd, and sizes are summed.
            episodes: vec![
                episode_file(1, 1, "Pilot", true, 100),
                episode_file(1, 2, "Second", false, 0),
                episode_file(1, 1, "Pilot (4K)", true, 400),
                episode_file(1, 2, "Second (4K)", true, 80),
            ],
            ..SeriesDetailsSources::default()
        })
        .expect("series details");

        assert_eq!(details.seasons.len(), 1);
        let season = &details.seasons[0];
        assert_eq!(season.episode_count, 2);
        assert_eq!(season.episodes_with_files, 2);
        assert_eq!(season.size_on_disk_bytes, 580);
        assert_eq!(season.episodes[0].title, "Pilot");
        assert_eq!(season.episodes[0].size_on_disk_bytes, 500);
        assert!(season.episodes[1].has_file);
        assert!(season.playback.is_none()); // playback unavailable
        assert_eq!(details.unattributed_play_count, None);
    }

    #[test]
    fn series_details_attaches_episode_playback_and_counts_unattributed() {
        let last_played = chrono::DateTime::from_timestamp(1_000, 0);
        let sources = || SeriesDetailsSources {
            instances: vec![series_source(
                "Show",
                200,
                2,
                vec![1],
                instance("one", "One"),
                0,
            )],
            episodes: vec![
                episode_file(1, 1, "Pilot", true, 100),
                episode_file(1, 2, "Second", true, 100),
            ],
            episode_playback: vec![
                SeriesEpisodePlayback {
                    season_number: 1,
                    episode_number: 1,
                    metrics: PlaybackMetrics {
                        play_count: 3,
                        play_duration_seconds: 1_800,
                        last_played_at: last_played,
                    },
                },
                // No matching episode cell (numbering mismatch or removed from
                // Sonarr): stays unattributed.
                SeriesEpisodePlayback {
                    season_number: 9,
                    episode_number: 1,
                    metrics: PlaybackMetrics {
                        play_count: 2,
                        play_duration_seconds: 600,
                        last_played_at: None,
                    },
                },
            ],
            playback_available: true,
            playback: Some(PlaybackMetrics {
                play_count: 7,
                play_duration_seconds: 3_000,
                last_played_at: last_played,
            }),
            ..SeriesDetailsSources::default()
        };

        let details = aggregate_series(sources()).expect("series details");
        let season = &details.seasons[0];
        let season_playback = season.playback.as_ref().expect("season playback");
        assert_eq!(season_playback.play_count, 3);
        assert_eq!(season_playback.play_duration_seconds, 1_800);
        assert_eq!(season_playback.last_played_at, last_played);
        let pilot = season.episodes[0].playback.as_ref().expect("pilot metrics");
        assert_eq!(pilot.play_count, 3);
        // Known but never-played episodes get zeroed metrics, not None.
        let second = season.episodes[1]
            .playback
            .as_ref()
            .expect("second metrics");
        assert_eq!(second.play_count, 0);
        // 7 series plays minus the 3 attributed to cells; the unmatched season 9
        // rows stay in the remainder.
        assert_eq!(details.unattributed_play_count, Some(4));

        // Legacy-window overlap can double count; the remainder clamps at zero.
        let mut overlapping = sources();
        overlapping.playback = Some(PlaybackMetrics {
            play_count: 2,
            play_duration_seconds: 600,
            last_played_at: last_played,
        });
        let details = aggregate_series(overlapping).expect("series details");
        assert_eq!(details.unattributed_play_count, Some(0));
    }

    fn movie_source(
        title: &str,
        size_on_disk_bytes: i64,
        file_count: i64,
        instance: InstanceReference,
        config_order: i64,
    ) -> MovieSource {
        MovieSource {
            tmdb_id: 10,
            title: title.to_owned(),
            title_slug: title.to_lowercase(),
            year: 2020,
            size_on_disk_bytes,
            file_count,
            available_at: None,
            instance,
            config_order,
        }
    }

    #[test]
    fn movie_details_sums_sizes_across_instances_and_uses_first_configured_metadata() {
        let details = aggregate_movie(MovieDetailsSources {
            instances: vec![
                // Deliberately out of config order to prove the sort.
                movie_source("Other", 400, 1, instance("two", "Two"), 1),
                movie_source("Movie", 100, 1, instance("one", "One"), 0),
            ],
            ..MovieDetailsSources::default()
        })
        .expect("movie details");

        assert_eq!(details.display_name, "Movie"); // lowest config_order wins
        assert_eq!(details.year, 2020);
        assert_eq!(details.size_on_disk_bytes, 500);
        assert_eq!(details.file_count, 2);
        assert_eq!(details.instances.len(), 2);
        assert_eq!(details.instance_details.len(), 2);
        assert_eq!(details.instance_details[0].instance.id, "one");
        assert_eq!(details.instance_details[0].size_on_disk_bytes, 100);
        assert!(details.playback.is_none());
    }

    #[test]
    fn movie_details_excludes_empty_instance_from_header_but_keeps_it_in_details() {
        let details = aggregate_movie(MovieDetailsSources {
            instances: vec![
                movie_source("Movie", 100, 1, instance("full", "Full"), 0),
                movie_source("Movie", 0, 0, instance("empty", "Empty"), 1),
            ],
            ..MovieDetailsSources::default()
        })
        .expect("movie details");

        assert_eq!(details.instances.len(), 1);
        assert_eq!(details.instances[0].id, "full");
        assert_eq!(details.instance_details.len(), 2);
        assert!(
            details
                .instance_details
                .iter()
                .any(|detail| detail.instance.id == "empty" && detail.file_count == 0)
        );
    }

    #[test]
    fn movie_details_distinguishes_playback_availability() {
        let build = |playback_available, playback| {
            aggregate_movie(MovieDetailsSources {
                instances: vec![movie_source("Movie", 100, 1, instance("one", "One"), 0)],
                playback_available,
                playback,
                ..MovieDetailsSources::default()
            })
            .expect("movie details")
        };

        assert!(build(false, None).playback.is_none());
        assert!(matches!(
            build(true, None).playback,
            Some(PlaybackMetrics { play_count: 0, .. })
        ));
        assert!(matches!(
            build(
                true,
                Some(PlaybackMetrics {
                    play_count: 7,
                    play_duration_seconds: 900,
                    last_played_at: None,
                })
            )
            .playback,
            Some(PlaybackMetrics { play_count: 7, .. })
        ));
    }

    #[test]
    fn movie_details_returns_none_without_instances() {
        assert!(aggregate_movie(MovieDetailsSources::default()).is_none());
    }

    #[test]
    fn movie_details_passes_user_playback_and_counts_unknown_users() {
        let build = |playback_available, play_count, user_counts: Vec<i64>| {
            aggregate_movie(MovieDetailsSources {
                instances: vec![movie_source("Movie", 100, 1, instance("one", "One"), 0)],
                user_playback: user_counts
                    .into_iter()
                    .enumerate()
                    .map(|(index, count)| UserPlayback {
                        user_id: i64::try_from(index).expect("small index"),
                        user_name: Some(format!("User {index}")),
                        playback: PlaybackMetrics {
                            play_count: count,
                            play_duration_seconds: 0,
                            last_played_at: None,
                        },
                    })
                    .collect(),
                playback_available,
                playback: Some(PlaybackMetrics {
                    play_count,
                    play_duration_seconds: 0,
                    last_played_at: None,
                }),
                ..MovieDetailsSources::default()
            })
            .expect("movie details")
        };

        // Rows pass through; the remainder is the header total minus them.
        let details = build(true, 7, vec![3, 2]);
        assert_eq!(details.user_playback.len(), 2);
        assert_eq!(
            details.user_playback[0].user_name.as_deref(),
            Some("User 0")
        );
        assert_eq!(details.unknown_user_play_count, Some(2));

        // Fully attributed items report zero, and inconsistent stored
        // aggregates clamp rather than going negative.
        assert_eq!(build(true, 5, vec![3, 2]).unknown_user_play_count, Some(0));
        assert_eq!(build(true, 4, vec![3, 2]).unknown_user_play_count, Some(0));

        // Without a playback source there is nothing to report.
        assert_eq!(build(false, 0, vec![]).unknown_user_play_count, None);
    }

    #[test]
    fn movie_details_takes_earliest_availability_and_passes_daily_playback() {
        let early = chrono::DateTime::from_timestamp(1_000, 0);
        let late = chrono::DateTime::from_timestamp(2_000, 0);
        let with_availability = |available_at, config_order| {
            let mut source = movie_source("Movie", 100, 1, instance("one", "One"), config_order);
            source.available_at = available_at;
            source
        };

        let details = aggregate_movie(MovieDetailsSources {
            // The later-added instance sorts first to prove min, not first-wins.
            instances: vec![
                with_availability(late, 0),
                with_availability(early, 1),
                with_availability(None, 2),
            ],
            daily_playback: vec![
                DailyPlayback {
                    date: "2024-01-10".to_owned(),
                    play_count: 2,
                    play_duration_seconds: 3_600,
                },
                DailyPlayback {
                    date: "2024-03-05".to_owned(),
                    play_count: 1,
                    play_duration_seconds: 1_800,
                },
            ],
            playback_available: true,
            ..MovieDetailsSources::default()
        })
        .expect("movie details");

        assert_eq!(details.available_at, early);
        assert_eq!(details.daily_playback.len(), 2);
        assert_eq!(details.daily_playback[0].date, "2024-01-10");
        assert_eq!(details.daily_playback[1].play_duration_seconds, 1_800);
    }

    fn artist_source(
        name: &str,
        size_on_disk_bytes: i64,
        file_count: i64,
        album_musicbrainz_ids: Vec<&str>,
        instance: InstanceReference,
        config_order: i64,
    ) -> ArtistSource {
        ArtistSource {
            musicbrainz_id: "artist-1".to_owned(),
            name: name.to_owned(),
            size_on_disk_bytes,
            file_count,
            album_musicbrainz_ids: album_musicbrainz_ids
                .into_iter()
                .map(str::to_owned)
                .collect(),
            instance,
            config_order,
        }
    }

    fn album_file(
        album_musicbrainz_id: &str,
        title: &str,
        size_on_disk_bytes: i64,
        file_count: i64,
    ) -> ArtistAlbumFile {
        ArtistAlbumFile {
            album_musicbrainz_id: album_musicbrainz_id.to_owned(),
            title: title.to_owned(),
            size_on_disk_bytes,
            file_count,
        }
    }

    #[test]
    fn artist_details_sums_sizes_across_instances_and_uses_first_configured_metadata() {
        let details = aggregate_artist(ArtistDetailsSources {
            instances: vec![
                // Deliberately out of config order to prove the sort.
                artist_source("Other", 400, 3, vec!["a", "b"], instance("two", "Two"), 1),
                artist_source("Artist", 100, 2, vec!["a"], instance("one", "One"), 0),
            ],
            ..ArtistDetailsSources::default()
        })
        .expect("artist details");

        assert_eq!(details.display_name, "Artist"); // lowest config_order wins
        assert_eq!(details.music_brainz_id, "artist-1");
        assert_eq!(details.size_on_disk_bytes, 500);
        assert_eq!(details.file_count, 5);
        assert_eq!(details.instances.len(), 2);
        assert_eq!(details.instance_details.len(), 2);
        assert_eq!(details.instance_details[0].instance.id, "one");
        assert_eq!(details.instance_details[0].album_count, 1);
        assert_eq!(details.instance_details[1].album_count, 2);
        assert!(details.playback.is_none());
    }

    #[test]
    fn artist_details_merges_albums_across_instances() {
        let details = aggregate_artist(ArtistDetailsSources {
            instances: vec![artist_source(
                "Artist",
                100,
                2,
                vec!["a", "b"],
                instance("one", "One"),
                0,
            )],
            // Rows arrive ordered by config_order; the first non-empty title
            // wins (a pre-0007 placeholder can't blank a known title), sizes
            // and file counts are summed.
            albums: vec![
                album_file("a", "", 0, 3),
                album_file("b", "Beta", 200, 2),
                album_file("a", "Alpha", 400, 3),
            ],
            ..ArtistDetailsSources::default()
        })
        .expect("artist details");

        assert_eq!(details.albums.len(), 2);
        // Sorted by lowercase title.
        assert_eq!(details.albums[0].title, "Alpha");
        assert_eq!(details.albums[0].music_brainz_id, "a");
        assert_eq!(details.albums[0].size_on_disk_bytes, 400);
        assert_eq!(details.albums[0].file_count, 6);
        assert_eq!(details.albums[1].title, "Beta");
    }

    #[test]
    fn artist_details_excludes_empty_instance_from_header_but_keeps_it_in_details() {
        let details = aggregate_artist(ArtistDetailsSources {
            instances: vec![
                artist_source("Artist", 100, 2, vec!["a"], instance("full", "Full"), 0),
                artist_source("Artist", 0, 0, vec![], instance("empty", "Empty"), 1),
            ],
            ..ArtistDetailsSources::default()
        })
        .expect("artist details");

        assert_eq!(details.instances.len(), 1);
        assert_eq!(details.instances[0].id, "full");
        assert_eq!(details.instance_details.len(), 2);
        assert!(
            details
                .instance_details
                .iter()
                .any(|detail| detail.instance.id == "empty" && detail.file_count == 0)
        );
    }

    #[test]
    fn artist_details_distinguishes_playback_availability() {
        let build = |playback_available, playback| {
            aggregate_artist(ArtistDetailsSources {
                instances: vec![artist_source(
                    "Artist",
                    100,
                    2,
                    vec!["a"],
                    instance("one", "One"),
                    0,
                )],
                playback_available,
                playback,
                ..ArtistDetailsSources::default()
            })
            .expect("artist details")
        };

        assert!(build(false, None).playback.is_none());
        assert!(matches!(
            build(true, None).playback,
            Some(PlaybackMetrics { play_count: 0, .. })
        ));
        assert!(matches!(
            build(
                true,
                Some(PlaybackMetrics {
                    play_count: 7,
                    play_duration_seconds: 900,
                    last_played_at: None,
                })
            )
            .playback,
            Some(PlaybackMetrics { play_count: 7, .. })
        ));
    }

    #[test]
    fn artist_details_returns_none_without_instances() {
        assert!(aggregate_artist(ArtistDetailsSources::default()).is_none());
    }
}
