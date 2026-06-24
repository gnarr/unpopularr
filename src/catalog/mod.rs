pub mod adapters;
mod application;
mod domain;
mod ports;

pub use application::CatalogService;
pub use domain::{
    ArtistSource, CatalogSources, ContentItem, InstanceReference, MovieSource, SeriesSource,
    aggregate,
};
pub use ports::CatalogRepository;
