use std::sync::Arc;

use anyhow::Result;

use super::{CatalogRepository, ContentItem, aggregate};

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
}
