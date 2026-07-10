use anyhow::Result;
use async_trait::async_trait;

use super::{ArtistDetailsSources, CatalogSources, MovieDetailsSources, SeriesDetailsSources};

#[async_trait]
pub trait CatalogRepository: Send + Sync {
    async fn load_sources(&self) -> Result<CatalogSources>;

    /// Load the raw per-instance material for a single series, or `None` when no
    /// series with `tvdb_id` has been synced.
    async fn load_series(&self, tvdb_id: i64) -> Result<Option<SeriesDetailsSources>>;

    /// Load the raw per-instance material for a single movie, or `None` when no
    /// movie with `tmdb_id` has been synced.
    async fn load_movie(&self, tmdb_id: i64) -> Result<Option<MovieDetailsSources>>;

    /// Load the raw per-instance material for a single artist, or `None` when
    /// no artist with `musicbrainz_id` has been synced. IDs are stored
    /// lowercase.
    async fn load_artist(&self, musicbrainz_id: &str) -> Result<Option<ArtistDetailsSources>>;
}
