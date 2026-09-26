//! Control commands accepted by a running shell over its Unix socket.
//!
//! The wire format is one command per line, using the same spelling as the
//! CLI: `toggle-bar`, `show-bar`, `hide-bar`.

use std::fmt;
use std::str::FromStr;

/// A command sent to a running shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCommand {
    /// Hide the bar if visible, show it if hidden.
    ToggleBar,
    /// Show the bar.
    ShowBar,
    /// Hide the bar.
    HideBar,
}

impl ControlCommand {
    /// Every command, in CLI help order.
    pub const ALL: [Self; 3] = [Self::ToggleBar, Self::ShowBar, Self::HideBar];

    /// Wire and CLI spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ToggleBar => "toggle-bar",
            Self::ShowBar => "show-bar",
            Self::HideBar => "hide-bar",
        }
    }

    /// Parses the wire and CLI spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "toggle-bar" => Some(Self::ToggleBar),
            "show-bar" => Some(Self::ShowBar),
            "hide-bar" => Some(Self::HideBar),
            _ => None,
        }
    }
}

impl FromStr for ControlCommand {
    type Err = ();

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text).ok_or(())
    }
}

impl fmt::Display for ControlCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::ControlCommand;

    #[test]
    fn round_trips_every_command() {
        for command in ControlCommand::ALL {
            assert_eq!(ControlCommand::parse(command.as_str()), Some(command));
            assert_eq!(command.to_string(), command.as_str());
        }
    }

    #[test]
    fn tolerates_surrounding_whitespace() {
        assert_eq!(
            ControlCommand::parse("  toggle-bar\n"),
            Some(ControlCommand::ToggleBar)
        );
    }

    #[test]
    fn rejects_unknown_commands() {
        assert_eq!(ControlCommand::parse("open-launcher"), None);
        assert_eq!(ControlCommand::parse(""), None);
        assert_eq!(
            "toggle-bar".parse::<ControlCommand>(),
            Ok(ControlCommand::ToggleBar)
        );
    }
}
