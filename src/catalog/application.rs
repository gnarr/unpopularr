use std::sync::Arc;

use anyhow::Result;

use super::{CatalogRepository, ContentItem, SeriesDetails, aggregate, aggregate_series};

#[derive(Clone)]
pub struct CatalogService {
    repository: Arc<dyn CatalogRepository>,
}

impl CatalogService {
    pub fn new(repository: Arc<dyn CatalogRepository>) -> Self {
        Self { repository }
    }

    pub async fn all_content(&self) -> Result<Vec<ContentItem>> {
        Ok(aggregate(self.repository.load_sources().await?))
    }

    pub async fn series_details(&self, tvdb_id: i64) -> Result<Option<SeriesDetails>> {
        Ok(self
            .repository
            .load_series(tvdb_id)
            .await?
            .and_then(aggregate_series))
    }
}
