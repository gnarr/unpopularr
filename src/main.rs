use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result};
use tokio::{net::TcpListener, signal, time};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use unpopularr::{
    catalog::{CatalogRepository, CatalogService, adapters::sqlite::SqliteCatalogRepository},
    collection::{
        CollectionRepository, StartSync, SyncService, SyncTrigger, adapters::arr::ArrClient,
        adapters::sqlite::SqliteCollectionRepository,
    },
    config::AppConfig,
    database,
    http::{AppState, router},
};

#[tokio::main]
async fn main() -> Result<()> {
    init_logging();
    let config = AppConfig::load()?;
    let pool = database::connect(&config.database.path).await?;

    let collection_repository = Arc::new(SqliteCollectionRepository::new(pool.clone()));
    collection_repository
        .reconcile_instances(&config.instances)
        .await
        .context("failed to reconcile configured instances")?;
    collection_repository
        .recover_interrupted_syncs(chrono::Utc::now())
        .await
        .context("failed to recover interrupted syncs")?;

    let catalog_repository: Arc<dyn CatalogRepository> =
        Arc::new(SqliteCatalogRepository::new(pool));
    let collection_repository: Arc<dyn CollectionRepository> = collection_repository;
    let instances = Arc::new(config.instances);
    let sync_service = SyncService::new(
        collection_repository,
        ArrClient::new()?,
        Arc::clone(&instances),
    );
    let catalog_service = CatalogService::new(catalog_repository);

    if config.sync.run_on_startup {
        start_background_sync(&sync_service, SyncTrigger::Startup).await;
    }
    spawn_scheduler(sync_service.clone(), config.sync.interval);

    let application = router(AppState {
        catalog: catalog_service,
        sync: sync_service,
    });
    let listener = TcpListener::bind(config.server.bind)
        .await
        .with_context(|| format!("failed to bind server to {}", config.server.bind))?;
    info!(address = %config.server.bind, "HTTP server listening");

    axum::serve(listener, application)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("HTTP server failed")
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn spawn_scheduler(sync_service: SyncService, interval: Duration) {
    tokio::spawn(async move {
        let mut ticker = time::interval(interval);
        ticker.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
        ticker.tick().await;

        loop {
            ticker.tick().await;
            start_background_sync(&sync_service, SyncTrigger::Scheduled).await;
        }
    });
}

async fn start_background_sync(sync_service: &SyncService, trigger: SyncTrigger) {
    match sync_service.start(trigger).await {
        Ok(StartSync::Started(run)) => {
            info!(
                sync_run_id = run.id,
                trigger = trigger.as_str(),
                "sync queued"
            );
        }
        Ok(StartSync::AlreadyRunning(_)) => {
            warn!(
                trigger = trigger.as_str(),
                "sync skipped because another sync is running"
            );
        }
        Err(error) => {
            warn!(trigger = trigger.as_str(), error = %error, "failed to start sync");
        }
    }
}

async fn shutdown_signal() {
    #[cfg(unix)]
    let result = tokio::select! {
        result = signal::ctrl_c() => result,
        result = terminate_signal() => result,
    };
    #[cfg(not(unix))]
    let result = signal::ctrl_c().await;

    if let Err(error) = result {
        warn!(error = %error, "failed to install shutdown signal handler");
    }
    info!("shutdown signal received");
}

#[cfg(unix)]
async fn terminate_signal() -> std::io::Result<()> {
    let mut signal = signal::unix::signal(signal::unix::SignalKind::terminate())?;
    signal.recv().await;
    Ok(())
}
