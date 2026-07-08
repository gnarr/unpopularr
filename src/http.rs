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
    playback::{PlaybackService, PlaybackSyncTrigger, StartPlaybackSync},
    web_assets::spa_fallback,
};

#[derive(Clone)]
pub struct AppState {
    pub catalog: CatalogService,
    pub sync: SyncService,
    pub playback: Option<PlaybackService>,
}

pub fn router(state: AppState) -> Router {
    let playback_enabled = state.playback.is_some();
    let api = Router::new()
        .route("/v1/content", get(all_content))
        .route("/v1/series/{tvdb_id}", get(series_details))
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

async fn series_details(State(state): State<Arc<AppState>>, Path(tvdb_id): Path<i64>) -> Response {
    match state.catalog.series_details(tvdb_id).await {
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
        for (id, order) in [("a", 0), ("b", 1)] {
            sqlx::query(
                r#"
                INSERT INTO instances (id, name, kind, config_order, last_successful_sync_at)
                VALUES (?1, ?1, 'sonarr', ?2, ?3)
                "#,
            )
            .bind(id)
            .bind(order)
            .bind(chrono::Utc::now())
            .execute(&pool)
            .await
            .expect("insert instance");
        }
        // Same tvdb_id on two instances with different titles/sizes/file counts.
        sqlx::query(
            r#"
            INSERT INTO series_snapshots
                (instance_id, tvdb_id, title, year, size_on_disk_bytes, file_count)
            VALUES ('a', 55, 'Show', 2020, 100, 3), ('b', 55, 'Other', 2021, 200, 2)
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
            VALUES ('plex', 'series', '55', 9, 1200, ?)
            "#,
        )
        .bind(chrono::Utc::now())
        .execute(&pool)
        .await
        .expect("insert playback snapshot");
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
}
