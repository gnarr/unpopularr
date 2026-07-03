use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackProvider {
    Tautulli,
}

impl PlaybackProvider {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tautulli => "tautulli",
        }
    }
}

#[derive(Clone)]
pub struct PlaybackSource {
    pub id: String,
    pub provider: PlaybackProvider,
    pub base_url: Url,
    pub api_key: String,
}

impl fmt::Debug for PlaybackSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PlaybackSource")
            .field("id", &self.id)
            .field("provider", &self.provider)
            .field("base_url", &self.base_url)
            .field("api_key", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ContentKey {
    Movie(i64),
    Series(i64),
    Artist(String),
}

impl ContentKey {
    pub const fn content_type(&self) -> &'static str {
        match self {
            Self::Movie(_) => "movie",
            Self::Series(_) => "series",
            Self::Artist(_) => "artist",
        }
    }

    pub fn content_id(&self) -> String {
        match self {
            Self::Movie(id) | Self::Series(id) => id.to_string(),
            Self::Artist(id) => id.clone(),
        }
    }
}

/// A single playback session collected from the source (one Tautulli history row).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaybackEvent {
    pub key: ContentKey,
    /// Stable source-side identifier used to deduplicate events across syncs.
    pub source_row_id: i64,
    pub played_at: DateTime<Utc>,
    pub duration_seconds: i64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PlaybackSnapshot {
    pub events: Vec<PlaybackEvent>,
    pub matched_history_rows: i64,
    pub unmatched_history_rows: i64,
}

impl PlaybackSnapshot {
    pub fn status(&self) -> PlaybackSyncStatus {
        if self.unmatched_history_rows > 0 {
            PlaybackSyncStatus::Partial
        } else {
            PlaybackSyncStatus::Succeeded
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackSyncTrigger {
    Startup,
    Scheduled,
    Manual,
}

impl PlaybackSyncTrigger {
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
pub enum PlaybackSyncStatus {
    Running,
    Succeeded,
    Partial,
    Failed,
}

impl PlaybackSyncStatus {
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
pub struct PlaybackSyncRun {
    pub id: i64,
    pub source_id: String,
    pub trigger: PlaybackSyncTrigger,
    pub status: PlaybackSyncStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub matched_history_rows: i64,
    pub unmatched_history_rows: i64,
    pub error: Option<String>,
}
