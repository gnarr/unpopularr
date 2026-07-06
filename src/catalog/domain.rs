use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceReference {
    pub id: String,
    pub name: String,
    pub last_successful_sync_at: DateTime<Utc>,
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

#[derive(Clone, Debug)]
pub struct MovieSource {
    pub tmdb_id: i64,
    pub title: String,
    pub year: i64,
    pub size_on_disk_bytes: i64,
    pub file_count: i64,
    pub instance: InstanceReference,
    pub config_order: i64,
}

#[derive(Clone, Debug)]
pub struct SeriesSource {
    pub tvdb_id: i64,
    pub title: String,
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

/// Raw per-instance material for a single series, straight from the repository
/// before aggregation into [`SeriesDetails`].
#[derive(Clone, Debug, Default)]
pub struct SeriesDetailsSources {
    pub instances: Vec<SeriesSource>,
    pub seasons: Vec<SeriesSeasonFiles>,
    pub playback_available: bool,
    pub playback: Option<PlaybackMetrics>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesSeasonDetail {
    pub season_number: i64,
    pub file_count: i64,
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
            aggregate.instances.push(source.instance);
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
            aggregate.instances.push(source.instance);
        }
    }

    let mut artists = BTreeMap::<String, ArtistAggregate>::new();
    for source in sources.artists {
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
            aggregate.instances.push(source.instance);
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
        if source.file_count > 0 {
            instances.push(source.instance.clone());
        }
        let mut season_numbers = source.season_numbers;
        season_numbers.sort_unstable();
        instance_details.push(SeriesInstanceDetail {
            instance: source.instance,
            size_on_disk_bytes: source.size_on_disk_bytes,
            file_count: source.file_count,
            season_numbers,
        });
    }

    let mut season_files = BTreeMap::<i64, i64>::new();
    for season in sources.seasons {
        *season_files.entry(season.season_number).or_default() += season.file_count;
    }
    let seasons = season_files
        .into_iter()
        .map(|(season_number, file_count)| SeriesSeasonDetail {
            season_number,
            file_count,
        })
        .collect();

    Some(SeriesDetails {
        display_name,
        tvdb_id,
        year,
        size_on_disk_bytes,
        file_count,
        instances,
        seasons,
        instance_details,
        playback: playback_metrics(sources.playback_available, sources.playback.as_ref()),
    })
}

fn playback_metrics(available: bool, metrics: Option<&PlaybackMetrics>) -> Option<PlaybackMetrics> {
    available.then(|| {
        metrics
            .cloned()
            .unwrap_or_else(PlaybackMetrics::never_played)
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
        ArtistSource, CatalogPlayback, CatalogSources, ContentItem, InstanceReference, MovieSource,
        PlaybackMetrics, SeriesDetailsSources, SeriesSeasonDetail, SeriesSeasonFiles, SeriesSource,
        aggregate, aggregate_series,
    };

    fn instance(id: &str, name: &str) -> InstanceReference {
        InstanceReference {
            id: id.to_owned(),
            name: name.to_owned(),
            last_successful_sync_at: Utc::now(),
        }
    }

    #[test]
    fn combines_instances_and_uses_first_configured_metadata() {
        let content = aggregate(CatalogSources {
            movies: vec![
                MovieSource {
                    tmdb_id: 10,
                    title: "Preferred".to_owned(),
                    year: 2020,
                    size_on_disk_bytes: 100,
                    file_count: 1,
                    instance: instance("hd", "HD"),
                    config_order: 0,
                },
                MovieSource {
                    tmdb_id: 10,
                    title: "Other".to_owned(),
                    year: 2021,
                    size_on_disk_bytes: 400,
                    file_count: 1,
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
    }

    #[test]
    fn counts_unique_seasons_and_albums_and_filters_empty_content() {
        let shared_instance = instance("one", "One");
        let content = aggregate(CatalogSources {
            movies: vec![MovieSource {
                tmdb_id: 99,
                title: "Empty".to_owned(),
                year: 2022,
                size_on_disk_bytes: 0,
                file_count: 0,
                instance: shared_instance.clone(),
                config_order: 0,
            }],
            series: vec![
                SeriesSource {
                    tvdb_id: 1,
                    title: "Show".to_owned(),
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
            year: 2024,
            size_on_disk_bytes: 100,
            file_count: 1,
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
            playback_available: false,
            playback: None,
        })
        .expect("series details");

        assert_eq!(details.display_name, "Show"); // lowest config_order wins
        assert_eq!(details.size_on_disk_bytes, 300);
        assert_eq!(details.file_count, 5);
        assert_eq!(details.instances.len(), 2);
        assert_eq!(
            details.seasons,
            vec![
                SeriesSeasonDetail {
                    season_number: 1,
                    file_count: 2,
                },
                SeriesSeasonDetail {
                    season_number: 2,
                    file_count: 2, // summed across both instances
                },
                SeriesSeasonDetail {
                    season_number: 3,
                    file_count: 4,
                },
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
            playback_available: false,
            playback: None,
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
}
