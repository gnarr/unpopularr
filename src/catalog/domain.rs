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
    },
    Series {
        display_name: String,
        size_on_disk_bytes: i64,
        file_count: i64,
        instances: Vec<InstanceReference>,
        tvdb_id: i64,
        year: i64,
        seasons_with_files: i64,
    },
    Artist {
        display_name: String,
        size_on_disk_bytes: i64,
        file_count: i64,
        instances: Vec<InstanceReference>,
        music_brainz_id: String,
        albums_with_files: i64,
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
        })
    }));
    content.extend(artists.into_iter().filter_map(|(music_brainz_id, artist)| {
        (artist.file_count > 0).then_some(ContentItem::Artist {
            display_name: artist.name,
            size_on_disk_bytes: artist.size_on_disk_bytes,
            file_count: artist.file_count,
            instances: artist.instances,
            music_brainz_id,
            albums_with_files: i64::try_from(artist.album_musicbrainz_ids.len())
                .unwrap_or(i64::MAX),
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
    use chrono::Utc;

    use super::{
        ArtistSource, CatalogSources, ContentItem, InstanceReference, MovieSource, SeriesSource,
        aggregate,
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
}
