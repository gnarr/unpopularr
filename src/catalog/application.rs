use std::sync::Arc;

use anyhow::Result;

use super::{
    ArtistDetails, CatalogRepository, ContentItem, MovieDetails, SeriesDetails, aggregate,
    aggregate_artist, aggregate_movie, aggregate_series,
};

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

    pub async fn movie_details(&self, tmdb_id: i64) -> Result<Option<MovieDetails>> {
        Ok(self
            .repository
            .load_movie(tmdb_id)
            .await?
            .and_then(aggregate_movie))
    }

    pub async fn artist_details(&self, musicbrainz_id: &str) -> Result<Option<ArtistDetails>> {
        Ok(self
            .repository
            .load_artist(musicbrainz_id)
            .await?
            .and_then(aggregate_artist))
    }
}
