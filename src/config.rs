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

use crate::instances::{Instance, InstanceKind};

const DEFAULT_CONFIG_PATH: &str = "config.toml";
const DEFAULT_SYNC_INTERVAL_SECONDS: u64 = 6 * 60 * 60;

#[derive(Debug)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub sync: SyncConfig,
    pub instances: Vec<Instance>,
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

#[derive(Deserialize)]
struct RawConfig {
    #[serde(default)]
    server: RawServerConfig,
    database: RawDatabaseConfig,
    #[serde(default)]
    sync: RawSyncConfig,
    #[serde(default)]
    instances: Vec<RawInstance>,
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
    api_key_env: String,
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
            validate_identifier(&raw.id)?;
            if raw.name.trim().is_empty() {
                bail!("instance {} has an empty name", raw.id);
            }
            if !ids.insert(raw.id.clone()) {
                bail!("duplicate instance id: {}", raw.id);
            }
            if !names.insert(raw.name.to_lowercase()) {
                bail!("duplicate instance name: {}", raw.name);
            }
            if !matches!(raw.base_url.scheme(), "http" | "https") {
                bail!("instance {} base_url must use http or https", raw.id);
            }
            if raw.base_url.cannot_be_a_base() {
                bail!("instance {} base_url cannot be used as a base URL", raw.id);
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

            let mut base_url = raw.base_url;
            if !base_url.path().ends_with('/') {
                let path = format!("{}/", base_url.path());
                base_url.set_path(&path);
            }

            instances.push(Instance {
                id: raw.id,
                name: raw.name,
                kind: raw.kind,
                base_url,
                api_key,
                config_order: i64::try_from(index).context("too many configured instances")?,
            });
        }

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
        })
    }
}

fn validate_identifier(id: &str) -> Result<()> {
    if id.is_empty()
        || !id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        bail!(
            "instance id {id:?} must contain only ASCII letters, numbers, hyphens, or underscores"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

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
api_key_env = "UNPOPULARR_TEST_RADARR_KEY"
"#,
        )
        .expect("write config");

        let config = AppConfig::load_from_with_env(path, |name| {
            assert_eq!(name, "UNPOPULARR_TEST_RADARR_KEY");
            Ok("secret".to_owned())
        })
        .expect("valid config");

        assert_eq!(
            config.instances[0].base_url.as_str(),
            "http://localhost:7878/radarr/"
        );
        assert_eq!(config.sync.interval.as_secs(), 21_600);
        assert!(config.sync.run_on_startup);
    }
}
