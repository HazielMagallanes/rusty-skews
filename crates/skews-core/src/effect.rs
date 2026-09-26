//! Effects produced by the kernel's `update` step.
//!
//! The kernel follows the *effects-as-data* model: state transitions are pure
//! and return a list of side-effect descriptions that the runtime executes in
//! order. This keeps behavior unit-testable without a display or event loop.

use crate::ModuleId;

/// Severity of a [`Effect::Log`] message.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LogLevel {
    /// Fine-grained diagnostics.
    Debug,
    /// Normal lifecycle information.
    Info,
    /// Something that deserves attention but is recoverable.
    Warn,
    /// Something that failed or will degrade behavior.
    Error,
}

/// Shell-level actions requested by modules.
///
/// The kernel never performs I/O; modules describe the intent and the runtime
/// executes it against the matching service.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Adjust the default audio sink volume by a relative delta (`0.05` = +5 %).
    AdjustVolume(f32),
    /// Set the default audio sink volume to an absolute value (`0.0..=1.0`).
    SetVolume(f32),
    /// Toggle mute on the default audio sink.
    ToggleMute,
    /// Toggle the dropdown panel of a module.
    TogglePanel(ModuleId),
    /// Enable or disable the Wi-Fi radio.
    SetWifi(bool),
    /// Connect to a Wi-Fi network by SSID.
    ConnectWifi(String),
    /// Enable or disable the Bluetooth adapter.
    SetBluetooth(bool),
    /// Connect a Bluetooth device by address.
    ConnectBluetooth(String),
    /// Disconnect a Bluetooth device by address.
    DisconnectBluetooth(String),
}

/// A side effect requested by the kernel.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Request a redraw of every mapped surface.
    Redraw,
    /// Execute a shell action through the runtime.
    Action(Action),
    /// Emit a log line through the shell's tracing pipeline.
    Log {
        /// Severity of the message.
        level: LogLevel,
        /// Human-readable message.
        message: String,
    },
}

/// An ordered list of effects to execute after an update.
pub type Effects = Vec<Effect>;

#[cfg(test)]
mod tests {
    use super::{Effect, Effects, LogLevel};

    #[test]
    fn effects_are_comparable_and_ordered() {
        let effects: Effects = vec![
            Effect::Log {
                level: LogLevel::Info,
                message: String::from("hello"),
            },
            Effect::Redraw,
        ];

        assert_eq!(
            effects[0],
            Effect::Log {
                level: LogLevel::Info,
                message: String::from("hello")
            }
        );
        assert_eq!(effects[1], Effect::Redraw);
    }
}
