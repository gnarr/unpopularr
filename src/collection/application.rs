use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Utc;
use futures::{StreamExt, stream};
use tokio::sync::{Mutex, OwnedMutexGuard};
use tracing::{error, info, warn};

use crate::{
    collection::{CollectionRepository, SyncRun, SyncStatus, SyncTrigger},
    instances::Instance,
};

use super::adapters::arr::ArrClient;

const MAX_CONCURRENT_INSTANCE_SYNCS: usize = 4;

#[derive(Clone)]
pub struct SyncService {
    repository: Arc<dyn CollectionRepository>,
    client: ArrClient,
    instances: Arc<Vec<Instance>>,
    sync_lock: Arc<Mutex<()>>,
}

pub enum StartSync {
    Started(SyncRun),
    AlreadyRunning(Option<SyncRun>),
}

impl SyncService {
    pub fn new(
        repository: Arc<dyn CollectionRepository>,
        client: ArrClient,
        instances: Arc<Vec<Instance>>,
    ) -> Self {
        Self {
            repository,
            client,
            instances,
            sync_lock: Arc::new(Mutex::new(())),
        }
    }

    pub async fn start(&self, trigger: SyncTrigger) -> Result<StartSync> {
        let Ok(guard) = Arc::clone(&self.sync_lock).try_lock_owned() else {
            return Ok(StartSync::AlreadyRunning(
                self.repository.active_or_latest_sync().await?,
            ));
        };

        let run = self
            .repository
            .create_sync_run(trigger, &self.instances, Utc::now())
            .await?;
        let service = self.clone();
        let run_id = run.id;
        tokio::spawn(async move {
            if let Err(error) = service.execute(run_id, guard).await {
                error!(sync_run_id = run_id, error = %error, "sync execution failed");
            }
        });

        Ok(StartSync::Started(run))
    }

    pub async fn active_or_latest(&self) -> Result<Option<SyncRun>> {
        self.repository.active_or_latest_sync().await
    }

    async fn execute(&self, run_id: i64, _guard: OwnedMutexGuard<()>) -> Result<()> {
        info!(sync_run_id = run_id, "sync started");

        stream::iter(self.instances.iter().cloned())
            .map(|instance| {
                let service = self.clone();
                async move {
                    let completed_at;
                    match service.client.collect(&instance).await {
                        Ok(snapshot) => {
                            completed_at = Utc::now();
                            let item_count = snapshot.item_count();
                            if let Err(error) = service
                                .repository
                                .store_successful_snapshot(
                                    run_id,
                                    &instance,
                                    &snapshot,
                                    completed_at,
                                )
                                .await
                            {
                                let error_message = sanitize_error(&error);
                                error!(
                                    sync_run_id = run_id,
                                    instance_id = instance.id,
                                    error = %error_message,
                                    "failed to persist instance snapshot"
                                );
                                service
                                    .repository
                                    .mark_instance_failed(
                                        run_id,
                                        &instance.id,
                                        &error_message,
                                        Utc::now(),
                                    )
                                    .await
                                    .context("failed to record snapshot persistence failure")?;
                            } else {
                                info!(
                                    sync_run_id = run_id,
                                    instance_id = instance.id,
                                    imported_items = item_count,
                                    "instance sync succeeded"
                                );
                            }
                        }
                        Err(error) => {
                            let error_message = sanitize_error(&error);
                            warn!(
                                sync_run_id = run_id,
                                instance_id = instance.id,
                                error = %error_message,
                                "instance sync failed"
                            );
                            service
                                .repository
                                .mark_instance_failed(
                                    run_id,
                                    &instance.id,
                                    &error_message,
                                    Utc::now(),
                                )
                                .await
                                .context("failed to record instance failure")?;
                        }
                    }
                    Ok::<(), anyhow::Error>(())
                }
            })
            .buffer_unordered(MAX_CONCURRENT_INSTANCE_SYNCS)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>>>()?;

        let run = self.repository.finish_sync_run(run_id, Utc::now()).await?;
        info!(
            sync_run_id = run_id,
            status = run.status.as_str(),
            imported_items = run.imported_items,
            "sync finished"
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
        "instance sync failed".to_owned()
    } else {
        message
    }
}

pub fn final_status(successful: usize, failed: usize) -> SyncStatus {
    match (successful, failed) {
        (_, 0) => SyncStatus::Succeeded,
        (0, _) => SyncStatus::Failed,
        _ => SyncStatus::Partial,
    }
}
