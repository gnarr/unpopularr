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

    /// Browser-facing base URL as a string, with any embedded credentials,
    /// query, or fragment stripped. Deep-link bases only need
    /// scheme/host/port/path, and this endpoint must never leak a token or
    /// `user:pass@` that a user placed in a configured URL.
    #[must_use]
    pub fn web_url_sanitized(&self) -> String {
        let mut url = self.web_url().clone();
        // set_password/set_username only error for cannot-be-a-base URLs, which
        // config validation already rejects; clear password before username so
        // no stray `@` is left behind.
        let _ = url.set_password(None);
        let _ = url.set_username("");
        url.set_query(None);
        url.set_fragment(None);
        url.into()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn instance(base_url: &str, external_url: Option<&str>) -> Instance {
        Instance {
            id: "id".to_owned(),
            name: "Name".to_owned(),
            kind: InstanceKind::Radarr,
            base_url: Url::parse(base_url).expect("base URL"),
            external_url: external_url.map(|url| Url::parse(url).expect("external URL")),
            api_key: "secret".to_owned(),
            config_order: 0,
        }
    }

    #[test]
    fn web_url_prefers_external_url_then_falls_back_to_base_url() {
        let with_external = instance("http://radarr:7878/", Some("https://radarr.example.com/"));
        assert_eq!(
            with_external.web_url().as_str(),
            "https://radarr.example.com/"
        );

        let without = instance("http://radarr:7878/", None);
        assert_eq!(without.web_url().as_str(), "http://radarr:7878/");
    }

    #[test]
    fn web_url_sanitized_strips_credentials_query_and_fragment() {
        let dirty = instance(
            "https://user:pass@radarr.example.com/radarr/?apikey=secret#frag",
            None,
        );
        assert_eq!(
            dirty.web_url_sanitized(),
            "https://radarr.example.com/radarr/"
        );
    }

    #[test]
    fn web_url_sanitized_leaves_clean_urls_untouched() {
        let clean = instance(
            "http://radarr:7878/",
            Some("https://radarr.example.com/app/"),
        );
        assert_eq!(clean.web_url_sanitized(), "https://radarr.example.com/app/");
    }
}
