use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Serialize;
use tower_http::trace::TraceLayer;
use tracing::error;

use crate::{
    catalog::CatalogService,
    collection::{StartSync, SyncService, SyncTrigger},
    instances::{Instance, InstanceKind},
    playback::{PlaybackService, PlaybackSyncTrigger, StartPlaybackSync},
    web_assets::spa_fallback,
};

#[derive(Clone)]
pub struct AppState {
    pub catalog: CatalogService,
    pub sync: SyncService,
    pub playback: Option<PlaybackService>,
    /// Configured instances, used to serve browser-facing deep-link URLs.
    pub instances: Arc<Vec<Instance>>,
}

pub fn router(state: AppState) -> Router {
    let playback_enabled = state.playback.is_some();
    let api = Router::new()
        .route("/v1/content", get(all_content))
        .route("/v1/instances", get(instances))
        .route("/v1/series/{tvdb_id}", get(series_details))
        .route("/v1/movies/{tmdb_id}", get(movie_details))
        .route("/v1/artists/{musicbrainz_id}", get(artist_details))
        .route("/v1/sync", get(sync_status).post(start_sync));
    let api = if playback_enabled {
        api.route(
            "/v1/playback/sync",
            get(playback_sync_status).post(start_playback_sync),
        )
    } else {
        api
    };
    // Unknown `/api/*` paths return a JSON 404 instead of falling through to the
    // SPA shell, so API clients never receive HTML.
    let api = api.fallback(api_not_found);

    Router::new()
        .nest("/api", api)
        .fallback(spa_fallback)
        .layer(TraceLayer::new_for_http())
        .with_state(Arc::new(state))
}

async fn api_not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse { error: "not found" }),
    )
        .into_response()
}

async fn all_content(State(state): State<Arc<AppState>>) -> Response {
    match state.catalog.all_content().await {
        Ok(content) => Json(content).into_response(),
        Err(error) => internal_error(error),
    }
}

/// Browser-facing metadata for one configured instance. The frontend joins
/// `external_url` with a per-item path to build deep links into the *arr web
/// UI. Deliberately excludes the API key.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstanceLink {
    id: String,
    kind: InstanceKind,
    external_url: String,
}

async fn instances(State(state): State<Arc<AppState>>) -> Response {
    let links: Vec<InstanceLink> = state
        .instances
        .iter()
        .map(|instance| InstanceLink {
            id: instance.id.clone(),
            kind: instance.kind,
            external_url: instance.web_url().to_string(),
        })
        .collect();
    Json(links).into_response()
}

async fn series_details(State(state): State<Arc<AppState>>, Path(tvdb_id): Path<i64>) -> Response {
    match state.catalog.series_details(tvdb_id).await {
        Ok(Some(details)) => Json(details).into_response(),
        Ok(None) => api_not_found().await,
        Err(error) => internal_error(error),
    }
}

async fn movie_details(State(state): State<Arc<AppState>>, Path(tmdb_id): Path<i64>) -> Response {
    match state.catalog.movie_details(tmdb_id).await {
        Ok(Some(details)) => Json(details).into_response(),
        Ok(None) => api_not_found().await,
        Err(error) => internal_error(error),
    }
}

async fn artist_details(
    State(state): State<Arc<AppState>>,
    Path(musicbrainz_id): Path<String>,
) -> Response {
    // Stored IDs are lowercased by the Lidarr sync; normalize the URL form so
    // hand-typed uppercase MBIDs still resolve.
    let musicbrainz_id = musicbrainz_id.to_ascii_lowercase();
    match state.catalog.artist_details(&musicbrainz_id).await {
        Ok(Some(details)) => Json(details).into_response(),
        Ok(None) => api_not_found().await,
        Err(error) => internal_error(error),
    }
}

async fn start_sync(State(state): State<Arc<AppState>>) -> Response {
    match state.sync.start(SyncTrigger::Manual).await {
        Ok(StartSync::Started(run)) => (StatusCode::ACCEPTED, Json(run)).into_response(),
        Ok(StartSync::AlreadyRunning(Some(run))) => {
            (StatusCode::CONFLICT, Json(run)).into_response()
        }
        Ok(StartSync::AlreadyRunning(None)) => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "a sync is already running",
            }),
        )
            .into_response(),
        Err(error) => internal_error(error),
    }
}

async fn sync_status(State(state): State<Arc<AppState>>) -> Response {
    match state.sync.active_or_latest().await {
        Ok(Some(run)) => Json(run).into_response(),
        Ok(None) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => internal_error(error),
    }
}

async fn start_playback_sync(State(state): State<Arc<AppState>>) -> Response {
    let Some(playback) = &state.playback else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match playback.start(PlaybackSyncTrigger::Manual).await {
        Ok(StartPlaybackSync::Started(run)) => (StatusCode::ACCEPTED, Json(run)).into_response(),
        Ok(StartPlaybackSync::AlreadyRunning(Some(run))) => {
            (StatusCode::CONFLICT, Json(run)).into_response()
        }
        Ok(StartPlaybackSync::AlreadyRunning(None)) => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "a playback sync is already running",
            }),
        )
            .into_response(),
        Err(error) => internal_error(error),
    }
}

async fn playback_sync_status(State(state): State<Arc<AppState>>) -> Response {
    let Some(playback) = &state.playback else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match playback.active_or_latest().await {
        Ok(Some(run)) => Json(run).into_response(),
        Ok(None) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => internal_error(error),
    }
}

fn internal_error(error: anyhow::Error) -> Response {
    error!(error = %error, "request failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: "internal server error",
        }),
    )
        .into_response()
}

#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use serde_json::Value;
    use tower::ServiceExt;
    use url::Url;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{path, query_param},
    };

    use crate::{
        catalog::{CatalogRepository, CatalogService, adapters::sqlite::SqliteCatalogRepository},
        collection::{
            CollectionRepository, SyncService, adapters::arr::ArrClient,
            adapters::sqlite::SqliteCollectionRepository,
        },
        database,
        instances::{Instance, InstanceKind},
        playback::{
            PlaybackProvider, PlaybackRepository, PlaybackService, PlaybackSource,
            adapters::{sqlite::SqlitePlaybackRepository, tautulli::TautulliClient},
        },
    };

    use super::{AppState, router};

    #[tokio::test]
    async fn sync_endpoints_report_conflicts_and_populate_content() {
        let server = MockServer::start().await;
        Mock::given(path("/api/v3/movie"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(100))
                    .set_body_json(serde_json::json!([{
                        "tmdbId": 42,
                        "title": "Movie",
                        "year": 2024,
                        "statistics": {"movieFileCount": 1, "sizeOnDisk": 100}
                    }])),
            )
            .mount(&server)
            .await;

        let instance = Instance {
            id: "radarr".to_owned(),
            name: "Radarr".to_owned(),
            kind: InstanceKind::Radarr,
            base_url: Url::parse(&format!("{}/", server.uri())).expect("URL"),
            external_url: None,
            api_key: "secret".to_owned(),
            config_order: 0,
        };
        let pool = database::test_pool().await;
        let collection_repository = Arc::new(SqliteCollectionRepository::new(pool.clone()));
        collection_repository
            .reconcile_instances(std::slice::from_ref(&instance))
            .await
            .expect("reconcile instances");
        let collection_port: Arc<dyn CollectionRepository> = collection_repository;
        let catalog_port: Arc<dyn CatalogRepository> =
            Arc::new(SqliteCatalogRepository::new(pool.clone()));
        let application = router(AppState {
            catalog: CatalogService::new(catalog_port),
            sync: SyncService::new(
                collection_port,
                ArrClient::new().expect("Arr client"),
                Arc::new(vec![instance]),
            ),
            playback: None,
            instances: Arc::new(Vec::new()),
        });

        let no_sync = application
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/sync")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(no_sync.status(), StatusCode::NO_CONTENT);

        let started = application
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sync")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(started.status(), StatusCode::ACCEPTED);

        let conflict = application
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sync")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(conflict.status(), StatusCode::CONFLICT);

        let completed = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let response = application
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri("/api/v1/sync")
                            .body(Body::empty())
                            .expect("request"),
                    )
                    .await
                    .expect("response");
                let body = to_bytes(response.into_body(), usize::MAX)
                    .await
                    .expect("response body");
                let run: Value = serde_json::from_slice(&body).expect("sync JSON");
                if run["status"] == "succeeded" {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await;
        assert!(completed.is_ok(), "sync did not complete");

        let response = application
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/content")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        let content: Value = serde_json::from_slice(&body).expect("content JSON");
        assert_eq!(content[0]["contentType"], "movie");
        assert_eq!(content[0]["tmdbId"], 42);
        assert_eq!(content[0]["instances"][0]["id"], "radarr");
        assert!(content[0]["playback"].is_null());

        sqlx::query(
            r#"
            INSERT INTO playback_sources (id, provider, last_successful_sync_at)
            VALUES ('plex', 'tautulli', ?)
            "#,
        )
        .bind(chrono::Utc::now())
        .execute(&pool)
        .await
        .expect("insert playback source");
        sqlx::query(
            r#"
            INSERT INTO playback_snapshots (
                source_id, content_type, content_id, play_count,
                play_duration_seconds, last_played_at
            )
            VALUES ('plex', 'movie', '42', 4, 7200, ?)
            "#,
        )
        .bind(chrono::Utc::now())
        .execute(&pool)
        .await
        .expect("insert playback snapshot");

        let response = application
            .oneshot(
                Request::builder()
                    .uri("/api/v1/content")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        let content: Value = serde_json::from_slice(&body).expect("content JSON");
        assert_eq!(content[0]["playback"]["playCount"], 4);
        assert_eq!(content[0]["playback"]["playDurationSeconds"], 7200);
    }

    #[tokio::test]
    async fn playback_sync_routes_are_optional_and_report_conflicts() {
        let pool = database::test_pool().await;
        let collection_repository = Arc::new(SqliteCollectionRepository::new(pool.clone()));
        let collection_port: Arc<dyn CollectionRepository> = collection_repository;
        let catalog_port: Arc<dyn CatalogRepository> =
            Arc::new(SqliteCatalogRepository::new(pool.clone()));
        let disabled = router(AppState {
            catalog: CatalogService::new(Arc::clone(&catalog_port)),
            sync: SyncService::new(
                Arc::clone(&collection_port),
                ArrClient::new().expect("Arr client"),
                Arc::new(Vec::new()),
            ),
            playback: None,
            instances: Arc::new(Vec::new()),
        });
        let response = disabled
            .oneshot(
                Request::builder()
                    .uri("/api/v1/playback/sync")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let server = MockServer::start().await;
        Mock::given(path("/api/v2"))
            .and(query_param("cmd", "get_history"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(100))
                    .set_body_json(serde_json::json!({
                        "response": {
                            "result": "success",
                            "message": null,
                            "data": {"recordsFiltered": 0, "data": []}
                        }
                    })),
            )
            .mount(&server)
            .await;

        let source = Arc::new(PlaybackSource {
            id: "plex".to_owned(),
            provider: PlaybackProvider::Tautulli,
            base_url: Url::parse(&format!("{}/", server.uri())).expect("URL"),
            api_key: "secret".to_owned(),
        });
        let playback_repository = Arc::new(SqlitePlaybackRepository::new(pool));
        playback_repository
            .reconcile_source(Some(&source))
            .await
            .expect("reconcile playback");
        let playback_port: Arc<dyn PlaybackRepository> = playback_repository;
        let enabled = router(AppState {
            catalog: CatalogService::new(catalog_port),
            sync: SyncService::new(
                collection_port,
                ArrClient::new().expect("Arr client"),
                Arc::new(Vec::new()),
            ),
            playback: Some(PlaybackService::new(
                playback_port,
                Arc::new(TautulliClient::new().expect("Tautulli client")),
                source,
            )),
            instances: Arc::new(Vec::new()),
        });

        let no_sync = enabled
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/playback/sync")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(no_sync.status(), StatusCode::NO_CONTENT);

        let started = enabled
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/playback/sync")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(started.status(), StatusCode::ACCEPTED);

        let conflict = enabled
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/playback/sync")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(conflict.status(), StatusCode::CONFLICT);

        let completed = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let response = enabled
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri("/api/v1/playback/sync")
                            .body(Body::empty())
                            .expect("request"),
                    )
                    .await
                    .expect("response");
                let body = to_bytes(response.into_body(), usize::MAX)
                    .await
                    .expect("response body");
                let run: Value = serde_json::from_slice(&body).expect("sync JSON");
                if run["status"] == "succeeded" {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await;
        assert!(completed.is_ok(), "playback sync did not complete");
    }

    async fn minimal_app() -> axum::Router {
        minimal_app_with_pool().await.0
    }

    async fn minimal_app_with_pool() -> (axum::Router, sqlx::SqlitePool) {
        let pool = database::test_pool().await;
        let collection_port: Arc<dyn CollectionRepository> =
            Arc::new(SqliteCollectionRepository::new(pool.clone()));
        let catalog_port: Arc<dyn CatalogRepository> =
            Arc::new(SqliteCatalogRepository::new(pool.clone()));
        let app = router(AppState {
            catalog: CatalogService::new(catalog_port),
            sync: SyncService::new(
                collection_port,
                ArrClient::new().expect("Arr client"),
                Arc::new(Vec::new()),
            ),
            playback: None,
            instances: Arc::new(Vec::new()),
        });
        (app, pool)
    }

    fn content_type_of(response: &axum::response::Response) -> String {
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned()
    }

    #[tokio::test]
    async fn unknown_api_path_returns_json_not_found() {
        let response = minimal_app()
            .await
            .oneshot(
                Request::builder()
                    .uri("/api/v1/does-not-exist")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(
            content_type_of(&response).starts_with("application/json"),
            "unknown /api path must return JSON, not the SPA shell"
        );
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        let error: Value = serde_json::from_slice(&body).expect("error JSON");
        assert_eq!(error["error"], "not found");
    }

    #[tokio::test]
    async fn non_api_path_is_handled_by_spa_fallback() {
        let response = minimal_app()
            .await
            .oneshot(
                Request::builder()
                    .uri("/some/deep/link")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        // The SPA fallback serves index.html (when a frontend is embedded) or a
        // plain-text 404 (when not) — never the API's JSON 404.
        assert!(
            !content_type_of(&response).starts_with("application/json"),
            "non-API paths must not be answered by the API JSON 404 handler"
        );
    }

    #[tokio::test]
    async fn instances_endpoint_returns_deep_link_metadata_without_keys() {
        let pool = database::test_pool().await;
        let collection_port: Arc<dyn CollectionRepository> =
            Arc::new(SqliteCollectionRepository::new(pool.clone()));
        let catalog_port: Arc<dyn CatalogRepository> = Arc::new(SqliteCatalogRepository::new(pool));
        let instances = Arc::new(vec![
            Instance {
                id: "sonarr".to_owned(),
                name: "Sonarr".to_owned(),
                kind: InstanceKind::Sonarr,
                base_url: Url::parse("http://sonarr:8989/").expect("URL"),
                external_url: Some(Url::parse("https://sonarr.example.com/").expect("URL")),
                api_key: "super-secret-key".to_owned(),
                config_order: 0,
            },
            Instance {
                id: "radarr".to_owned(),
                name: "Radarr".to_owned(),
                kind: InstanceKind::Radarr,
                base_url: Url::parse("http://radarr:7878/").expect("URL"),
                external_url: None,
                api_key: "another-secret".to_owned(),
                config_order: 1,
            },
        ]);
        let app = router(AppState {
            catalog: CatalogService::new(catalog_port),
            sync: SyncService::new(
                collection_port,
                ArrClient::new().expect("Arr client"),
                Arc::clone(&instances),
            ),
            playback: None,
            instances,
        });

        let (status, body) = get_json(&app, "/api/v1/instances").await;
        assert_eq!(status, StatusCode::OK);
        let list = body.as_array().expect("instances array");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0]["id"], "sonarr");
        assert_eq!(list[0]["kind"], "sonarr");
        // A configured external_url wins over the internal API base_url.
        assert_eq!(list[0]["externalUrl"], "https://sonarr.example.com/");
        // Falls back to base_url when external_url is absent.
        assert_eq!(list[1]["externalUrl"], "http://radarr:7878/");
        // API keys must never leak into the browser-facing payload.
        let raw = serde_json::to_string(&body).expect("serialize");
        assert!(!raw.contains("super-secret-key"));
        assert!(!raw.contains("another-secret"));
        assert!(!raw.contains("apiKey"));
    }

    #[tokio::test]
    async fn series_details_returns_json_not_found_for_unknown_tvdb() {
        let response = minimal_app()
            .await
            .oneshot(
                Request::builder()
                    .uri("/api/v1/series/999")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(content_type_of(&response).starts_with("application/json"));
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        let error: Value = serde_json::from_slice(&body).expect("error JSON");
        assert_eq!(error["error"], "not found");
    }

    #[tokio::test]
    async fn series_details_aggregates_across_instances_and_reports_playback() {
        let (app, pool) = minimal_app_with_pool().await;
        insert_instances(&pool, "sonarr").await;
        // Same tvdb_id on two instances with different titles/sizes/file counts.
        sqlx::query(
            r#"
            INSERT INTO series_snapshots
                (instance_id, tvdb_id, title, title_slug, year, size_on_disk_bytes, file_count)
            VALUES ('a', 55, 'Show', 'show', 2020, 100, 3),
                   ('b', 55, 'Other', 'other', 2021, 200, 2)
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert series");
        // Season 2 appears on both instances; the details endpoint sums it.
        sqlx::query(
            r#"
            INSERT INTO series_season_snapshots (instance_id, tvdb_id, season_number, file_count)
            VALUES ('a', 55, 1, 2), ('a', 55, 2, 1), ('b', 55, 2, 1), ('b', 55, 3, 4)
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert seasons");
        // S01E01 exists on both instances (sizes summed, existence OR'd);
        // S01E02 is known but missing everywhere.
        sqlx::query(
            r#"
            INSERT INTO series_episode_snapshots (
                instance_id, tvdb_id, season_number, episode_number,
                title, air_date_utc, has_file, size_on_disk_bytes
            )
            VALUES
                ('a', 55, 1, 1, 'Pilot', '2020-01-01T00:00:00Z', 1, 100),
                ('a', 55, 1, 2, 'Second', '2020-01-08T00:00:00Z', 0, 0),
                ('b', 55, 1, 1, 'Pilot (4K)', '2020-01-01T00:00:00Z', 1, 400)
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert episodes");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/series/55")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        let details: Value = serde_json::from_slice(&body).expect("series JSON");
        assert_eq!(details["displayName"], "Show"); // lowest config_order wins
        assert_eq!(details["titleSlug"], "show"); // slug follows the same winner
        assert_eq!(details["sizeOnDiskBytes"], 300);
        assert_eq!(details["fileCount"], 5);
        assert_eq!(details["instances"].as_array().expect("instances").len(), 2);
        assert_eq!(
            details["seasons"][0],
            serde_json::json!({
                "seasonNumber": 1,
                "fileCount": 2,
                "episodeCount": 2,
                "episodesWithFiles": 1,
                "sizeOnDiskBytes": 500,
                "playback": null,
                "episodes": [
                    {
                        "episodeNumber": 1,
                        "title": "Pilot",
                        "airDateUtc": "2020-01-01T00:00:00Z",
                        "hasFile": true,
                        "sizeOnDiskBytes": 500,
                        "playback": null,
                    },
                    {
                        "episodeNumber": 2,
                        "title": "Second",
                        "airDateUtc": "2020-01-08T00:00:00Z",
                        "hasFile": false,
                        "sizeOnDiskBytes": 0,
                        "playback": null,
                    },
                ],
            })
        );
        // Seasons without episode rows still appear, with empty episode lists.
        assert_eq!(details["seasons"][1]["seasonNumber"], 2);
        assert_eq!(details["seasons"][1]["fileCount"], 2);
        assert_eq!(details["seasons"][1]["episodes"], serde_json::json!([]));
        assert_eq!(details["seasons"][2]["seasonNumber"], 3);
        assert!(details["playback"].is_null());
        assert!(details["unattributedPlayCount"].is_null());

        insert_playback_snapshot(&pool, "series", "55", 9, 1200).await;
        // One event carries its episode position, one predates position capture.
        sqlx::query(
            r#"
            INSERT INTO playback_events (
                source_id, source_row_id, content_type, content_id,
                played_at, duration_seconds, season_number, episode_number
            )
            VALUES
                ('plex', 1, 'series', '55', ?1, 600, 1, 1),
                ('plex', 2, 'series', '55', ?1, 300, NULL, NULL)
            "#,
        )
        .bind(chrono::Utc::now())
        .execute(&pool)
        .await
        .expect("insert playback events");

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/series/55")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        let details: Value = serde_json::from_slice(&body).expect("series JSON");
        assert_eq!(details["playback"]["playCount"], 9);
        assert_eq!(details["playback"]["playDurationSeconds"], 1200);
        let season = &details["seasons"][0];
        assert_eq!(season["playback"]["playCount"], 1);
        assert_eq!(season["playback"]["playDurationSeconds"], 600);
        assert_eq!(season["episodes"][0]["playback"]["playCount"], 1);
        // Known but never-played episodes report zeroed metrics, not null.
        assert_eq!(season["episodes"][1]["playback"]["playCount"], 0);
        // 9 series plays, 1 attributed to an episode cell.
        assert_eq!(details["unattributedPlayCount"], 8);
    }

    fn utc(iso: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(iso)
            .expect("valid RFC3339")
            .with_timezone(&chrono::Utc)
    }

    async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        (status, serde_json::from_slice(&body).expect("JSON body"))
    }

    async fn insert_instances(pool: &sqlx::SqlitePool, kind: &str) {
        for (id, order) in [("a", 0), ("b", 1)] {
            sqlx::query(
                r#"
                INSERT INTO instances (id, name, kind, config_order, last_successful_sync_at)
                VALUES (?1, ?1, ?2, ?3, ?4)
                "#,
            )
            .bind(id)
            .bind(kind)
            .bind(order)
            .bind(chrono::Utc::now())
            .execute(pool)
            .await
            .expect("insert instance");
        }
    }

    async fn insert_playback_snapshot(
        pool: &sqlx::SqlitePool,
        content_type: &str,
        content_id: &str,
        play_count: i64,
        play_duration_seconds: i64,
    ) {
        sqlx::query(
            r#"
            INSERT INTO playback_sources (id, provider, last_successful_sync_at)
            VALUES ('plex', 'tautulli', ?)
            "#,
        )
        .bind(chrono::Utc::now())
        .execute(pool)
        .await
        .expect("insert playback source");
        sqlx::query(
            r#"
            INSERT INTO playback_snapshots (
                source_id, content_type, content_id, play_count,
                play_duration_seconds, last_played_at
            )
            VALUES ('plex', ?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(content_type)
        .bind(content_id)
        .bind(play_count)
        .bind(play_duration_seconds)
        .bind(chrono::Utc::now())
        .execute(pool)
        .await
        .expect("insert playback snapshot");
    }

    #[tokio::test]
    async fn movie_details_returns_json_not_found_for_unknown_tmdb() {
        let app = minimal_app().await;
        let (status, error) = get_json(&app, "/api/v1/movies/999").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(error["error"], "not found");
    }

    #[tokio::test]
    async fn movie_details_aggregates_across_instances_and_reports_playback() {
        let (app, pool) = minimal_app_with_pool().await;
        insert_instances(&pool, "radarr").await;
        // Same tmdb_id on two instances with different titles/sizes/file counts.
        sqlx::query(
            r#"
            INSERT INTO movie_snapshots
                (instance_id, tmdb_id, title, year, size_on_disk_bytes, file_count)
            VALUES ('a', 42, 'Movie', 2020, 100, 1), ('b', 42, 'Other', 2021, 400, 1)
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert movies");

        let (status, details) = get_json(&app, "/api/v1/movies/42").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(details["displayName"], "Movie"); // lowest config_order wins
        assert_eq!(details["tmdbId"], 42);
        assert_eq!(details["year"], 2020);
        assert_eq!(details["sizeOnDiskBytes"], 500);
        assert_eq!(details["fileCount"], 2);
        assert_eq!(details["instances"].as_array().expect("instances").len(), 2);
        let instance_details = details["instanceDetails"]
            .as_array()
            .expect("instance details");
        assert_eq!(instance_details.len(), 2);
        assert_eq!(instance_details[0]["instance"]["id"], "a");
        assert_eq!(instance_details[0]["sizeOnDiskBytes"], 100);
        assert_eq!(instance_details[1]["sizeOnDiskBytes"], 400);
        assert!(details["playback"].is_null());

        insert_playback_snapshot(&pool, "movie", "42", 4, 7200).await;
        let (_, details) = get_json(&app, "/api/v1/movies/42").await;
        assert_eq!(details["playback"]["playCount"], 4);
        assert_eq!(details["playback"]["playDurationSeconds"], 7200);
    }

    #[tokio::test]
    async fn movie_details_reports_availability_and_daily_playback() {
        let (app, pool) = minimal_app_with_pool().await;
        insert_instances(&pool, "radarr").await;
        // Same movie on two instances added on different dates; the earliest
        // (instance 'b', despite its higher config_order) is the plot's edge.
        sqlx::query(
            r#"
            INSERT INTO movie_snapshots
                (instance_id, tmdb_id, title, year, size_on_disk_bytes, file_count, added_at)
            VALUES ('a', 42, 'Movie', 2020, 100, 1, ?1), ('b', 42, 'Movie', 2020, 400, 1, ?2)
            "#,
        )
        .bind(utc("2023-06-01T00:00:00Z"))
        .bind(utc("2023-05-01T00:00:00Z"))
        .execute(&pool)
        .await
        .expect("insert movies");

        // A playback source makes playback available; two sessions on the same
        // day (summed) plus one on a later day prove the per-day buckets.
        insert_playback_snapshot(&pool, "movie", "42", 3, 2100).await;
        for (row_id, played_at, duration) in [
            (1_i64, "2024-01-10T08:00:00Z", 600_i64),
            (2, "2024-01-10T20:00:00Z", 300),
            (3, "2024-03-05T00:00:00Z", 1200),
        ] {
            sqlx::query(
                r#"
                INSERT INTO playback_events (
                    source_id, source_row_id, content_type, content_id,
                    played_at, duration_seconds
                )
                VALUES ('plex', ?1, 'movie', '42', ?2, ?3)
                "#,
            )
            .bind(row_id)
            .bind(utc(played_at))
            .bind(duration)
            .execute(&pool)
            .await
            .expect("insert playback event");
        }

        let (status, details) = get_json(&app, "/api/v1/movies/42").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(details["availableAt"], "2023-05-01T00:00:00Z");
        let daily = details["dailyPlayback"].as_array().expect("daily");
        assert_eq!(daily.len(), 2);
        assert_eq!(
            daily[0],
            serde_json::json!({
                "date": "2024-01-10",
                "playCount": 2,
                "playDurationSeconds": 900,
            })
        );
        assert_eq!(
            daily[1],
            serde_json::json!({
                "date": "2024-03-05",
                "playCount": 1,
                "playDurationSeconds": 1200,
            })
        );
    }

    #[tokio::test]
    async fn artist_details_returns_json_not_found_for_unknown_musicbrainz_id() {
        let app = minimal_app().await;
        let (status, error) =
            get_json(&app, "/api/v1/artists/00000000-0000-0000-0000-000000000000").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(error["error"], "not found");
    }

    #[tokio::test]
    async fn artist_details_aggregates_albums_and_reports_playback() {
        let (app, pool) = minimal_app_with_pool().await;
        insert_instances(&pool, "lidarr").await;
        sqlx::query(
            r#"
            INSERT INTO artist_snapshots
                (instance_id, musicbrainz_id, name, size_on_disk_bytes, file_count)
            VALUES ('a', 'artist-1', 'Artist', 500, 6), ('b', 'artist-1', 'Other', 400, 3)
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert artists");
        // A shared album on both instances (sizes/files summed, first title
        // wins) plus one pre-0007 row without the new columns, proving the
        // migration defaults ('' / 0) serialize end to end.
        sqlx::query(
            r#"
            INSERT INTO artist_album_snapshots
                (instance_id, artist_musicbrainz_id, album_musicbrainz_id,
                 title, size_on_disk_bytes, file_count)
            VALUES
                ('a', 'artist-1', 'album-1', 'Alpha', 300, 3),
                ('b', 'artist-1', 'album-1', 'Alpha (b)', 400, 3)
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert albums");
        sqlx::query(
            r#"
            INSERT INTO artist_album_snapshots
                (instance_id, artist_musicbrainz_id, album_musicbrainz_id, file_count)
            VALUES ('a', 'artist-1', 'album-legacy', 3)
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert legacy album");

        let (status, details) = get_json(&app, "/api/v1/artists/artist-1").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(details["displayName"], "Artist"); // lowest config_order wins
        assert_eq!(details["musicBrainzId"], "artist-1");
        assert_eq!(details["sizeOnDiskBytes"], 900);
        assert_eq!(details["fileCount"], 9);
        assert_eq!(details["instances"].as_array().expect("instances").len(), 2);
        // Empty titles sort first; the legacy row keeps the migration defaults.
        assert_eq!(
            details["albums"],
            serde_json::json!([
                {
                    "musicBrainzId": "album-legacy",
                    "title": "",
                    "sizeOnDiskBytes": 0,
                    "fileCount": 3,
                },
                {
                    "musicBrainzId": "album-1",
                    "title": "Alpha",
                    "sizeOnDiskBytes": 700,
                    "fileCount": 6,
                },
            ])
        );
        let instance_details = details["instanceDetails"]
            .as_array()
            .expect("instance details");
        assert_eq!(instance_details[0]["instance"]["id"], "a");
        assert_eq!(instance_details[0]["albumCount"], 2);
        assert_eq!(instance_details[1]["albumCount"], 1);
        assert!(details["playback"].is_null());

        insert_playback_snapshot(&pool, "artist", "artist-1", 12, 3600).await;
        // Uppercased MBIDs in the URL are normalized to the stored lowercase.
        let (status, details) = get_json(&app, "/api/v1/artists/ARTIST-1").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(details["playback"]["playCount"], 12);
        assert_eq!(details["playback"]["playDurationSeconds"], 3600);
    }
}
