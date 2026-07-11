use std::{collections::BTreeMap, time::Duration};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use futures::{StreamExt, TryStreamExt, stream};
use reqwest::{
    Client, Url,
    header::{CONTENT_TYPE, LOCATION},
    redirect::Policy,
};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::{
    collection::{
        ArtistAlbumSnapshot, ArtistSnapshot, MovieSnapshot, SeriesEpisodeSnapshot,
        SeriesSeasonSnapshot, SeriesSnapshot, Snapshot,
    },
    instances::{Instance, InstanceKind},
};

const MAX_CONCURRENT_EPISODE_REQUESTS: usize = 8;

#[derive(Clone)]
pub struct ArrClient {
    client: Client,
}

impl ArrClient {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(Policy::none())
            .user_agent(concat!("unpopularr/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("failed to create HTTP client")?;
        Ok(Self { client })
    }

    pub async fn collect(&self, instance: &Instance) -> Result<Snapshot> {
        match instance.kind {
            InstanceKind::Radarr => self.collect_radarr(instance).await,
            InstanceKind::Sonarr => self.collect_sonarr(instance).await,
            InstanceKind::Lidarr => self.collect_lidarr(instance).await,
        }
    }

    async fn collect_radarr(&self, instance: &Instance) -> Result<Snapshot> {
        let movies: Vec<RadarrMovie> = self
            .get(instance, "api/v3/movie", &[("excludeLocalCovers", "true")])
            .await?;

        Ok(Snapshot::Movies(
            movies
                .into_iter()
                .map(|movie| {
                    let statistics = movie.statistics.unwrap_or_default();
                    MovieSnapshot {
                        tmdb_id: movie.tmdb_id,
                        title: movie.title,
                        year: movie.year,
                        size_on_disk_bytes: non_negative(
                            statistics.size_on_disk.or(movie.size_on_disk).unwrap_or(0),
                        ),
                        file_count: non_negative(statistics.movie_file_count.unwrap_or_else(
                            || {
                                if movie.has_file.unwrap_or(false) {
                                    1
                                } else {
                                    0
                                }
                            },
                        )),
                        added_at: movie.added,
                    }
                })
                .collect(),
        ))
    }

    async fn collect_sonarr(&self, instance: &Instance) -> Result<Snapshot> {
        let series: Vec<SonarrSeries> = self
            .get(
                instance,
                "api/v3/series",
                &[("includeSeasonImages", "false")],
            )
            .await?;

        // Episode detail requires one request per series; `buffered` bounds the
        // load on Sonarr while keeping the snapshot order deterministic.
        let series = stream::iter(series)
            .map(|series| async move {
                let series_id = series.id.to_string();
                let episodes: Vec<SonarrEpisode> = self
                    .get(
                        instance,
                        "api/v3/episode",
                        &[
                            ("seriesId", series_id.as_str()),
                            ("includeEpisodeFile", "true"),
                        ],
                    )
                    .await?;
                Ok::<_, anyhow::Error>((series, episodes))
            })
            .buffered(MAX_CONCURRENT_EPISODE_REQUESTS)
            .try_collect::<Vec<_>>()
            .await?;

        Ok(Snapshot::Series(
            series
                .into_iter()
                .map(|(series, episodes)| {
                    let statistics = series.statistics.unwrap_or_default();
                    let seasons = series
                        .seasons
                        .into_iter()
                        .filter_map(|season| {
                            let file_count = non_negative(
                                season
                                    .statistics
                                    .and_then(|statistics| statistics.episode_file_count)
                                    .unwrap_or(0),
                            );
                            (season.season_number > 0 && file_count > 0).then_some(
                                SeriesSeasonSnapshot {
                                    season_number: season.season_number,
                                    file_count,
                                },
                            )
                        })
                        .collect();

                    SeriesSnapshot {
                        tvdb_id: series.tvdb_id,
                        title: series.title,
                        year: series.year,
                        size_on_disk_bytes: non_negative(statistics.size_on_disk.unwrap_or(0)),
                        file_count: non_negative(statistics.episode_file_count.unwrap_or(0)),
                        seasons,
                        episodes: episode_snapshots(episodes),
                    }
                })
                .collect(),
        ))
    }

    async fn collect_lidarr(&self, instance: &Instance) -> Result<Snapshot> {
        let (artists, albums): (Vec<LidarrArtist>, Vec<LidarrAlbum>) = tokio::try_join!(
            self.get(instance, "api/v1/artist", &[]),
            self.get(instance, "api/v1/album", &[])
        )?;

        let mut albums_by_artist =
            std::collections::HashMap::<i64, Vec<ArtistAlbumSnapshot>>::new();
        for album in albums {
            let statistics = album.statistics.unwrap_or_default();
            let file_count = non_negative(statistics.track_file_count.unwrap_or(0));
            if file_count > 0 {
                albums_by_artist
                    .entry(album.artist_id)
                    .or_default()
                    .push(ArtistAlbumSnapshot {
                        musicbrainz_id: normalize_musicbrainz_id(&album.foreign_album_id, "album")?,
                        title: album.title,
                        size_on_disk_bytes: non_negative(statistics.size_on_disk.unwrap_or(0)),
                        file_count,
                    });
            }
        }

        let artists = artists
            .into_iter()
            .map(|artist| {
                let statistics = artist.statistics.unwrap_or_default();
                Ok(ArtistSnapshot {
                    musicbrainz_id: normalize_musicbrainz_id(&artist.foreign_artist_id, "artist")?,
                    name: artist.artist_name,
                    size_on_disk_bytes: non_negative(statistics.size_on_disk.unwrap_or(0)),
                    file_count: non_negative(statistics.track_file_count.unwrap_or(0)),
                    albums: albums_by_artist.remove(&artist.id).unwrap_or_default(),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Snapshot::Artists(artists))
    }

    async fn get<T>(&self, instance: &Instance, path: &str, query: &[(&str, &str)]) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let url = endpoint(&instance.base_url, path)?;
        let response = self
            .client
            .get(url)
            .query(query)
            .header("X-Api-Key", &instance.api_key)
            .send()
            .await
            .with_context(|| format!("{} request failed", instance.kind))?;

        let status = response.status();
        if status.is_redirection() {
            let destination = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| Url::parse(value).ok())
                .and_then(|url| url.host_str().map(str::to_owned))
                .unwrap_or_else(|| "another URL".to_owned());
            bail!(
                "{} API request was redirected to {destination}; configure a direct instance URL or bypass the authentication proxy for the Arr API",
                instance.kind
            );
        }
        if !status.is_success() {
            bail!("{} returned HTTP {}", instance.kind, status.as_u16());
        }

        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("unknown");
        if !content_type
            .split(';')
            .next()
            .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("application/json"))
        {
            bail!(
                "{} returned {content_type} instead of application/json",
                instance.kind
            );
        }

        response
            .json()
            .await
            .with_context(|| format!("{} returned an invalid response", instance.kind))
    }
}

fn endpoint(base_url: &Url, path: &str) -> Result<Url> {
    base_url
        .join(path)
        .with_context(|| format!("failed to build endpoint URL for {path}"))
}

fn non_negative(value: i64) -> i64 {
    value.max(0)
}

/// Maps Sonarr episodes to snapshots, excluding specials and deduplicating on
/// (season, episode) so pathological Sonarr data cannot violate the storage
/// primary key. Unaired episodes are kept; the UI renders them distinctly.
fn episode_snapshots(episodes: Vec<SonarrEpisode>) -> Vec<SeriesEpisodeSnapshot> {
    episodes
        .into_iter()
        .filter(|episode| episode.season_number > 0 && episode.episode_number >= 0)
        .map(|episode| {
            (
                (episode.season_number, episode.episode_number),
                SeriesEpisodeSnapshot {
                    season_number: episode.season_number,
                    episode_number: episode.episode_number,
                    title: episode.title,
                    air_date_utc: episode.air_date_utc,
                    has_file: episode.has_file,
                    size_on_disk_bytes: non_negative(
                        episode.episode_file.and_then(|file| file.size).unwrap_or(0),
                    ),
                },
            )
        })
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect()
}

fn normalize_musicbrainz_id(id: &str, entity: &str) -> Result<String> {
    let id = id.trim();
    if id.is_empty() {
        bail!("Lidarr returned an empty MusicBrainz {entity} ID");
    }
    Ok(id.to_ascii_lowercase())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RadarrMovie {
    tmdb_id: i64,
    title: String,
    #[serde(default)]
    year: i64,
    size_on_disk: Option<i64>,
    has_file: Option<bool>,
    /// When the movie was added to Radarr's library. Radarr always sends this
    /// for library movies, but treat it as optional to tolerate older APIs.
    added: Option<DateTime<Utc>>,
    statistics: Option<RadarrStatistics>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RadarrStatistics {
    movie_file_count: Option<i64>,
    size_on_disk: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SonarrSeries {
    /// Sonarr's internal series id, required to fetch the episode list.
    id: i64,
    tvdb_id: i64,
    title: String,
    #[serde(default)]
    year: i64,
    #[serde(default)]
    seasons: Vec<SonarrSeason>,
    statistics: Option<SonarrStatistics>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SonarrSeason {
    season_number: i64,
    statistics: Option<SonarrSeasonStatistics>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SonarrSeasonStatistics {
    episode_file_count: Option<i64>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SonarrStatistics {
    episode_file_count: Option<i64>,
    size_on_disk: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SonarrEpisode {
    season_number: i64,
    episode_number: i64,
    #[serde(default)]
    title: String,
    air_date_utc: Option<DateTime<Utc>>,
    #[serde(default)]
    has_file: bool,
    episode_file: Option<SonarrEpisodeFile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SonarrEpisodeFile {
    size: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LidarrArtist {
    id: i64,
    artist_name: String,
    foreign_artist_id: String,
    statistics: Option<LidarrArtistStatistics>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LidarrArtistStatistics {
    track_file_count: Option<i64>,
    size_on_disk: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LidarrAlbum {
    artist_id: i64,
    foreign_album_id: String,
    #[serde(default)]
    title: String,
    statistics: Option<LidarrAlbumStatistics>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LidarrAlbumStatistics {
    track_file_count: Option<i64>,
    size_on_disk: Option<i64>,
}

#[cfg(test)]
mod tests {
    use url::Url;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path, query_param},
    };

    use crate::instances::{Instance, InstanceKind};

    use super::{ArrClient, Snapshot};

    fn instance(server: &MockServer, kind: InstanceKind, path: &str) -> Instance {
        Instance {
            id: "test".to_owned(),
            name: "Test".to_owned(),
            kind,
            base_url: Url::parse(&format!("{}{path}/", server.uri())).expect("base URL"),
            api_key: "secret".to_owned(),
            config_order: 0,
        }
    }

    #[tokio::test]
    async fn collects_radarr_movies_from_a_base_path() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/radarr/api/v3/movie"))
            .and(query_param("excludeLocalCovers", "true"))
            .and(header("X-Api-Key", "secret"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                    "tmdbId": 42,
                    "title": "Movie",
                    "year": 2024,
                    "added": "2024-03-15T10:00:00Z",
                    "statistics": {"movieFileCount": 2, "sizeOnDisk": 1234}
                }])),
            )
            .mount(&server)
            .await;

        let snapshot = ArrClient::new()
            .expect("client")
            .collect(&instance(&server, InstanceKind::Radarr, "/radarr"))
            .await
            .expect("snapshot");

        let Snapshot::Movies(movies) = snapshot else {
            panic!("expected movies");
        };
        assert_eq!(movies[0].tmdb_id, 42);
        assert_eq!(movies[0].file_count, 2);
        assert_eq!(movies[0].size_on_disk_bytes, 1234);
        assert_eq!(
            movies[0].added_at.map(|added| added.to_rfc3339()),
            Some("2024-03-15T10:00:00+00:00".to_owned())
        );
    }

    #[tokio::test]
    async fn excludes_specials_from_sonarr_season_snapshots() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v3/series"))
            .and(query_param("includeSeasonImages", "false"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                    "id": 9,
                    "tvdbId": 7,
                    "title": "Series",
                    "year": 2020,
                    "statistics": {"episodeFileCount": 4, "sizeOnDisk": 800},
                    "seasons": [
                        {"seasonNumber": 0, "statistics": {"episodeFileCount": 1}},
                        {"seasonNumber": 1, "statistics": {"episodeFileCount": 3}},
                        {"seasonNumber": 2, "statistics": {"episodeFileCount": 0}}
                    ]
                }])),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v3/episode"))
            .and(query_param("seriesId", "9"))
            .and(query_param("includeEpisodeFile", "true"))
            .and(header("X-Api-Key", "secret"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "seasonNumber": 0,
                    "episodeNumber": 1,
                    "title": "Special",
                    "hasFile": true,
                    "episodeFile": {"size": 100}
                },
                {
                    "seasonNumber": 1,
                    "episodeNumber": 2,
                    "title": "Aired, missing",
                    "airDateUtc": "2020-01-08T02:00:00Z",
                    "hasFile": false
                },
                {
                    "seasonNumber": 1,
                    "episodeNumber": 1,
                    "title": "On disk",
                    "airDateUtc": "2020-01-01T02:00:00Z",
                    "hasFile": true,
                    "episodeFile": {"size": 512}
                },
                {
                    "seasonNumber": 2,
                    "episodeNumber": 1,
                    "title": "",
                    "hasFile": false
                }
            ])))
            .mount(&server)
            .await;

        let snapshot = ArrClient::new()
            .expect("client")
            .collect(&instance(&server, InstanceKind::Sonarr, ""))
            .await
            .expect("snapshot");

        let Snapshot::Series(series) = snapshot else {
            panic!("expected series");
        };
        assert_eq!(series[0].seasons.len(), 1);
        assert_eq!(series[0].seasons[0].season_number, 1);

        // Specials are excluded; the rest are ordered by (season, episode).
        let episodes = &series[0].episodes;
        assert_eq!(episodes.len(), 3);
        assert_eq!(episodes[0].season_number, 1);
        assert_eq!(episodes[0].episode_number, 1);
        assert!(episodes[0].has_file);
        assert_eq!(episodes[0].size_on_disk_bytes, 512);
        assert_eq!(episodes[1].title, "Aired, missing");
        assert!(!episodes[1].has_file);
        assert_eq!(episodes[1].size_on_disk_bytes, 0);
        assert_eq!(
            episodes[1].air_date_utc.map(|date| date.to_rfc3339()),
            Some("2020-01-08T02:00:00+00:00".to_owned())
        );
        // Unaired (no air date yet) episodes are retained.
        assert_eq!(episodes[2].season_number, 2);
        assert_eq!(episodes[2].air_date_utc, None);
    }

    #[tokio::test]
    async fn combines_lidarr_artist_and_album_responses() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/artist"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                    "id": 3,
                    "artistName": "Artist",
                    "foreignArtistId": "ARTIST-ID",
                    "statistics": {"trackFileCount": 5, "sizeOnDisk": 1000}
                }])),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v1/album"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "artistId": 3,
                    "foreignAlbumId": "ALBUM-ONE",
                    "title": "Album One",
                    "statistics": {"trackFileCount": 5, "sizeOnDisk": 800}
                },
                {
                    "artistId": 3,
                    "foreignAlbumId": "ALBUM-TWO",
                    "statistics": {"trackFileCount": 0}
                }
            ])))
            .mount(&server)
            .await;

        let snapshot = ArrClient::new()
            .expect("client")
            .collect(&instance(&server, InstanceKind::Lidarr, ""))
            .await
            .expect("snapshot");

        let Snapshot::Artists(artists) = snapshot else {
            panic!("expected artists");
        };
        assert_eq!(artists[0].musicbrainz_id, "artist-id");
        assert_eq!(artists[0].albums.len(), 1);
        assert_eq!(artists[0].albums[0].musicbrainz_id, "album-one");
        assert_eq!(artists[0].albums[0].title, "Album One");
        assert_eq!(artists[0].albums[0].size_on_disk_bytes, 800);
    }

    #[tokio::test]
    async fn rejects_unsuccessful_and_malformed_responses() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v3/movie"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let error = ArrClient::new()
            .expect("client")
            .collect(&instance(&server, InstanceKind::Radarr, ""))
            .await
            .expect_err("HTTP error");
        assert!(error.to_string().contains("HTTP 503"));

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v3/movie"))
            .respond_with(ResponseTemplate::new(200).set_body_raw("not json", "application/json"))
            .mount(&server)
            .await;

        let error = ArrClient::new()
            .expect("client")
            .collect(&instance(&server, InstanceKind::Radarr, ""))
            .await
            .expect_err("malformed response");
        assert!(
            error.to_string().contains("invalid response"),
            "unexpected error: {error:#}"
        );
    }

    #[tokio::test]
    async fn rejects_authentication_proxy_redirects_without_following_them() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v3/movie"))
            .respond_with(
                ResponseTemplate::new(302)
                    .insert_header("Location", "https://auth.example.test/login"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let error = ArrClient::new()
            .expect("client")
            .collect(&instance(&server, InstanceKind::Radarr, ""))
            .await
            .expect_err("redirect");

        assert!(
            error
                .to_string()
                .contains("redirected to auth.example.test")
        );
    }
}
