use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::instances::InstanceKind;

#[derive(Clone, Debug)]
pub enum Snapshot {
    Movies(Vec<MovieSnapshot>),
    Series(Vec<SeriesSnapshot>),
    Artists(Vec<ArtistSnapshot>),
}

impl Snapshot {
    pub fn item_count(&self) -> usize {
        match self {
            Self::Movies(items) => items.len(),
            Self::Series(items) => items.len(),
            Self::Artists(items) => items.len(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct MovieSnapshot {
    pub tmdb_id: i64,
    pub title: String,
    pub year: i64,
    pub size_on_disk_bytes: i64,
    pub file_count: i64,
}

#[derive(Clone, Debug)]
pub struct SeriesSnapshot {
    pub tvdb_id: i64,
    pub title: String,
    pub year: i64,
    pub size_on_disk_bytes: i64,
    pub file_count: i64,
    pub seasons: Vec<SeriesSeasonSnapshot>,
}

#[derive(Clone, Debug)]
pub struct SeriesSeasonSnapshot {
    pub season_number: i64,
    pub file_count: i64,
}

#[derive(Clone, Debug)]
pub struct ArtistSnapshot {
    pub musicbrainz_id: String,
    pub name: String,
    pub size_on_disk_bytes: i64,
    pub file_count: i64,
    pub albums: Vec<ArtistAlbumSnapshot>,
}

#[derive(Clone, Debug)]
pub struct ArtistAlbumSnapshot {
    pub musicbrainz_id: String,
    pub file_count: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncTrigger {
    Startup,
    Scheduled,
    Manual,
}

impl SyncTrigger {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::Scheduled => "scheduled",
            Self::Manual => "manual",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncStatus {
    Running,
    Succeeded,
    Partial,
    Failed,
}

impl SyncStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Partial => "partial",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRun {
    pub id: i64,
    pub trigger: SyncTrigger,
    pub status: SyncStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub imported_items: i64,
    pub instances: Vec<InstanceSyncResult>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceSyncResult {
    pub id: String,
    pub name: String,
    pub kind: InstanceKind,
    pub status: SyncStatus,
    pub imported_items: i64,
    pub error: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
