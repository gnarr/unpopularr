use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Utc;
use tokio::sync::{Mutex, OwnedMutexGuard};
use tracing::{error, info, warn};

use super::{
    PlaybackRepository, PlaybackSource, PlaybackSourceClient, PlaybackSyncRun, PlaybackSyncTrigger,
};

#[derive(Clone)]
pub struct PlaybackService {
    repository: Arc<dyn PlaybackRepository>,
    client: Arc<dyn PlaybackSourceClient>,
    source: Arc<PlaybackSource>,
    sync_lock: Arc<Mutex<()>>,
}

pub enum StartPlaybackSync {
    Started(PlaybackSyncRun),
    AlreadyRunning(Option<PlaybackSyncRun>),
}

impl PlaybackService {
    pub fn new(
        repository: Arc<dyn PlaybackRepository>,
        client: Arc<dyn PlaybackSourceClient>,
        source: Arc<PlaybackSource>,
    ) -> Self {
        Self {
            repository,
            client,
            source,
            sync_lock: Arc::new(Mutex::new(())),
        }
    }

    pub async fn start(&self, trigger: PlaybackSyncTrigger) -> Result<StartPlaybackSync> {
        let Ok(guard) = Arc::clone(&self.sync_lock).try_lock_owned() else {
            return Ok(StartPlaybackSync::AlreadyRunning(
                self.repository.active_or_latest_sync().await?,
            ));
        };

        let run = self
            .repository
            .create_sync_run(&self.source, trigger, Utc::now())
            .await?;
        let service = self.clone();
        let run_id = run.id;
        tokio::spawn(async move {
            if let Err(error) = service.execute(run_id, guard).await {
                error!(playback_sync_run_id = run_id, error = %error, "playback sync execution failed");
            }
        });

        Ok(StartPlaybackSync::Started(run))
    }

    pub async fn active_or_latest(&self) -> Result<Option<PlaybackSyncRun>> {
        self.repository.active_or_latest_sync().await
    }

    async fn execute(&self, run_id: i64, _guard: OwnedMutexGuard<()>) -> Result<()> {
        info!(playback_sync_run_id = run_id, "playback sync started");
        let snapshot = match self.client.collect(&self.source).await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let message = sanitize_error(&error);
                warn!(
                    playback_sync_run_id = run_id,
                    error = %message,
                    "playback source collection failed"
                );
                self.repository
                    .mark_failed(run_id, 0, 0, &message, Utc::now())
                    .await
                    .context("failed to record playback sync failure")?;
                return Ok(());
            }
        };

        if snapshot.matched_history_rows == 0 && snapshot.unmatched_history_rows > 0 {
            self.repository
                .mark_failed(
                    run_id,
                    snapshot.matched_history_rows,
                    snapshot.unmatched_history_rows,
                    "playback history contained no supported external identifiers",
                    Utc::now(),
                )
                .await?;
            return Ok(());
        }

        let status = snapshot.status();
        let run = match self
            .repository
            .store_snapshot(run_id, &self.source, &snapshot, status, Utc::now())
            .await
        {
            Ok(run) => run,
            Err(error) => {
                let message = sanitize_error(&error);
                error!(
                    playback_sync_run_id = run_id,
                    error = %message,
                    "failed to persist playback snapshot"
                );
                self.repository
                    .mark_failed(
                        run_id,
                        snapshot.matched_history_rows,
                        snapshot.unmatched_history_rows,
                        "playback snapshot persistence failed",
                        Utc::now(),
                    )
                    .await
                    .context("failed to record playback persistence failure")?
            }
        };

        info!(
            playback_sync_run_id = run.id,
            status = run.status.as_str(),
            matched_history_rows = run.matched_history_rows,
            unmatched_history_rows = run.unmatched_history_rows,
            "playback sync finished"
        );
        Ok(())
    }
}

fn sanitize_error(error: &anyhow::Error) -> String {
    let message = format!("{error:#}")
        .chars()
        .filter(|character| !character.is_control())
        .take(500)
        .collect::<String>();
    if message.is_empty() {
        "playback sync failed".to_owned()
    } else {
        message
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use anyhow::Result;
    use async_trait::async_trait;
    use url::Url;

    use crate::{
        database,
        playback::{
            ContentKey, PlaybackAggregate, PlaybackProvider, PlaybackRepository, PlaybackSnapshot,
            PlaybackSource, PlaybackSourceClient, PlaybackSyncStatus, PlaybackSyncTrigger,
            StartPlaybackSync, adapters::sqlite::SqlitePlaybackRepository,
        },
    };

    use super::PlaybackService;

    struct StaticClient {
        snapshot: PlaybackSnapshot,
    }

    #[async_trait]
    impl PlaybackSourceClient for StaticClient {
        async fn collect(&self, _source: &PlaybackSource) -> Result<PlaybackSnapshot> {
            Ok(self.snapshot.clone())
        }
    }

    fn source() -> Arc<PlaybackSource> {
        Arc::new(PlaybackSource {
            id: "plex".to_owned(),
            provider: PlaybackProvider::Tautulli,
            base_url: Url::parse("http://localhost/").expect("URL"),
            api_key: "secret".to_owned(),
        })
    }

    async fn completed_run(
        repository: &Arc<SqlitePlaybackRepository>,
    ) -> crate::playback::PlaybackSyncRun {
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let run = repository
                    .active_or_latest_sync()
                    .await
                    .expect("load run")
                    .expect("run");
                if run.status != PlaybackSyncStatus::Running {
                    break run;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("sync completed")
    }

    #[tokio::test]
    async fn records_partial_runs_when_some_history_is_unmatched() {
        let repository = Arc::new(SqlitePlaybackRepository::new(database::test_pool().await));
        let source = source();
        repository
            .reconcile_source(Some(&source))
            .await
            .expect("reconcile");
        let repository_port: Arc<dyn PlaybackRepository> = repository.clone();
        let service = PlaybackService::new(
            repository_port,
            Arc::new(StaticClient {
                snapshot: PlaybackSnapshot {
                    aggregates: vec![PlaybackAggregate {
                        key: ContentKey::Movie(1),
                        play_count: 1,
                        play_duration_seconds: 60,
                        last_played_at: None,
                    }],
                    matched_history_rows: 1,
                    unmatched_history_rows: 1,
                },
            }),
            source,
        );

        assert!(matches!(
            service
                .start(PlaybackSyncTrigger::Manual)
                .await
                .expect("start"),
            StartPlaybackSync::Started(_)
        ));
        let run = completed_run(&repository).await;
        assert_eq!(run.status, PlaybackSyncStatus::Partial);
        assert_eq!(run.matched_history_rows, 1);
        assert_eq!(run.unmatched_history_rows, 1);
    }

    #[tokio::test]
    async fn fails_without_replacing_when_no_history_can_be_matched() {
        let pool = database::test_pool().await;
        let repository = Arc::new(SqlitePlaybackRepository::new(pool.clone()));
        let source = source();
        repository
            .reconcile_source(Some(&source))
            .await
            .expect("reconcile");
        let repository_port: Arc<dyn PlaybackRepository> = repository.clone();
        let service = PlaybackService::new(
            repository_port,
            Arc::new(StaticClient {
                snapshot: PlaybackSnapshot {
                    aggregates: Vec::new(),
                    matched_history_rows: 0,
                    unmatched_history_rows: 2,
                },
            }),
            source,
        );

        service
            .start(PlaybackSyncTrigger::Manual)
            .await
            .expect("start");
        let run = completed_run(&repository).await;
        assert_eq!(run.status, PlaybackSyncStatus::Failed);
        assert_eq!(run.unmatched_history_rows, 2);
        let snapshot_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playback_snapshots")
            .fetch_one(&pool)
            .await
            .expect("count snapshots");
        assert_eq!(snapshot_count, 0);
    }
}
