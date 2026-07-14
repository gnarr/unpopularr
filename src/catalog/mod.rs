pub mod adapters;
mod application;
mod domain;
mod ports;

pub use application::CatalogService;
pub use domain::{
    ArtistAlbumDetail, ArtistAlbumFile, ArtistDetails, ArtistDetailsSources, ArtistInstanceDetail,
    ArtistSource, CatalogPlayback, CatalogSources, ContentItem, DailyPlayback, InstanceReference,
    MovieDetails, MovieDetailsSources, MovieInstanceDetail, MovieSource, PlaybackMetrics,
    SeriesDetails, SeriesDetailsSources, SeriesEpisodeDetail, SeriesEpisodeFile,
    SeriesEpisodePlayback, SeriesInstanceDetail, SeriesSeasonDetail, SeriesSeasonFiles,
    SeriesSource, UserPlayback, aggregate, aggregate_artist, aggregate_movie, aggregate_series,
};
pub use ports::CatalogRepository;
