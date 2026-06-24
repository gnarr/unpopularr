use std::{
    collections::{BTreeMap, HashMap, HashSet},
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::{StreamExt, TryStreamExt, stream};
use reqwest::{
    Client, Url,
    header::{CONTENT_TYPE, LOCATION},
    redirect::Policy,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};

use crate::playback::{
    ContentKey, PlaybackAggregate, PlaybackSnapshot, PlaybackSource, PlaybackSourceClient,
};

const HISTORY_PAGE_SIZE: i64 = 1_000;
const MAX_CONCURRENT_METADATA_REQUESTS: usize = 8;

#[derive(Clone)]
pub struct TautulliClient {
    client: Client,
}

impl TautulliClient {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(Policy::none())
            .user_agent(concat!("unpopularr/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("failed to create Tautulli HTTP client")?;
        Ok(Self { client })
    }

    async fn history(&self, source: &PlaybackSource) -> Result<Vec<HistoryEntry>> {
        let mut start = 0_i64;
        let mut history = Vec::new();

        loop {
            let page: HistoryPage = self
                .get(
                    source,
                    &[
                        ("cmd", "get_history".to_owned()),
                        ("grouping", "1".to_owned()),
                        ("include_activity", "0".to_owned()),
                        ("order_column", "date".to_owned()),
                        ("order_dir", "asc".to_owned()),
                        ("start", start.to_string()),
                        ("length", HISTORY_PAGE_SIZE.to_string()),
                    ],
                )
                .await?;
            let page_length =
                i64::try_from(page.data.len()).map_err(|_| anyhow!("history page is too large"))?;
            history.extend(page.data.into_iter().filter_map(HistoryEntry::from_row));
            start = start.saturating_add(page_length);

            if page_length == 0
                || page_length < HISTORY_PAGE_SIZE
                || start >= page.records_filtered.max(0)
            {
                break;
            }
        }

        Ok(history)
    }

    async fn metadata(&self, source: &PlaybackSource, rating_key: i64) -> Result<Metadata> {
        self.get(
            source,
            &[
                ("cmd", "get_metadata".to_owned()),
                ("rating_key", rating_key.to_string()),
            ],
        )
        .await
    }

    async fn get<T>(&self, source: &PlaybackSource, query: &[(&str, String)]) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let command = query
            .iter()
            .find_map(|(name, value)| (*name == "cmd").then_some(value.as_str()))
            .unwrap_or("unknown command");
        let url = endpoint(&source.base_url)?;
        let response = self
            .client
            .get(url)
            .query(&[("apikey", source.api_key.as_str())])
            .query(query)
            .send()
            .await
            .map_err(|error| request_error(&source.base_url, &error))?;

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
                "Tautulli API request was redirected to {destination}; configure a direct URL or bypass the authentication proxy"
            );
        }
        if !status.is_success() {
            bail!("Tautulli returned HTTP {}", status.as_u16());
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
            bail!("Tautulli returned {content_type} instead of application/json");
        }

        let body = response
            .bytes()
            .await
            .map_err(|_| anyhow!("Tautulli response could not be read"))?;
        let envelope: ApiEnvelope<serde_json::Value> = serde_json::from_slice(&body)
            .map_err(|error| anyhow!("Tautulli {command} response envelope is invalid: {error}"))?;
        if envelope.response.result != "success" {
            bail!("Tautulli API request failed");
        }
        serde_json::from_value(envelope.response.data)
            .map_err(|error| anyhow!("Tautulli {command} response data is invalid: {error}"))
    }
}

#[async_trait]
impl PlaybackSourceClient for TautulliClient {
    async fn collect(&self, source: &PlaybackSource) -> Result<PlaybackSnapshot> {
        let history = self.history(source).await?;
        let lookup_keys = history
            .iter()
            .filter_map(|entry| entry.lookup_key)
            .collect::<HashSet<_>>();
        let metadata = stream::iter(lookup_keys)
            .map(|lookup_key| async move {
                let metadata = self.metadata(source, lookup_key.rating_key).await?;
                Ok::<_, anyhow::Error>((lookup_key, metadata.content_key(lookup_key.kind)))
            })
            .buffer_unordered(MAX_CONCURRENT_METADATA_REQUESTS)
            .try_collect::<HashMap<_, _>>()
            .await?;

        let mut aggregates = BTreeMap::<ContentKey, AggregateValues>::new();
        let mut matched_history_rows = 0_i64;
        let mut unmatched_history_rows = 0_i64;

        for entry in history {
            let content_key = entry
                .lookup_key
                .and_then(|lookup_key| metadata.get(&lookup_key))
                .and_then(Clone::clone);
            let Some(content_key) = content_key else {
                unmatched_history_rows = unmatched_history_rows.saturating_add(1);
                continue;
            };

            matched_history_rows = matched_history_rows.saturating_add(1);
            let aggregate = aggregates.entry(content_key).or_default();
            aggregate.play_count = aggregate.play_count.saturating_add(entry.play_count);
            aggregate.play_duration_seconds = aggregate
                .play_duration_seconds
                .saturating_add(entry.play_duration_seconds);
            aggregate.last_played_at = match (aggregate.last_played_at, entry.last_played_at) {
                (Some(current), Some(candidate)) => Some(current.max(candidate)),
                (None, candidate) => candidate,
                (current, None) => current,
            };
        }

        Ok(PlaybackSnapshot {
            aggregates: aggregates
                .into_iter()
                .map(|(key, values)| PlaybackAggregate {
                    key,
                    play_count: values.play_count,
                    play_duration_seconds: values.play_duration_seconds,
                    last_played_at: values.last_played_at,
                })
                .collect(),
            matched_history_rows,
            unmatched_history_rows,
        })
    }
}

fn endpoint(base_url: &Url) -> Result<Url> {
    base_url
        .join("api/v2")
        .map_err(|_| anyhow!("failed to build Tautulli API URL"))
}

fn request_error(base_url: &Url, error: &reqwest::Error) -> anyhow::Error {
    let destination = match (base_url.host_str(), base_url.port_or_known_default()) {
        (Some(host), Some(port)) => format!("{host}:{port}"),
        (Some(host), None) => host.to_owned(),
        (None, _) => "configured host".to_owned(),
    };
    let reason = if error.is_timeout() {
        "request timed out"
    } else if error.is_connect() {
        "connection failed"
    } else if error.is_request() {
        "request could not be constructed"
    } else {
        "transport error"
    };

    anyhow!("Tautulli request to {destination} failed: {reason}")
}

#[derive(Deserialize)]
struct ApiEnvelope<T> {
    response: ApiResponse<T>,
}

#[derive(Deserialize)]
struct ApiResponse<T> {
    result: String,
    data: T,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryPage {
    #[serde(deserialize_with = "deserialize_i64")]
    records_filtered: i64,
    #[serde(default)]
    data: Vec<HistoryRow>,
}

#[derive(Deserialize)]
struct HistoryRow {
    media_type: String,
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    rating_key: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    grandparent_rating_key: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    group_count: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    play_duration: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    started: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    stopped: Option<i64>,
}

struct HistoryEntry {
    lookup_key: Option<LookupKey>,
    play_count: i64,
    play_duration_seconds: i64,
    last_played_at: Option<DateTime<Utc>>,
}

impl HistoryEntry {
    fn from_row(row: HistoryRow) -> Option<Self> {
        let (kind, rating_key) = match row.media_type.as_str() {
            "movie" => (TopLevelKind::Movie, row.rating_key),
            "episode" => (TopLevelKind::Series, row.grandparent_rating_key),
            "track" => (TopLevelKind::Artist, row.grandparent_rating_key),
            _ => return None,
        };
        let lookup_key = rating_key
            .filter(|rating_key| *rating_key > 0)
            .map(|rating_key| LookupKey { kind, rating_key });
        let last_played_at = row
            .stopped
            .filter(|timestamp| *timestamp > 0)
            .or_else(|| row.started.filter(|timestamp| *timestamp > 0))
            .and_then(|timestamp| DateTime::from_timestamp(timestamp, 0));

        Some(Self {
            lookup_key,
            play_count: row.group_count.unwrap_or(1).max(1),
            play_duration_seconds: row.play_duration.unwrap_or(0).max(0),
            last_played_at,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct LookupKey {
    kind: TopLevelKind,
    rating_key: i64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum TopLevelKind {
    Movie,
    Series,
    Artist,
}

#[derive(Deserialize)]
struct Metadata {
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    guid: String,
    #[serde(default, deserialize_with = "deserialize_string_list")]
    guids: Vec<String>,
}

impl Metadata {
    fn content_key(&self, kind: TopLevelKind) -> Option<ContentKey> {
        self.guids
            .iter()
            .map(String::as_str)
            .chain((!self.guid.is_empty()).then_some(self.guid.as_str()))
            .find_map(|guid| parse_content_key(kind, guid))
    }
}

fn parse_content_key(kind: TopLevelKind, guid: &str) -> Option<ContentKey> {
    match kind {
        TopLevelKind::Movie => parse_numeric_guid(
            guid,
            &[
                "tmdb://",
                "themoviedb://",
                "com.plexapp.agents.themoviedb://",
            ],
        )
        .map(ContentKey::Movie),
        TopLevelKind::Series => parse_numeric_guid(
            guid,
            &[
                "tvdb://",
                "thetvdb://",
                "thetvdbdvdorder://",
                "com.plexapp.agents.thetvdb://",
            ],
        )
        .map(ContentKey::Series),
        TopLevelKind::Artist => guid
            .strip_prefix("mbid://")
            .and_then(first_guid_component)
            .filter(|id| !id.is_empty())
            .map(|id| ContentKey::Artist(id.to_ascii_lowercase())),
    }
}

fn parse_numeric_guid(guid: &str, prefixes: &[&str]) -> Option<i64> {
    prefixes
        .iter()
        .find_map(|prefix| guid.strip_prefix(prefix))
        .and_then(first_guid_component)
        .and_then(|id| id.parse().ok())
        .filter(|id| *id > 0)
}

fn first_guid_component(value: &str) -> Option<&str> {
    value.split(['/', '?']).next()
}

fn deserialize_i64<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_optional_i64(deserializer).map(Option::unwrap_or_default)
}

fn deserialize_optional_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(number)) => number
            .as_i64()
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom("number is outside the i64 range")),
        Some(serde_json::Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                Ok(None)
            } else {
                value.parse().map(Some).map_err(serde::de::Error::custom)
            }
        }
        Some(_) => Err(serde::de::Error::custom(
            "expected an integer, integer string, null, or empty string",
        )),
    }
}

fn deserialize_optional_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(|value| value.unwrap_or_default())
}

fn deserialize_string_list<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<Vec<String>>::deserialize(deserializer).map(Option::unwrap_or_default)
}

#[derive(Default)]
struct AggregateValues {
    play_count: i64,
    play_duration_seconds: i64,
    last_played_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use url::Url;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path, query_param},
    };

    use crate::playback::{ContentKey, PlaybackProvider, PlaybackSource, PlaybackSourceClient};

    use super::TautulliClient;

    fn source(server: &MockServer, base_path: &str) -> PlaybackSource {
        PlaybackSource {
            id: "plex".to_owned(),
            provider: PlaybackProvider::Tautulli,
            base_url: Url::parse(&format!("{}{base_path}/", server.uri())).expect("URL"),
            api_key: "secret".to_owned(),
        }
    }

    fn envelope(data: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "response": {
                "result": "success",
                "message": null,
                "data": data
            }
        })
    }

    #[tokio::test]
    async fn aggregates_movies_series_and_artists_from_a_base_path() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tautulli/api/v2"))
            .and(query_param("apikey", "secret"))
            .and(query_param("cmd", "get_history"))
            .and(query_param("grouping", "1"))
            .and(query_param("include_activity", "0"))
            .and(query_param("order_column", "date"))
            .and(query_param("order_dir", "asc"))
            .and(query_param("start", "0"))
            .and(query_param("length", "1000"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(envelope(serde_json::json!({
                    "recordsFiltered": 4,
                    "data": [
                        {
                            "media_type": "movie",
                            "rating_key": 10,
                            "group_count": 2,
                            "play_duration": 120,
                            "started": 100,
                            "stopped": 200,
                            "user": "not retained",
                            "ip_address": "not retained"
                        },
                        {
                            "media_type": "episode",
                            "grandparent_rating_key": 20,
                            "group_count": 1,
                            "play_duration": 60,
                            "started": 300
                        },
                        {
                            "media_type": "track",
                            "grandparent_rating_key": 30,
                            "group_count": 1,
                            "play_duration": 30,
                            "stopped": 400
                        },
                        {
                            "media_type": "live",
                            "rating_key": 40
                        }
                    ]
                }))),
            )
            .mount(&server)
            .await;

        for (rating_key, metadata) in [
            (
                10,
                serde_json::json!({"guid": "com.plexapp.agents.themoviedb://42?lang=en"}),
            ),
            (
                20,
                serde_json::json!({"guid": "plex://show/a", "guids": ["tvdb://7"]}),
            ),
            (
                30,
                serde_json::json!({"guid": "plex://artist/a", "guids": ["mbid://ARTIST-ID"]}),
            ),
        ] {
            Mock::given(method("GET"))
                .and(path("/tautulli/api/v2"))
                .and(query_param("cmd", "get_metadata"))
                .and(query_param("rating_key", rating_key.to_string()))
                .respond_with(ResponseTemplate::new(200).set_body_json(envelope(metadata)))
                .mount(&server)
                .await;
        }

        let snapshot = TautulliClient::new()
            .expect("client")
            .collect(&source(&server, "/tautulli"))
            .await
            .expect("snapshot");

        assert_eq!(snapshot.matched_history_rows, 3);
        assert_eq!(snapshot.unmatched_history_rows, 0);
        assert_eq!(snapshot.aggregates.len(), 3);
        assert_eq!(snapshot.aggregates[0].key, ContentKey::Movie(42));
        assert_eq!(snapshot.aggregates[0].play_count, 2);
        assert_eq!(snapshot.aggregates[1].key, ContentKey::Series(7));
        assert_eq!(
            snapshot.aggregates[2].key,
            ContentKey::Artist("artist-id".to_owned())
        );
    }

    #[tokio::test]
    async fn accepts_string_and_null_values_from_tautulli() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2"))
            .and(query_param("cmd", "get_history"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(envelope(serde_json::json!({
                    "recordsFiltered": "2",
                    "data": [
                        {
                            "media_type": "movie",
                            "rating_key": "10",
                            "grandparent_rating_key": "",
                            "group_count": "2",
                            "play_duration": "120",
                            "started": null,
                            "stopped": "200"
                        },
                        {
                            "media_type": "episode",
                            "rating_key": "",
                            "grandparent_rating_key": null
                        }
                    ]
                }))),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v2"))
            .and(query_param("cmd", "get_metadata"))
            .and(query_param("rating_key", "10"))
            .respond_with(ResponseTemplate::new(200).set_body_json(envelope(
                serde_json::json!({"guid": null, "guids": ["tmdb://42"]}),
            )))
            .mount(&server)
            .await;

        let snapshot = TautulliClient::new()
            .expect("client")
            .collect(&source(&server, ""))
            .await
            .expect("snapshot");

        assert_eq!(snapshot.matched_history_rows, 1);
        assert_eq!(snapshot.unmatched_history_rows, 1);
        assert_eq!(snapshot.aggregates[0].key, ContentKey::Movie(42));
        assert_eq!(snapshot.aggregates[0].play_count, 2);
        assert_eq!(snapshot.aggregates[0].play_duration_seconds, 120);
    }

    #[tokio::test]
    async fn reports_history_without_supported_guids_as_unmatched() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2"))
            .and(query_param("cmd", "get_history"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(envelope(serde_json::json!({
                    "recordsFiltered": 2,
                    "data": [
                        {"media_type": "movie", "rating_key": 10},
                        {"media_type": "episode"}
                    ]
                }))),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v2"))
            .and(query_param("cmd", "get_metadata"))
            .and(query_param("rating_key", "10"))
            .respond_with(ResponseTemplate::new(200).set_body_json(envelope(
                serde_json::json!({"guid": "plex://movie/no-external-id"}),
            )))
            .mount(&server)
            .await;

        let snapshot = TautulliClient::new()
            .expect("client")
            .collect(&source(&server, ""))
            .await
            .expect("snapshot");
        assert_eq!(snapshot.matched_history_rows, 0);
        assert_eq!(snapshot.unmatched_history_rows, 2);
    }

    #[tokio::test]
    async fn reports_connection_failures_without_exposing_the_api_key() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("temporary listener");
        let address = listener.local_addr().expect("listener address");
        drop(listener);
        let source = PlaybackSource {
            id: "plex".to_owned(),
            provider: PlaybackProvider::Tautulli,
            base_url: Url::parse(&format!("http://{address}/")).expect("URL"),
            api_key: "must-not-appear".to_owned(),
        };

        let error = TautulliClient::new()
            .expect("client")
            .collect(&source)
            .await
            .expect_err("connection failure");
        let message = error.to_string();

        assert!(message.contains(&address.to_string()));
        assert!(message.contains("connection failed"));
        assert!(!message.contains("must-not-appear"));
    }

    #[tokio::test]
    async fn reads_all_history_pages() {
        let server = MockServer::start().await;
        let first_page = vec![serde_json::json!({"media_type": "live"}); 1_000];
        Mock::given(method("GET"))
            .and(path("/api/v2"))
            .and(query_param("cmd", "get_history"))
            .and(query_param("start", "0"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(envelope(serde_json::json!({
                    "recordsFiltered": 1001,
                    "data": first_page
                }))),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v2"))
            .and(query_param("cmd", "get_history"))
            .and(query_param("start", "1000"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(envelope(serde_json::json!({
                    "recordsFiltered": 1001,
                    "data": [{"media_type": "movie", "rating_key": 10}]
                }))),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v2"))
            .and(query_param("cmd", "get_metadata"))
            .and(query_param("rating_key", "10"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(envelope(serde_json::json!({"guids": ["tmdb://42"]}))),
            )
            .mount(&server)
            .await;

        let snapshot = TautulliClient::new()
            .expect("client")
            .collect(&source(&server, ""))
            .await
            .expect("snapshot");
        assert_eq!(snapshot.matched_history_rows, 1);
        assert_eq!(snapshot.aggregates[0].key, ContentKey::Movie(42));
    }

    #[tokio::test]
    async fn rejects_redirects_and_invalid_api_envelopes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2"))
            .respond_with(
                ResponseTemplate::new(302)
                    .insert_header("Location", "https://auth.example.test/login"),
            )
            .mount(&server)
            .await;
        let error = TautulliClient::new()
            .expect("client")
            .collect(&source(&server, ""))
            .await
            .expect_err("redirect");
        assert!(error.to_string().contains("auth.example.test"));

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "response": {"result": "error", "message": "secret details", "data": {}}
            })))
            .mount(&server)
            .await;
        let error = TautulliClient::new()
            .expect("client")
            .collect(&source(&server, ""))
            .await
            .expect_err("API failure");
        assert_eq!(error.to_string(), "Tautulli API request failed");
    }

    #[tokio::test]
    async fn rejects_http_errors_and_malformed_responses_without_exposing_the_key() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let error = TautulliClient::new()
            .expect("client")
            .collect(&source(&server, ""))
            .await
            .expect_err("HTTP failure");
        assert!(error.to_string().contains("HTTP 401"));
        assert!(!format!("{error:#}").contains("secret"));

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2"))
            .respond_with(ResponseTemplate::new(200).set_body_raw("not json", "application/json"))
            .mount(&server)
            .await;
        let error = TautulliClient::new()
            .expect("client")
            .collect(&source(&server, ""))
            .await
            .expect_err("invalid response");
        assert!(
            error
                .to_string()
                .starts_with("Tautulli get_history response envelope is invalid:")
        );
        assert!(!format!("{error:#}").contains("secret"));

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2"))
            .respond_with(ResponseTemplate::new(200).set_body_raw("<html></html>", "text/html"))
            .mount(&server)
            .await;
        let error = TautulliClient::new()
            .expect("client")
            .collect(&source(&server, ""))
            .await
            .expect_err("unexpected content type");
        assert!(error.to_string().contains("instead of application/json"));
    }
}
