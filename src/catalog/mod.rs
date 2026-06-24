pub mod adapters;
mod application;
mod domain;
mod ports;

pub use application::CatalogService;
pub use domain::{
    ArtistSource, CatalogPlayback, CatalogSources, ContentItem, InstanceReference, MovieSource,
    PlaybackMetrics, SeriesSource, aggregate,
};
pub use ports::CatalogRepository;
