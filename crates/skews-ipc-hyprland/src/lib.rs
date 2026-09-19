//! Typed parsing of Hyprland IPC events.
//!
//! Hyprland emits newline-delimited events of the form `kind>>payload` on its
//! second IPC socket. This crate turns those lines into a typed [`HyprEvent`]
//! without any I/O, so parsing is directly unit-testable.

#![forbid(unsafe_code)]

pub mod client;

pub use client::{WorkspaceInfo, active_workspace, event_socket_path, spawn_event_listener};

/// A single Hyprland IPC event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HyprEvent {
    /// A workspace became active (`workspace>>name`).
    Workspace {
        /// Workspace name (may be numeric or named).
        name: String,
    },
    /// The active window changed (`activewindow>>class,title`).
    ActiveWindow {
        /// Window class / app id.
        class: String,
        /// Window title.
        title: String,
    },
    /// A monitor gained focus (`focusedmon>>monitor,workspace`).
    FocusedMonitor {
        /// Monitor name.
        monitor: String,
        /// Workspace that became focused.
        workspace: String,
    },
    /// A window opened (`openwindow>>address,workspace,class,title`).
    OpenWindow {
        /// Window address (hex handle).
        address: String,
        /// Workspace the window opened on.
        workspace: String,
        /// Window class / app id.
        class: String,
        /// Window title.
        title: String,
    },
    /// A window closed (`closewindow>>address`).
    CloseWindow {
        /// Window address (hex handle).
        address: String,
    },
    /// Any event kind this crate does not model yet.
    Unknown {
        /// Event kind as reported by the compositor.
        kind: String,
        /// Raw payload.
        payload: String,
    },
}

/// Parses one Hyprland IPC event line.
///
/// Unknown kinds and malformed payloads are reported as
/// [`HyprEvent::Unknown`]; this function never panics.
#[must_use]
pub fn parse_event(line: &str) -> HyprEvent {
    let Some((kind, payload)) = line.split_once(">>") else {
        return HyprEvent::Unknown {
            kind: line.trim().to_owned(),
            payload: String::new(),
        };
    };

    match kind {
        "workspace" => HyprEvent::Workspace {
            name: payload.to_owned(),
        },
        "closewindow" => HyprEvent::CloseWindow {
            address: payload.to_owned(),
        },
        "activewindow" => split_pair(payload)
            .map(|(class, title)| HyprEvent::ActiveWindow { class, title })
            .unwrap_or_else(|| unknown(kind, payload)),
        "focusedmon" => split_pair(payload)
            .map(|(monitor, workspace)| HyprEvent::FocusedMonitor { monitor, workspace })
            .unwrap_or_else(|| unknown(kind, payload)),
        "openwindow" => {
            let mut parts = payload.splitn(4, ',');
            match (parts.next(), parts.next(), parts.next(), parts.next()) {
                (Some(address), Some(workspace), Some(class), Some(title)) => {
                    HyprEvent::OpenWindow {
                        address: address.to_owned(),
                        workspace: workspace.to_owned(),
                        class: class.to_owned(),
                        title: title.to_owned(),
                    }
                }
                _ => unknown(kind, payload),
            }
        }
        _ => unknown(kind, payload),
    }
}

fn split_pair(payload: &str) -> Option<(String, String)> {
    if payload.is_empty() {
        return Some((String::new(), String::new()));
    }
    payload
        .split_once(',')
        .map(|(a, b)| (a.to_owned(), b.to_owned()))
}

fn unknown(kind: &str, payload: &str) -> HyprEvent {
    HyprEvent::Unknown {
        kind: kind.to_owned(),
        payload: payload.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{HyprEvent, parse_event};

    #[test]
    fn parses_workspace() {
        assert_eq!(
            parse_event("workspace>>2"),
            HyprEvent::Workspace {
                name: String::from("2")
            }
        );
        assert_eq!(
            parse_event("workspace>>web"),
            HyprEvent::Workspace {
                name: String::from("web")
            }
        );
    }

    #[test]
    fn parses_active_window_and_keeps_commas_in_title() {
        assert_eq!(
            parse_event("activewindow>>firefox,Mozilla Firefox"),
            HyprEvent::ActiveWindow {
                class: String::from("firefox"),
                title: String::from("Mozilla Firefox"),
            }
        );
        assert_eq!(
            parse_event("activewindow>>kitty,a, b, c"),
            HyprEvent::ActiveWindow {
                class: String::from("kitty"),
                title: String::from("a, b, c"),
            }
        );
    }

    #[test]
    fn parses_empty_active_window() {
        assert_eq!(
            parse_event("activewindow>>"),
            HyprEvent::ActiveWindow {
                class: String::new(),
                title: String::new()
            }
        );
    }

    #[test]
    fn parses_focused_monitor() {
        assert_eq!(
            parse_event("focusedmon>>eDP-1,1"),
            HyprEvent::FocusedMonitor {
                monitor: String::from("eDP-1"),
                workspace: String::from("1"),
            }
        );
    }

    #[test]
    fn parses_open_and_close_window() {
        assert_eq!(
            parse_event("openwindow>>55607d7d6a20,2,firefox,Mozilla Firefox"),
            HyprEvent::OpenWindow {
                address: String::from("55607d7d6a20"),
                workspace: String::from("2"),
                class: String::from("firefox"),
                title: String::from("Mozilla Firefox"),
            }
        );
        assert_eq!(
            parse_event("closewindow>>55607d7d6a20"),
            HyprEvent::CloseWindow {
                address: String::from("55607d7d6a20")
            }
        );
    }

    #[test]
    fn malformed_lines_become_unknown() {
        assert!(matches!(parse_event("garbage"), HyprEvent::Unknown { .. }));
        assert!(matches!(
            parse_event("activewindow>>no-comma"),
            HyprEvent::Unknown { .. }
        ));
        assert!(matches!(
            parse_event("openwindow>>too,few"),
            HyprEvent::Unknown { .. }
        ));
        assert!(matches!(parse_event(""), HyprEvent::Unknown { .. }));
    }
}
