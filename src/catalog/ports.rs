use anyhow::Result;
use async_trait::async_trait;

use super::CatalogSources;

#[async_trait]
pub trait CatalogRepository: Send + Sync {
    async fn load_sources(&self) -> Result<CatalogSources>;
}
