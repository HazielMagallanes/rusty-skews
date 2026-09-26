//! MPRIS media player status and control over D-Bus.
//!
//! Enumerates `org.mpris.MediaPlayer2.*` bus names, prefers the player that is
//! actually playing, and reads metadata through
//! `org.freedesktop.DBus.Properties` (ADR-0005: protocol, not CLI). The
//! [`MediaSource`] trait keeps modules testable with fakes.

use std::collections::HashMap;

use thiserror::Error;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedValue;

/// Errors produced by the media adapter.
#[derive(Debug, Error)]
pub enum MediaError {
    /// The D-Bus call failed.
    #[error("mpris call failed: {0}")]
    Dbus(#[from] zbus::Error),
    /// No player is available.
    #[error("no media player is running")]
    NoPlayer,
}

/// Playback state reported by the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackStatus {
    /// Audio is playing.
    Playing,
    /// Playback is paused.
    Paused,
    /// Playback is stopped.
    Stopped,
}

/// One player's current state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerStatus {
    /// Player bus name without the `org.mpris.MediaPlayer2.` prefix.
    pub player: String,
    /// Playback state.
    pub status: PlaybackStatus,
    /// Track title (`xesam:title`).
    pub title: String,
    /// First listed artist (`xesam:artist`).
    pub artist: String,
}

/// Port for media status and transport control.
pub trait MediaSource: Send + Sync {
    /// Reads the active player's status, preferring one that is playing.
    fn status(&self) -> Result<Option<PlayerStatus>, MediaError>;
    /// Toggles play/pause on the active player.
    fn play_pause(&self) -> Result<(), MediaError>;
    /// Skips to the next track.
    fn next(&self) -> Result<(), MediaError>;
    /// Returns to the previous track.
    fn previous(&self) -> Result<(), MediaError>;
}

/// MPRIS-backed source.
pub struct Mpris;

const PLAYER_PREFIX: &str = "org.mpris.MediaPlayer2.";
const PLAYER_PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER_INTERFACE: &str = "org.mpris.MediaPlayer2.Player";

impl Mpris {
    fn connection() -> Result<Connection, MediaError> {
        Ok(Connection::session()?)
    }

    fn players(conn: &Connection) -> Result<Vec<String>, MediaError> {
        let reply = conn.call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "ListNames",
            &(),
        )?;
        let names: Vec<String> = reply.body().deserialize()?;

        let mut players: Vec<String> = names
            .into_iter()
            .filter(|name| name.starts_with(PLAYER_PREFIX))
            .collect();
        players.sort();
        Ok(players)
    }

    fn status_of(conn: &Connection, name: &str) -> Result<PlayerStatus, MediaError> {
        let proxy = Proxy::new(conn, name, PLAYER_PATH, PLAYER_INTERFACE)?;

        let raw: String = proxy.get_property("PlaybackStatus")?;
        let status = match raw.as_str() {
            "Playing" => PlaybackStatus::Playing,
            "Paused" => PlaybackStatus::Paused,
            _ => PlaybackStatus::Stopped,
        };

        let metadata: HashMap<String, OwnedValue> =
            proxy.get_property("Metadata").unwrap_or_default();
        let title = metadata
            .get("xesam:title")
            .and_then(|value| String::try_from(value.clone()).ok())
            .unwrap_or_default();
        let artist = metadata
            .get("xesam:artist")
            .and_then(|value| Vec::<String>::try_from(value.clone()).ok())
            .and_then(|artists| artists.into_iter().next())
            .unwrap_or_default();

        Ok(PlayerStatus {
            player: name.trim_start_matches(PLAYER_PREFIX).to_owned(),
            status,
            title,
            artist,
        })
    }

    /// Picks the playing player, falling back to the first one available.
    fn active_player(conn: &Connection) -> Result<Option<String>, MediaError> {
        let mut fallback = None;

        for name in Self::players(conn)? {
            match Self::status_of(conn, &name) {
                Ok(status) if status.status == PlaybackStatus::Playing => return Ok(Some(name)),
                Ok(_) => {
                    fallback.get_or_insert(name);
                }
                // A player that disappeared mid-enumeration is skipped.
                Err(_) => {}
            }
        }

        Ok(fallback)
    }

    fn call(&self, method: &str) -> Result<(), MediaError> {
        let conn = Self::connection()?;
        let name = Self::active_player(&conn)?.ok_or(MediaError::NoPlayer)?;
        let proxy = Proxy::new(&conn, name, PLAYER_PATH, PLAYER_INTERFACE)?;
        proxy.call_method(method, &())?;
        Ok(())
    }
}

impl MediaSource for Mpris {
    fn status(&self) -> Result<Option<PlayerStatus>, MediaError> {
        let conn = Self::connection()?;
        match Self::active_player(&conn)? {
            Some(name) => Self::status_of(&conn, &name).map(Some),
            None => Ok(None),
        }
    }

    fn play_pause(&self) -> Result<(), MediaError> {
        self.call("PlayPause")
    }

    fn next(&self) -> Result<(), MediaError> {
        self.call("Next")
    }

    fn previous(&self) -> Result<(), MediaError> {
        self.call("Previous")
    }
}
