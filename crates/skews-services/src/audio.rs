//! Audio control for the default sink through WirePlumber's `wpctl`.
//!
//! `wpctl` is a documented CLI exception (ADR-0005): it is invoked with an
//! argument array, never through a shell, and only with fixed arguments plus a
//! numeric volume. The native `pipewire-rs` path can replace this adapter
//! without touching the modules that consume it.

use std::process::Command;

use thiserror::Error;

/// Errors produced by the audio adapter.
#[derive(Debug, Error)]
pub enum AudioError {
    /// `wpctl` could not be spawned.
    #[error("failed to run wpctl: {0}")]
    Io(#[from] std::io::Error),
    /// `wpctl` exited with a failure status.
    #[error("wpctl exited with status {status}: {stderr}")]
    CommandFailed {
        /// Exit code reported by the shell.
        status: i32,
        /// Captured stderr.
        stderr: String,
    },
    /// `wpctl` output was not in the expected format.
    #[error("unexpected wpctl output: {0:?}")]
    Parse(String),
}

/// State of the default audio sink.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioStatus {
    /// Volume in `0.0..=1.0`.
    pub volume: f32,
    /// Whether the sink is muted.
    pub muted: bool,
}

/// Reads the default sink state.
pub fn status() -> Result<AudioStatus, AudioError> {
    let output = Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .output()?;

    if !output.status.success() {
        return Err(AudioError::CommandFailed {
            status: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }

    parse_status(&String::from_utf8_lossy(&output.stdout))
}

/// Parses `wpctl get-volume` output: `Volume: 0.65` or `Volume: 0.65 [MUTED]`.
pub fn parse_status(text: &str) -> Result<AudioStatus, AudioError> {
    let text = text.trim();
    let value = text
        .strip_prefix("Volume:")
        .ok_or_else(|| AudioError::Parse(text.to_owned()))?
        .trim();

    let (number, rest) = match value.split_once(char::is_whitespace) {
        Some((number, rest)) => (number, rest.trim()),
        None => (value, ""),
    };

    let volume: f32 = number
        .parse()
        .map_err(|_| AudioError::Parse(text.to_owned()))?;

    Ok(AudioStatus {
        volume: clamp_volume(volume),
        muted: rest.contains("[MUTED]"),
    })
}

/// Adjusts the default sink volume by a relative delta.
pub fn adjust_volume(delta: f32) -> Result<(), AudioError> {
    let magnitude = delta.abs();
    let argument = if delta >= 0.0 {
        format!("{magnitude}+")
    } else {
        format!("{magnitude}-")
    };

    run_wpctl(&["set-volume", "@DEFAULT_AUDIO_SINK@", &argument])
}

/// Toggles mute on the default sink.
pub fn toggle_mute() -> Result<(), AudioError> {
    run_wpctl(&["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"])
}

/// Clamps a volume to the valid range.
#[must_use]
pub fn clamp_volume(volume: f32) -> f32 {
    volume.clamp(0.0, 1.0)
}

fn run_wpctl(args: &[&str]) -> Result<(), AudioError> {
    let output = Command::new("wpctl").args(args).output()?;

    if !output.status.success() {
        return Err(AudioError::CommandFailed {
            status: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AudioError, clamp_volume, parse_status};

    #[test]
    fn parses_plain_volume() {
        let status = parse_status("Volume: 0.65\n").unwrap();

        assert!((status.volume - 0.65).abs() < 1e-6);
        assert!(!status.muted);
    }

    #[test]
    fn parses_muted_volume() {
        let status = parse_status("Volume: 1.00 [MUTED]\n").unwrap();

        assert!((status.volume - 1.0).abs() < 1e-6);
        assert!(status.muted);
    }

    #[test]
    fn rejects_unexpected_output() {
        assert!(matches!(parse_status("garbage"), Err(AudioError::Parse(_))));
        assert!(matches!(
            parse_status("Volume: loud"),
            Err(AudioError::Parse(_))
        ));
    }

    #[test]
    fn clamps_out_of_range_volumes() {
        assert_eq!(clamp_volume(1.5), 1.0);
        assert_eq!(clamp_volume(-0.2), 0.0);
        assert_eq!(clamp_volume(0.4), 0.4);
    }
}
