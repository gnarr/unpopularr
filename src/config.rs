use std::{
    collections::HashSet,
    env, fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use url::Url;

use crate::{
    instances::{Instance, InstanceKind},
    playback::{PlaybackProvider, PlaybackSource},
};

const DEFAULT_CONFIG_PATH: &str = "config.toml";
const DEFAULT_SYNC_INTERVAL_SECONDS: u64 = 6 * 60 * 60;

#[derive(Debug)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub sync: SyncConfig,
    pub instances: Vec<Instance>,
    pub playback: Option<PlaybackConfig>,
    /// Present when `[media_probe]` is configured. Its presence is the enable
    /// flag; there is no separate boolean.
    pub media_probe: Option<MediaProbeConfig>,
}

#[derive(Debug)]
pub struct ServerConfig {
    pub bind: SocketAddr,
}

#[derive(Debug)]
pub struct DatabaseConfig {
    pub path: PathBuf,
}

#[derive(Debug)]
pub struct SyncConfig {
    pub interval: Duration,
    pub run_on_startup: bool,
}

#[derive(Debug)]
pub struct PlaybackConfig {
    pub source: PlaybackSource,
    pub interval: Duration,
    pub run_on_startup: bool,
}

/// Settings for the optional filesystem probe, which reads media file
/// timestamps to catch acquisition dates that predate what the *arr apps know.
#[derive(Debug)]
pub struct MediaProbeConfig {
    /// Prefix rewrites from an *arr instance's view of the filesystem to this
    /// process's. Sorted longest-first, so the most specific rule wins
    /// regardless of the order it was written in. Empty means "same paths".
    pub path_mappings: Vec<PathMapping>,
}

#[derive(Debug)]
pub struct PathMapping {
    pub from: PathBuf,
    pub to: PathBuf,
}

#[derive(Deserialize)]
struct RawConfig {
    #[serde(default)]
    server: RawServerConfig,
    database: RawDatabaseConfig,
    #[serde(default)]
    sync: RawSyncConfig,
    #[serde(default)]
    instances: Vec<RawInstance>,
    playback: Option<RawPlaybackConfig>,
    media_probe: Option<RawMediaProbeConfig>,
}

#[derive(Deserialize)]
#[serde(default)]
struct RawServerConfig {
    bind: String,
}

impl Default for RawServerConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:3000".to_owned(),
        }
    }
}

#[derive(Deserialize)]
struct RawDatabaseConfig {
    path: PathBuf,
}

#[derive(Deserialize)]
#[serde(default)]
struct RawSyncConfig {
    interval_seconds: u64,
    run_on_startup: bool,
}

impl Default for RawSyncConfig {
    fn default() -> Self {
        Self {
            interval_seconds: DEFAULT_SYNC_INTERVAL_SECONDS,
            run_on_startup: true,
        }
    }
}

#[derive(Deserialize)]
struct RawInstance {
    id: String,
    name: String,
    kind: InstanceKind,
    base_url: Url,
    #[serde(default)]
    external_url: Option<Url>,
    api_key_env: String,
}

#[derive(Deserialize)]
struct RawMediaProbeConfig {
    /// Optional: omitted when this process sees media at the same paths the
    /// *arr apps report, which is the common single-compose-stack case.
    #[serde(default)]
    path_mappings: Vec<RawPathMapping>,
}

#[derive(Deserialize)]
struct RawPathMapping {
    from: PathBuf,
    to: PathBuf,
}

#[derive(Deserialize)]
struct RawPlaybackConfig {
    id: String,
    provider: PlaybackProvider,
    base_url: Url,
    api_key_env: String,
    #[serde(default = "default_playback_interval_seconds")]
    interval_seconds: u64,
    #[serde(default = "default_true")]
    run_on_startup: bool,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let path = env::var("UNPOPULARR_CONFIG").unwrap_or_else(|_| DEFAULT_CONFIG_PATH.to_owned());
        Self::load_from(path)
    }

    pub fn load_from(path: impl AsRef<Path>) -> Result<Self> {
        Self::load_from_with_env(path, |name| env::var(name))
    }

    fn load_from_with_env(
        path: impl AsRef<Path>,
        get_env: impl Fn(&str) -> Result<String, env::VarError>,
    ) -> Result<Self> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read configuration from {}", path.display()))?;
        let raw: RawConfig = toml::from_str(&contents)
            .with_context(|| format!("failed to parse configuration from {}", path.display()))?;

        raw.validate(&get_env)
    }
}

impl RawConfig {
    fn validate(
        self,
        get_env: &impl Fn(&str) -> Result<String, env::VarError>,
    ) -> Result<AppConfig> {
        if self.instances.is_empty() {
            bail!("configuration must contain at least one [[instances]] entry");
        }
        if self.sync.interval_seconds == 0 {
            bail!("sync.interval_seconds must be greater than zero");
        }

        let bind = self
            .server
            .bind
            .parse()
            .with_context(|| format!("server.bind is invalid: {}", self.server.bind))?;
        let mut ids = HashSet::new();
        let mut names = HashSet::new();
        let mut instances = Vec::with_capacity(self.instances.len());

        for (index, raw) in self.instances.into_iter().enumerate() {
            validate_identifier("instance", &raw.id)?;
            if raw.name.trim().is_empty() {
                bail!("instance {} has an empty name", raw.id);
            }
            if !ids.insert(raw.id.clone()) {
                bail!("duplicate instance id: {}", raw.id);
            }
            if !names.insert(raw.name.to_lowercase()) {
                bail!("duplicate instance name: {}", raw.name);
            }
            if raw.api_key_env.trim().is_empty() {
                bail!("instance {} api_key_env must not be empty", raw.id);
            }

            let api_key = get_env(&raw.api_key_env).with_context(|| {
                format!(
                    "environment variable {} referenced by instance {} is not set",
                    raw.api_key_env, raw.id
                )
            })?;
            if api_key.trim().is_empty() {
                bail!(
                    "environment variable {} referenced by instance {} is empty",
                    raw.api_key_env,
                    raw.id
                );
            }

            let base_url =
                normalize_base_url(raw.base_url, &format!("instance {} base_url", raw.id))?;
            let external_url = raw
                .external_url
                .map(|url| normalize_base_url(url, &format!("instance {} external_url", raw.id)))
                .transpose()?;

            instances.push(Instance {
                id: raw.id,
                name: raw.name,
                kind: raw.kind,
                base_url,
                external_url,
                api_key,
                config_order: i64::try_from(index).context("too many configured instances")?,
            });
        }

        let playback = self
            .playback
            .map(|raw| validate_playback(raw, get_env))
            .transpose()?;
        let media_probe = self.media_probe.map(validate_media_probe).transpose()?;

        Ok(AppConfig {
            server: ServerConfig { bind },
            database: DatabaseConfig {
                path: self.database.path,
            },
            sync: SyncConfig {
                interval: Duration::from_secs(self.sync.interval_seconds),
                run_on_startup: self.sync.run_on_startup,
            },
            instances,
            playback,
            media_probe,
        })
    }
}

fn validate_media_probe(raw: RawMediaProbeConfig) -> Result<MediaProbeConfig> {
    let mut seen = HashSet::new();
    let mut path_mappings = Vec::with_capacity(raw.path_mappings.len());
    for mapping in raw.path_mappings {
        let from = normalize_mapped_path(mapping.from, "media_probe.path_mappings.from")?;
        let to = normalize_mapped_path(mapping.to, "media_probe.path_mappings.to")?;
        if !seen.insert(from.clone()) {
            bail!(
                "duplicate media_probe.path_mappings.from: {}",
                from.display()
            );
        }
        path_mappings.push(PathMapping { from, to });
    }

    // Longest first, so a rule for /data/media/tv still wins when a broader
    // rule for /data was written above it.
    path_mappings.sort_by_key(|mapping| std::cmp::Reverse(mapping.from.components().count()));

    Ok(MediaProbeConfig { path_mappings })
}

/// Requires an absolute path and strips any trailing separator, so `/data` and
/// `/data/` behave identically. A relative or empty `from` would match
/// everything, so both are rejected rather than normalized.
fn normalize_mapped_path(path: PathBuf, label: &str) -> Result<PathBuf> {
    if !path.is_absolute() {
        bail!("{label} must be an absolute path, got {:?}", path.display());
    }
    Ok(path.components().collect())
}

fn validate_playback(
    raw: RawPlaybackConfig,
    get_env: &impl Fn(&str) -> Result<String, env::VarError>,
) -> Result<PlaybackConfig> {
    validate_identifier("playback source", &raw.id)?;
    if raw.interval_seconds == 0 {
        bail!("playback.interval_seconds must be greater than zero");
    }
    if raw.api_key_env.trim().is_empty() {
        bail!("playback.api_key_env must not be empty");
    }

    let api_key = get_env(&raw.api_key_env).with_context(|| {
        format!(
            "environment variable {} referenced by playback source {} is not set",
            raw.api_key_env, raw.id
        )
    })?;
    if api_key.trim().is_empty() {
        bail!(
            "environment variable {} referenced by playback source {} is empty",
            raw.api_key_env,
            raw.id
        );
    }

    let base_url = normalize_base_url(raw.base_url, "playback.base_url")?;

    Ok(PlaybackConfig {
        source: PlaybackSource {
            id: raw.id,
            provider: raw.provider,
            base_url,
            api_key,
        },
        interval: Duration::from_secs(raw.interval_seconds),
        run_on_startup: raw.run_on_startup,
    })
}

const fn default_playback_interval_seconds() -> u64 {
    DEFAULT_SYNC_INTERVAL_SECONDS
}

const fn default_true() -> bool {
    true
}

/// Validates that `url` is an http/https base URL and normalizes it with a
/// trailing slash so later `Url::join` calls resolve paths as segments rather
/// than replacing the last one. `label` names the field for error messages
/// (e.g. `"instance radarr base_url"`).
fn normalize_base_url(mut url: Url, label: &str) -> Result<Url> {
    if !matches!(url.scheme(), "http" | "https") {
        bail!("{label} must use http or https");
    }
    if url.cannot_be_a_base() {
        bail!("{label} cannot be used as a base URL");
    }
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    Ok(url)
}

fn validate_identifier(entity: &str, id: &str) -> Result<()> {
    if id.is_empty()
        || !id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        bail!(
            "{entity} id {id:?} must contain only ASCII letters, numbers, hyphens, or underscores"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tempfile::tempdir;
    use url::Url;

    use super::AppConfig;

    #[test]
    fn loads_and_normalizes_configuration() {
        let directory = tempdir().expect("temp directory");
        let path = directory.path().join("config.toml");
        fs::write(
            &path,
            r#"
[database]
path = "unpopularr.db"

[[instances]]
id = "radarr-hd"
name = "Radarr HD"
kind = "radarr"
base_url = "http://localhost:7878/radarr"
api_key_env = "UNPOPULARR_TEST_NORMALIZE_RADARR_KEY"
"#,
        )
        .expect("write config");

        let config = AppConfig::load_from_with_env(path, |name| {
            assert_eq!(name, "UNPOPULARR_TEST_NORMALIZE_RADARR_KEY");
            Ok("secret".to_owned())
        })
        .expect("valid config");

        assert_eq!(
            config.instances[0].base_url.as_str(),
            "http://localhost:7878/radarr/"
        );
        assert!(config.instances[0].external_url.is_none());
        assert_eq!(config.sync.interval.as_secs(), 21_600);
        assert!(config.sync.run_on_startup);
        assert!(config.playback.is_none());
    }

    #[test]
    fn normalizes_external_url_and_defaults_to_base_url() {
        let directory = tempdir().expect("temp directory");
        let path = directory.path().join("config.toml");
        fs::write(
            &path,
            r#"
[database]
path = "unpopularr.db"

[[instances]]
id = "sonarr"
name = "Sonarr"
kind = "sonarr"
base_url = "http://sonarr:8989"
external_url = "https://sonarr.example.com/app"
api_key_env = "UNPOPULARR_TEST_EXTERNAL_SONARR_KEY"

[[instances]]
id = "radarr"
name = "Radarr"
kind = "radarr"
base_url = "http://radarr:7878"
api_key_env = "UNPOPULARR_TEST_EXTERNAL_RADARR_KEY"
"#,
        )
        .expect("write config");

        let config =
            AppConfig::load_from_with_env(path, |_| Ok("secret".to_owned())).expect("valid config");

        // Configured external_url is normalized with a trailing slash.
        assert_eq!(
            config.instances[0].external_url.as_ref().map(Url::as_str),
            Some("https://sonarr.example.com/app/")
        );
        assert_eq!(
            config.instances[0].web_url().as_str(),
            "https://sonarr.example.com/app/"
        );
        // Omitted external_url falls back to base_url for browser links.
        assert!(config.instances[1].external_url.is_none());
        assert_eq!(
            config.instances[1].web_url().as_str(),
            "http://radarr:7878/"
        );
    }

    #[test]
    fn rejects_external_url_with_unsupported_scheme() {
        let directory = tempdir().expect("temp directory");
        let path = directory.path().join("config.toml");
        fs::write(
            &path,
            r#"
[database]
path = "unpopularr.db"

[[instances]]
id = "sonarr"
name = "Sonarr"
kind = "sonarr"
base_url = "http://sonarr:8989"
external_url = "ftp://sonarr.example.com"
api_key_env = "UNPOPULARR_TEST_BAD_EXTERNAL_KEY"
"#,
        )
        .expect("write config");

        let error = AppConfig::load_from_with_env(path, |_| Ok("secret".to_owned()))
            .expect_err("invalid external_url");
        assert!(format!("{error:#}").contains("external_url must use http or https"));
    }

    #[test]
    fn loads_optional_playback_configuration_without_exposing_the_key() {
        let directory = tempdir().expect("temp directory");
        let path = directory.path().join("config.toml");
        fs::write(
            &path,
            r#"
[database]
path = "unpopularr.db"

[playback]
id = "plex-main"
provider = "tautulli"
base_url = "http://localhost:8181/tautulli"
api_key_env = "UNPOPULARR_TEST_TAUTULLI_KEY"

[[instances]]
id = "radarr"
name = "Radarr"
kind = "radarr"
base_url = "http://localhost:7878"
api_key_env = "UNPOPULARR_TEST_PLAYBACK_RADARR_KEY"
"#,
        )
        .expect("write config");

        let config = AppConfig::load_from_with_env(path, |name| match name {
            "UNPOPULARR_TEST_PLAYBACK_RADARR_KEY" => Ok("arr-secret".to_owned()),
            "UNPOPULARR_TEST_TAUTULLI_KEY" => Ok("playback-secret".to_owned()),
            _ => panic!("unexpected environment variable lookup: {name}"),
        })
        .expect("valid config");

        let playback = config.playback.expect("playback config");
        assert_eq!(
            playback.source.base_url.as_str(),
            "http://localhost:8181/tautulli/"
        );
        assert_eq!(playback.interval.as_secs(), 21_600);
        assert!(playback.run_on_startup);
        assert!(!format!("{:?}", playback.source).contains("playback-secret"));
    }

    #[test]
    fn rejects_unsupported_playback_providers() {
        let directory = tempdir().expect("temp directory");
        let path = directory.path().join("config.toml");
        fs::write(
            &path,
            r#"
[database]
path = "unpopularr.db"

[playback]
id = "plex-main"
provider = "plex"
base_url = "http://localhost:32400"
api_key_env = "UNPOPULARR_TEST_PLEX_KEY"

[[instances]]
id = "radarr"
name = "Radarr"
kind = "radarr"
base_url = "http://localhost:7878"
api_key_env = "UNPOPULARR_TEST_UNSUPPORTED_RADARR_KEY"
"#,
        )
        .expect("write config");

        let error = AppConfig::load_from_with_env(path, |name| {
            panic!("unexpected environment variable lookup: {name}")
        })
        .expect_err("unsupported provider");

        assert!(format!("{error:#}").contains("tautulli"));
    }

    fn media_probe_config(body: &str) -> anyhow::Result<AppConfig> {
        let directory = tempdir().expect("temp directory");
        let path = directory.path().join("config.toml");
        fs::write(
            &path,
            format!(
                r#"
[database]
path = "unpopularr.db"

{body}

[[instances]]
id = "radarr"
name = "Radarr"
kind = "radarr"
base_url = "http://localhost:7878"
api_key_env = "UNPOPULARR_TEST_PROBE_RADARR_KEY"
"#
            ),
        )
        .expect("write config");

        AppConfig::load_from_with_env(path, |_| Ok("arr-secret".to_owned()))
    }

    #[test]
    fn media_probe_is_off_unless_its_section_is_present() {
        let config = media_probe_config("").expect("valid config");
        assert!(config.media_probe.is_none());

        // The section alone is a valid configuration: it means this process
        // sees media at the same paths the *arr apps report.
        let config = media_probe_config("[media_probe]").expect("valid config");
        let probe = config.media_probe.expect("media probe config");
        assert!(probe.path_mappings.is_empty());
    }

    #[test]
    fn media_probe_mappings_are_normalized_and_sorted_longest_first() {
        let config = media_probe_config(
            r#"[media_probe]
path_mappings = [
  { from = "/data", to = "/mnt/" },
  { from = "/data/media/tv", to = "/media/tv" },
]"#,
        )
        .expect("valid config");

        let mappings = config
            .media_probe
            .expect("media probe config")
            .path_mappings;
        // Declaration order does not decide which rule wins.
        assert_eq!(mappings[0].from, PathBuf::from("/data/media/tv"));
        assert_eq!(mappings[1].from, PathBuf::from("/data"));
        assert_eq!(mappings[1].to, PathBuf::from("/mnt"));
    }

    #[test]
    fn rejects_relative_empty_and_duplicate_media_probe_mappings() {
        // A relative or empty prefix would match every path.
        let error = media_probe_config(
            r#"[media_probe]
path_mappings = [{ from = "data/tv", to = "/media/tv" }]"#,
        )
        .expect_err("relative from");
        assert!(format!("{error:#}").contains("absolute"));

        let error = media_probe_config(
            r#"[media_probe]
path_mappings = [{ from = "", to = "/media/tv" }]"#,
        )
        .expect_err("empty from");
        assert!(format!("{error:#}").contains("absolute"));

        let error = media_probe_config(
            r#"[media_probe]
path_mappings = [
  { from = "/data/tv", to = "/media/tv" },
  { from = "/data/tv/", to = "/media/other" },
]"#,
        )
        .expect_err("duplicate from");
        assert!(format!("{error:#}").contains("duplicate"));
    }
}
