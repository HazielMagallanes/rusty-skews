//! Hyprland IPC client: command-socket queries and the event stream.
//!
//! Socket paths follow the Hyprland layout:
//! `$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/`.
//!
//! Command-socket queries are request/response over `.socket.sock`; the event
//! stream arrives line-by-line on `.socket2.sock` and is parsed with
//! [`parse_event`](crate::parse_event).

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::HyprEvent;

/// Active workspace information returned by the command socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceInfo {
    /// Numeric workspace id.
    pub id: i64,
    /// Workspace name (numeric or named).
    pub name: String,
}

/// Path of the Hyprland command socket (`.socket.sock`).
#[must_use]
pub fn command_socket_path(runtime_dir: &Path, signature: &str) -> PathBuf {
    runtime_dir
        .join("hypr")
        .join(signature)
        .join(".socket.sock")
}

/// Path of the Hyprland event socket (`.socket2.sock`).
#[must_use]
pub fn event_socket_path(runtime_dir: &Path, signature: &str) -> PathBuf {
    runtime_dir
        .join("hypr")
        .join(signature)
        .join(".socket2.sock")
}

/// Queries the active workspace through the command socket.
pub fn active_workspace(runtime_dir: &Path, signature: &str) -> std::io::Result<WorkspaceInfo> {
    let reply = command(runtime_dir, signature, "j/activeworkspace")?;
    parse_workspace_json(&reply)
}

/// Parses a `j/activeworkspace` JSON reply.
pub fn parse_workspace_json(json: &str) -> std::io::Result<WorkspaceInfo> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(invalid_data)?;

    let id = value
        .get("id")
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| invalid_data("missing `id`"))?;
    let name = value
        .get("name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| invalid_data("missing `name`"))?
        .to_owned();

    Ok(WorkspaceInfo { id, name })
}

/// Spawns a thread that streams parsed events into `on_event`.
///
/// The callback returns `false` to stop listening (for example when the
/// receiving end of a channel is gone). The thread reconnects when the socket
/// closes, for example after a compositor restart.
pub fn spawn_event_listener<F>(
    path: PathBuf,
    mut on_event: F,
) -> std::io::Result<std::thread::JoinHandle<()>>
where
    F: FnMut(HyprEvent) -> bool + Send + 'static,
{
    std::thread::Builder::new()
        .name(String::from("hyprland-events"))
        .spawn(move || {
            loop {
                if let Ok(stream) = UnixStream::connect(&path) {
                    let reader = BufReader::new(stream);
                    for line in reader.lines() {
                        match line {
                            Ok(line) => {
                                if !on_event(crate::parse_event(&line)) {
                                    return;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                }

                std::thread::sleep(Duration::from_millis(1000));
            }
        })
}

fn command(runtime_dir: &Path, signature: &str, request: &str) -> std::io::Result<String> {
    let mut stream = UnixStream::connect(command_socket_path(runtime_dir, signature))?;
    stream.write_all(request.as_bytes())?;
    stream.shutdown(std::net::Shutdown::Write)?;

    let mut reply = String::new();
    stream.read_to_string(&mut reply)?;

    Ok(reply)
}

fn invalid_data(message: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message.to_string())
}

#[cfg(test)]
mod tests {
    use super::{command_socket_path, event_socket_path, parse_workspace_json};
    use std::path::Path;

    #[test]
    fn socket_paths_follow_the_hyprland_layout() {
        let runtime_dir = Path::new("/run/user/1000");
        let signature = "abc123";

        assert_eq!(
            command_socket_path(runtime_dir, signature),
            Path::new("/run/user/1000/hypr/abc123/.socket.sock")
        );
        assert_eq!(
            event_socket_path(runtime_dir, signature),
            Path::new("/run/user/1000/hypr/abc123/.socket2.sock")
        );
    }

    #[test]
    fn parses_active_workspace_replies() {
        let info = parse_workspace_json(r##"{"id": 2, "name": "2", "monitor": "eDP-1"}"##).unwrap();

        assert_eq!(info.id, 2);
        assert_eq!(info.name, "2");
    }

    #[test]
    fn rejects_malformed_replies() {
        assert!(parse_workspace_json("not json").is_err());
        assert!(parse_workspace_json(r#"{"name": "2"}"#).is_err());
        assert!(parse_workspace_json(r#"{"id": 2}"#).is_err());
    }
}
