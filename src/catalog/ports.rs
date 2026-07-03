use anyhow::Result;
use async_trait::async_trait;

use super::{CatalogSources, SeriesDetailsSources};

#[async_trait]
pub trait CatalogRepository: Send + Sync {
    async fn load_sources(&self) -> Result<CatalogSources>;

    /// Load the raw per-instance material for a single series, or `None` when no
    /// series with `tvdb_id` has been synced.
    async fn load_series(&self, tvdb_id: i64) -> Result<Option<SeriesDetailsSources>>;
}
