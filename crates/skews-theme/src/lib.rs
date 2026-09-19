//! Design tokens for the shell, resolved from a matugen `colors.json`-style
//! palette with safe built-in defaults.
//!
//! Every token is optional in the input file: missing keys fall back to the
//! built-in palette and unknown keys are ignored, so theme generation can
//! evolve independently from the shell.

#![forbid(unsafe_code)]

use std::path::Path;

use serde_json::Value;
use skews_core::Rgba;

/// Errors produced while loading theme tokens.
#[derive(Debug, thiserror::Error)]
pub enum ThemeError {
    /// The palette file could not be read.
    #[error("failed to read theme file `{path}`: {source}")]
    Io {
        /// Path of the offending file.
        path: std::path::PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },
    /// The palette file is not valid JSON.
    #[error("failed to parse theme file: {0}")]
    Parse(#[from] serde_json::Error),
    /// A palette entry is not a valid hex color.
    #[error("theme token `{token}` is invalid: {source}")]
    InvalidColor {
        /// Name of the offending token.
        token: String,
        /// Underlying color error.
        source: skews_core::color::ColorError,
    },
}

/// Color and typography tokens consumed by the UI layer.
#[derive(Debug, Clone, PartialEq)]
pub struct Tokens {
    /// Base window background.
    pub surface: Rgba,
    /// Slightly elevated container background (bar, cards).
    pub surface_container: Rgba,
    /// Primary text on surfaces.
    pub on_surface: Rgba,
    /// Accent color for highlights and active states.
    pub primary: Rgba,
    /// Secondary accent.
    pub secondary: Rgba,
    /// Tertiary accent.
    pub tertiary: Rgba,
    /// Error/destructive color.
    pub error: Rgba,
    /// Battery indicator color.
    pub battery: Rgba,
    /// CPU temperature color.
    pub temp_cpu: Rgba,
    /// GPU temperature color.
    pub temp_gpu: Rgba,
    /// Memory usage color.
    pub memory: Rgba,
    /// Default UI font family.
    pub font_family: String,
}

impl Default for Tokens {
    fn default() -> Self {
        Self {
            surface: Rgba::from_hex("#1e1e2e").expect("built-in palette is valid"),
            surface_container: Rgba::from_hex("#313244").expect("built-in palette is valid"),
            on_surface: Rgba::from_hex("#cdd6f4").expect("built-in palette is valid"),
            primary: Rgba::from_hex("#89b4fa").expect("built-in palette is valid"),
            secondary: Rgba::from_hex("#cba6f7").expect("built-in palette is valid"),
            tertiary: Rgba::from_hex("#f5c2e7").expect("built-in palette is valid"),
            error: Rgba::from_hex("#f38ba8").expect("built-in palette is valid"),
            battery: Rgba::from_hex("#a6e3a1").expect("built-in palette is valid"),
            temp_cpu: Rgba::from_hex("#d08770").expect("built-in palette is valid"),
            temp_gpu: Rgba::from_hex("#bf616a").expect("built-in palette is valid"),
            memory: Rgba::from_hex("#77b950").expect("built-in palette is valid"),
            font_family: String::from("Roboto"),
        }
    }
}

impl Tokens {
    /// Parses a flat JSON object of `token -> hex color` pairs.
    pub fn from_json(text: &str) -> Result<Self, ThemeError> {
        let value: Value = serde_json::from_str(text)?;
        let mut tokens = Self::default();

        let Some(object) = value.as_object() else {
            return Ok(tokens);
        };

        fn parse_color(token: &str, raw: &str) -> Result<Rgba, ThemeError> {
            Rgba::from_hex(raw).map_err(|source| ThemeError::InvalidColor {
                token: token.to_owned(),
                source,
            })
        }

        for (key, value) in object {
            let Some(raw) = value.as_str() else {
                continue;
            };

            match key.as_str() {
                "font_family" => tokens.font_family = raw.to_owned(),
                "surface" => tokens.surface = parse_color(key, raw)?,
                "surface_container" => tokens.surface_container = parse_color(key, raw)?,
                "on_surface" => tokens.on_surface = parse_color(key, raw)?,
                "primary" => tokens.primary = parse_color(key, raw)?,
                "secondary" => tokens.secondary = parse_color(key, raw)?,
                "tertiary" => tokens.tertiary = parse_color(key, raw)?,
                "error" => tokens.error = parse_color(key, raw)?,
                "battery" => tokens.battery = parse_color(key, raw)?,
                "temp_cpu" => tokens.temp_cpu = parse_color(key, raw)?,
                "temp_gpu" => tokens.temp_gpu = parse_color(key, raw)?,
                "memory" => tokens.memory = parse_color(key, raw)?,
                _ => {}
            }
        }

        Ok(tokens)
    }

    /// Loads tokens from a palette file.
    pub fn load(path: &Path) -> Result<Self, ThemeError> {
        let text = std::fs::read_to_string(path).map_err(|source| ThemeError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_json(&text)
    }
}

/// Returns the default palette path: `$XDG_CACHE_HOME/rusty-skews/colors.json`.
#[must_use]
pub fn default_palette_path() -> std::path::PathBuf {
    palette_path_in(
        std::env::var_os("XDG_CACHE_HOME").map(std::path::PathBuf::from),
        std::env::var_os("HOME").map(std::path::PathBuf::from),
    )
}

/// Pure variant of [`default_palette_path`] for tests and odd environments.
#[must_use]
pub fn palette_path_in(
    cache_home: Option<std::path::PathBuf>,
    home: Option<std::path::PathBuf>,
) -> std::path::PathBuf {
    let base = cache_home
        .filter(|path| path.is_absolute())
        .or_else(|| home.map(|home| home.join(".cache")))
        .unwrap_or_else(|| std::path::PathBuf::from(".cache"));
    base.join("rusty-skews").join("colors.json")
}

#[cfg(test)]
mod tests {
    use super::{ThemeError, Tokens, palette_path_in};
    use skews_core::Rgba;
    use std::path::PathBuf;

    #[test]
    fn default_palette_path_prefers_cache_home() {
        let path = palette_path_in(
            Some(PathBuf::from("/cache")),
            Some(PathBuf::from("/home/me")),
        );

        assert_eq!(path, PathBuf::from("/cache/rusty-skews/colors.json"));
    }

    #[test]
    fn defaults_match_builtin_palette() {
        let tokens = Tokens::default();

        assert_eq!(tokens.primary, Rgba::from_hex("#89b4fa").unwrap());
        assert_eq!(tokens.font_family, "Roboto");
    }

    #[test]
    fn partial_palette_overrides_only_given_tokens() {
        let tokens =
            Tokens::from_json(r##"{"primary": "#ff0000", "font_family": "Inter"}"##).unwrap();

        assert_eq!(tokens.primary, Rgba::from_hex("#ff0000").unwrap());
        assert_eq!(tokens.on_surface, Tokens::default().on_surface);
        assert_eq!(tokens.font_family, "Inter");
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let tokens = Tokens::from_json(r##"{"not_a_token": "#123456"}"##).unwrap();

        assert_eq!(tokens, Tokens::default());
    }

    #[test]
    fn invalid_colors_are_reported_with_token_name() {
        let err = Tokens::from_json(r##"{"primary": "not-a-color"}"##).unwrap_err();

        assert!(matches!(err, ThemeError::InvalidColor { token, .. } if token == "primary"));
    }

    #[test]
    fn malformed_json_fails() {
        assert!(matches!(Tokens::from_json("{"), Err(ThemeError::Parse(_))));
    }
}
