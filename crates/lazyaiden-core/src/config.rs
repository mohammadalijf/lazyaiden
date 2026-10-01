use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};

/// Source of environment variables; injectable so precedence is testable.
pub type Env<'a> = &'a dyn Fn(&str) -> Option<String>;

/// The real process environment. Empty values count as unset.
pub fn process_env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}

/// Contents of `config.toml`. Every key is optional.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFile {
    /// Directory holding the local profile YAML files.
    pub profiles_dir: Option<PathBuf>,
    /// Override of the Fellow API root (testing, proxies).
    pub base_url: Option<String>,
    /// The brewer to act on when the account has several.
    pub brewer_id: Option<String>,
}

fn config_error(path: &Path, message: impl ToString) -> CoreError {
    CoreError::Config {
        path: path.into(),
        message: message.to_string(),
    }
}

impl ConfigFile {
    /// Reads the file; a missing file yields the defaults.
    pub fn load(path: &Path) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|e| config_error(path, e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(config_error(path, e)),
        }
    }

    /// Writes the file, creating parent directories.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| config_error(path, e))?;
        }
        let text = toml::to_string_pretty(self).map_err(|e| config_error(path, e))?;
        fs::write(path, text).map_err(|e| config_error(path, e))
    }
}

/// Command-line overrides; the highest-precedence source.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Overrides {
    /// `--config`
    pub config_path: Option<PathBuf>,
    /// `--profiles-dir`
    pub profiles_dir: Option<PathBuf>,
    /// `--brewer`
    pub brewer_id: Option<String>,
    /// `--base-url`
    pub base_url: Option<String>,
}

impl Overrides {
    /// The profiles directory chosen on purpose (`--profiles-dir` or `LAZYAIDEN_PROFILES_DIR`),
    /// ignoring the config file and the default. Demo mode only uses a directory chosen this way.
    pub fn explicit_profiles_dir(&self, env: Env) -> Option<PathBuf> {
        self.profiles_dir
            .clone()
            .or_else(|| env("LAZYAIDEN_PROFILES_DIR").map(PathBuf::from))
            .map(|p| expand_home(p, env))
    }
}

/// Fully resolved configuration: `flag > environment > config file > default`.
///
/// Environment variables: `LAZYAIDEN_CONFIG`, `LAZYAIDEN_PROFILES_DIR`, `LAZYAIDEN_BREWER_ID`, `LAZYAIDEN_BASE_URL`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Location of the config file (it may not exist).
    pub path: PathBuf,
    /// Directory holding the local profile YAML files.
    pub profiles_dir: PathBuf,
    /// API root override, if any.
    pub base_url: Option<String>,
    /// The chosen brewer, if any.
    pub brewer_id: Option<String>,
}

fn expand_home(path: PathBuf, env: Env) -> PathBuf {
    match (path.strip_prefix("~"), env("HOME")) {
        (Ok(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => path,
    }
}

/// `$XDG_CONFIG_HOME/lazyaiden/config.toml`, else `$HOME/.config/lazyaiden/config.toml`.
pub fn default_config_path(env: Env) -> Option<PathBuf> {
    let base = env("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("lazyaiden").join("config.toml"))
}

impl Config {
    /// Resolves the configuration, reading the config file.
    pub fn resolve(overrides: &Overrides, env: Env) -> Result<Self> {
        let path = overrides
            .config_path
            .clone()
            .or_else(|| env("LAZYAIDEN_CONFIG").map(PathBuf::from))
            .map(|p| expand_home(p, env))
            .or_else(|| default_config_path(env))
            .ok_or_else(|| {
                config_error(
                    Path::new("config.toml"),
                    "cannot locate a config directory: HOME is not set",
                )
            })?;
        let file = ConfigFile::load(&path)?;

        let profiles_dir = overrides
            .profiles_dir
            .clone()
            .or_else(|| env("LAZYAIDEN_PROFILES_DIR").map(PathBuf::from))
            .or(file.profiles_dir)
            .map(|p| expand_home(p, env))
            .unwrap_or_else(|| path.parent().unwrap_or(Path::new(".")).join("profiles"));

        Ok(Self {
            profiles_dir,
            base_url: overrides
                .base_url
                .clone()
                .or_else(|| env("LAZYAIDEN_BASE_URL"))
                .or(file.base_url),
            brewer_id: overrides
                .brewer_id
                .clone()
                .or_else(|| env("LAZYAIDEN_BREWER_ID"))
                .or(file.brewer_id),
            path,
        })
    }

    /// Stores `brewer_id` in the config file, keeping its other keys, and updates `self`.
    pub fn persist_brewer(&mut self, brewer_id: &str) -> Result<()> {
        let mut file = ConfigFile::load(&self.path)?;
        file.brewer_id = Some(brewer_id.to_owned());
        file.save(&self.path)?;
        self.brewer_id = Some(brewer_id.to_owned());
        Ok(())
    }
}
