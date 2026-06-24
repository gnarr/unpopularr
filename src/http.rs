use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
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
};

#[derive(Clone)]
pub struct AppState {
    pub catalog: CatalogService,
    pub sync: SyncService,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/content", get(all_content))
        .route("/api/v1/sync", get(sync_status).post(start_sync))
        .layer(TraceLayer::new_for_http())
        .with_state(Arc::new(state))
}

async fn all_content(State(state): State<Arc<AppState>>) -> Response {
    match state.catalog.all_content().await {
        Ok(content) => Json(content).into_response(),
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
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers::path};

    use crate::{
        catalog::{CatalogRepository, CatalogService, adapters::sqlite::SqliteCatalogRepository},
        collection::{
            CollectionRepository, SyncService, adapters::arr::ArrClient,
            adapters::sqlite::SqliteCollectionRepository,
        },
        database,
        instances::{Instance, InstanceKind},
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
        let catalog_port: Arc<dyn CatalogRepository> = Arc::new(SqliteCatalogRepository::new(pool));
        let application = router(AppState {
            catalog: CatalogService::new(catalog_port),
            sync: SyncService::new(
                collection_port,
                ArrClient::new().expect("Arr client"),
                Arc::new(vec![instance]),
            ),
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
    }
}
