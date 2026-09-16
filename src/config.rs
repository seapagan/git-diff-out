use std::{
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use serde::Deserialize;

use crate::cli::{Cli, QuietOverride};

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub output_dir: PathBuf,
    pub quiet: bool,
    pub base_branch: Option<String>,
    pub clipboard: ClipboardConfig,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClipboardConfig {
    pub osc52_fallback: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("."),
            quiet: false,
            base_branch: None,
            clipboard: ClipboardConfig::default(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(ConfigError::Read(path.to_path_buf(), error)),
        };
        toml::from_str(&contents).map_err(|error| ConfigError::Parse(path.to_path_buf(), error))
    }

    pub fn default_path() -> Result<PathBuf, ConfigError> {
        ProjectDirs::from("", "", "git-diff-out")
            .map(|dirs| dirs.config_dir().join("config.toml"))
            .ok_or(ConfigError::NoConfigDirectory)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct EffectiveConfig {
    pub output_dir: PathBuf,
    pub quiet: bool,
}

impl EffectiveConfig {
    pub fn new(config: &Config, cli: &Cli, cwd: &Path) -> Self {
        let selected = cli.output_dir.as_ref().unwrap_or(&config.output_dir);
        let output_dir = if selected.is_absolute() {
            selected.clone()
        } else {
            cwd.join(selected)
        };
        let quiet = match cli.quiet_override() {
            QuietOverride::None => config.quiet,
            QuietOverride::Quiet => true,
            QuietOverride::Verbose => false,
        };
        Self { output_dir, quiet }
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Read(PathBuf, io::Error),
    Parse(PathBuf, toml::de::Error),
    NoConfigDirectory,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(path, error) => {
                write!(
                    formatter,
                    "cannot read config '{}': {error}",
                    path.display()
                )
            }
            Self::Parse(path, error) => {
                write!(formatter, "invalid config '{}': {error}", path.display())
            }
            Self::NoConfigDirectory => write!(formatter, "cannot determine the config directory"),
        }
    }
}

impl Error for ConfigError {}
