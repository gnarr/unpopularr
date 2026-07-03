pub mod adapters;
mod application;
mod domain;
mod ports;

pub use application::CatalogService;
pub use domain::{
    ArtistSource, CatalogPlayback, CatalogSources, ContentItem, InstanceReference, MovieSource,
    PlaybackMetrics, SeriesDetails, SeriesDetailsSources, SeriesInstanceDetail, SeriesSeasonDetail,
    SeriesSeasonFiles, SeriesSource, aggregate, aggregate_series,
};
pub use ports::CatalogRepository;
