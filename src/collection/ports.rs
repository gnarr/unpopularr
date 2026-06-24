use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::instances::Instance;

use super::{Snapshot, SyncRun, SyncTrigger};

#[async_trait]
pub trait CollectionRepository: Send + Sync {
    async fn reconcile_instances(&self, instances: &[Instance]) -> Result<()>;

    async fn recover_interrupted_syncs(&self, completed_at: DateTime<Utc>) -> Result<()>;

    async fn create_sync_run(
        &self,
        trigger: SyncTrigger,
        instances: &[Instance],
        started_at: DateTime<Utc>,
    ) -> Result<SyncRun>;

    async fn store_successful_snapshot(
        &self,
        sync_run_id: i64,
        instance: &Instance,
        snapshot: &Snapshot,
        completed_at: DateTime<Utc>,
    ) -> Result<()>;

    async fn mark_instance_failed(
        &self,
        sync_run_id: i64,
        instance_id: &str,
        error: &str,
        completed_at: DateTime<Utc>,
    ) -> Result<()>;

    async fn finish_sync_run(
        &self,
        sync_run_id: i64,
        completed_at: DateTime<Utc>,
    ) -> Result<SyncRun>;

    async fn active_or_latest_sync(&self) -> Result<Option<SyncRun>>;
}
