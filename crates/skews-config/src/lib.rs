//! Configuration schema, validation and loading for rusty-skews.
//!
//! The configuration is a single TOML document. Modules are referenced by id
//! only; unknown ids are rejected later by the module registry, while this
//! crate enforces structural rules that do not depend on the registry.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Minimum accepted bar height in pixels.
pub const MIN_BAR_HEIGHT: u32 = 8;
/// Maximum accepted bar height in pixels.
pub const MAX_BAR_HEIGHT: u32 = 240;

/// Errors produced while loading or validating configuration.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The configuration file could not be read.
    #[error("failed to read config file `{path}`: {source}")]
    Io {
        /// Path of the offending file.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },
    /// The configuration file is not valid TOML for our schema.
    #[error("failed to parse config file `{path}`: {source}")]
    Parse {
        /// Path of the offending file.
        path: PathBuf,
        /// Underlying TOML error.
        source: Box<toml::de::Error>,
    },
    /// The configuration parsed but violates structural rules.
    #[error("invalid configuration: {0}")]
    Validation(String),
}

/// Root configuration document.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Bar geometry and region composition.
    #[serde(default)]
    pub bar: BarConfig,
    /// Per-module options, keyed by module id.
    #[serde(default)]
    pub modules: BTreeMap<String, toml::Value>,
}

/// Bar geometry and region composition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BarConfig {
    /// Bar height in pixels.
    #[serde(default = "default_height")]
    pub height: u32,
    /// Output name the bar binds to, or `*` for every output.
    #[serde(default = "default_monitor")]
    pub monitor: String,
    /// Modules placed in the left region.
    #[serde(default = "default_left")]
    pub left: BarRegion,
    /// Modules placed in the center region.
    #[serde(default = "default_center")]
    pub center: BarRegion,
    /// Modules placed in the right region.
    #[serde(default = "default_right")]
    pub right: BarRegion,
}

/// A bar region: an ordered list of module ids.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BarRegion {
    /// Module ids in display order.
    #[serde(default)]
    pub modules: Vec<String>,
}

impl Default for BarConfig {
    fn default() -> Self {
        Self {
            height: default_height(),
            monitor: default_monitor(),
            left: default_left(),
            center: default_center(),
            right: default_right(),
        }
    }
}

impl BarConfig {
    /// Iterates over every configured region with its name.
    pub fn regions(&self) -> [(&'static str, &[String]); 3] {
        [
            ("left", self.left.modules.as_slice()),
            ("center", self.center.modules.as_slice()),
            ("right", self.right.modules.as_slice()),
        ]
    }
}

fn default_height() -> u32 {
    32
}

fn default_monitor() -> String {
    String::from("*")
}

fn region(modules: &[&str]) -> BarRegion {
    BarRegion {
        modules: modules.iter().map(|module| String::from(*module)).collect(),
    }
}

fn default_left() -> BarRegion {
    region(&["workspaces"])
}

/// Returns the default configuration path: `$XDG_CONFIG_HOME/rusty-skews/config.toml`.
#[must_use]
pub fn default_config_path() -> PathBuf {
    config_path_in(
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
        std::env::var_os("HOME").map(PathBuf::from),
    )
}

/// Pure variant of [`default_config_path`] for tests and odd environments.
#[must_use]
pub fn config_path_in(config_home: Option<PathBuf>, home: Option<PathBuf>) -> PathBuf {
    let base = config_home
        .filter(|path| path.is_absolute())
        .or_else(|| home.map(|home| home.join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"));
    base.join("rusty-skews").join("config.toml")
}

fn default_center() -> BarRegion {
    region(&["clock"])
}

fn default_right() -> BarRegion {
    region(&["cpu", "memory", "battery", "volume"])
}

impl Config {
    /// Parses and validates a configuration document from TOML text.
    pub fn from_toml_str(text: &str) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(text).map_err(|source| ConfigError::Parse {
            path: PathBuf::from("<memory>"),
            source: Box::new(source),
        })?;
        config.validate()?;
        Ok(config)
    }

    /// Loads and validates a configuration file.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let config: Self = toml::from_str(&text).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source: Box::new(source),
        })?;
        config.validate()?;
        Ok(config)
    }

    /// Enforces structural rules that do not require the module registry.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let height = self.bar.height;
        if !(MIN_BAR_HEIGHT..=MAX_BAR_HEIGHT).contains(&height) {
            return Err(ConfigError::Validation(format!(
                "`bar.height` must be between {MIN_BAR_HEIGHT} and {MAX_BAR_HEIGHT} (got {height})"
            )));
        }

        if self.bar.monitor.trim().is_empty() {
            return Err(ConfigError::Validation(String::from(
                "`bar.monitor` must not be empty (use \"*\" for all outputs)",
            )));
        }

        let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
        for (region, modules) in self.bar.regions() {
            for module in modules {
                if module.trim().is_empty() {
                    return Err(ConfigError::Validation(format!(
                        "`bar.{region}` contains an empty module id"
                    )));
                }
                if let Some(previous) = seen.insert(module.as_str(), region) {
                    return Err(ConfigError::Validation(format!(
                        "module `{module}` is listed twice (in `bar.{previous}` and `bar.{region}`)"
                    )));
                }
            }
        }

        for key in self.modules.keys() {
            if key.trim().is_empty() {
                return Err(ConfigError::Validation(String::from(
                    "`modules` contains an empty module id",
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, ConfigError, MAX_BAR_HEIGHT, config_path_in};
    use std::path::PathBuf;

    #[test]
    fn default_config_path_prefers_xdg_config_home() {
        let path = config_path_in(Some(PathBuf::from("/xdg")), Some(PathBuf::from("/home/me")));

        assert_eq!(path, PathBuf::from("/xdg/rusty-skews/config.toml"));
    }

    #[test]
    fn default_config_path_falls_back_to_home() {
        let path = config_path_in(None, Some(PathBuf::from("/home/me")));

        assert_eq!(
            path,
            PathBuf::from("/home/me/.config/rusty-skews/config.toml")
        );
    }

    #[test]
    fn empty_document_yields_defaults() {
        let config = Config::from_toml_str("").unwrap();

        assert_eq!(config.bar.height, 32);
        assert_eq!(config.bar.monitor, "*");
        assert_eq!(config.bar.left.modules, vec!["workspaces"]);
        assert_eq!(config.bar.center.modules, vec!["clock"]);
        assert_eq!(
            config.bar.right.modules,
            vec!["cpu", "memory", "battery", "volume"]
        );
        assert!(config.modules.is_empty());
    }

    #[test]
    fn full_document_parses() {
        let config = Config::from_toml_str(
            r##"
            [bar]
            height = 40
            monitor = "DP-1"

            [bar.left]
            modules = ["workspaces", "media"]
            [bar.center]
            modules = ["clock"]
            [bar.right]
            modules = ["cpu", "battery", "tray"]

            [modules.battery]
            format = "{percent}%"
            warn_below = 20
            "##,
        )
        .unwrap();

        assert_eq!(config.bar.height, 40);
        assert_eq!(config.bar.monitor, "DP-1");
        assert!(config.modules.contains_key("battery"));
    }

    #[test]
    fn duplicate_modules_are_rejected() {
        let err = Config::from_toml_str(
            r#"
            [bar.left]
            modules = ["clock"]
            [bar.center]
            modules = ["clock"]
            [bar.right]
            modules = []
            "#,
        )
        .unwrap_err();

        assert!(
            matches!(err, ConfigError::Validation(message) if message.contains("listed twice"))
        );
    }

    #[test]
    fn out_of_range_height_is_rejected() {
        for height in [0, MAX_BAR_HEIGHT + 1] {
            let text = format!("[bar]\nheight = {height}\n");
            let err = Config::from_toml_str(&text).unwrap_err();

            assert!(
                matches!(err, ConfigError::Validation(message) if message.contains("bar.height"))
            );
        }
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let err = Config::from_toml_str("[bar]\nheigth = 32\n").unwrap_err();

        assert!(matches!(err, ConfigError::Parse { .. }));
    }

    #[test]
    fn empty_module_ids_are_rejected() {
        let err = Config::from_toml_str(
            r#"
            [bar.left]
            modules = [""]
            [bar.center]
            modules = []
            [bar.right]
            modules = []
            "#,
        )
        .unwrap_err();

        assert!(
            matches!(err, ConfigError::Validation(message) if message.contains("empty module id"))
        );
    }
}
