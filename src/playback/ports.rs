use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::{
    PlaybackSnapshot, PlaybackSource, PlaybackSyncRun, PlaybackSyncStatus, PlaybackSyncTrigger,
};

#[async_trait]
pub trait PlaybackSourceClient: Send + Sync {
    async fn collect(&self, source: &PlaybackSource) -> Result<PlaybackSnapshot>;
}

#[async_trait]
pub trait PlaybackRepository: Send + Sync {
    async fn reconcile_source(&self, source: Option<&PlaybackSource>) -> Result<()>;

    async fn recover_interrupted_syncs(&self, completed_at: DateTime<Utc>) -> Result<()>;

    async fn create_sync_run(
        &self,
        source: &PlaybackSource,
        trigger: PlaybackSyncTrigger,
        started_at: DateTime<Utc>,
    ) -> Result<PlaybackSyncRun>;

    async fn store_events(
        &self,
        sync_run_id: i64,
        source: &PlaybackSource,
        snapshot: &PlaybackSnapshot,
        status: PlaybackSyncStatus,
        completed_at: DateTime<Utc>,
    ) -> Result<PlaybackSyncRun>;

    async fn mark_failed(
        &self,
        sync_run_id: i64,
        matched_history_rows: i64,
        unmatched_history_rows: i64,
        error: &str,
        completed_at: DateTime<Utc>,
    ) -> Result<PlaybackSyncRun>;

    async fn active_or_latest_sync(&self) -> Result<Option<PlaybackSyncRun>>;
}
