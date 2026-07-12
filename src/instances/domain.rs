use std::fmt;

use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InstanceKind {
    Sonarr,
    Radarr,
    Lidarr,
}

impl InstanceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sonarr => "sonarr",
            Self::Radarr => "radarr",
            Self::Lidarr => "lidarr",
        }
    }
}

impl fmt::Display for InstanceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub kind: InstanceKind,
    pub base_url: Url,
    /// Browser-facing URL for deep links into this service's web UI. `None`
    /// when unset in config, in which case [`Self::base_url`] is used. Distinct
    /// from `base_url` because the API address is often internal (e.g. a Docker
    /// service name) and unreachable from a user's browser.
    pub external_url: Option<Url>,
    pub api_key: String,
    pub config_order: i64,
}

impl Instance {
    /// The URL a browser should use to reach this service, preferring the
    /// configured `external_url` and falling back to the API `base_url`.
    #[must_use]
    pub fn web_url(&self) -> &Url {
        self.external_url.as_ref().unwrap_or(&self.base_url)
    }
}

impl fmt::Debug for Instance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Instance")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("base_url", &self.base_url)
            .field("external_url", &self.external_url)
            .field("api_key", &"[REDACTED]")
            .field("config_order", &self.config_order)
            .finish()
    }
}
