//! Unix-socket control channel for a running shell.
//!
//! The shell binds `$XDG_RUNTIME_DIR/rusty-skews/control.sock` and answers
//! one-line commands; `rusty-skews-ctl` writes them. This keeps compositor
//! keybinds trivial:
//!
//! ```text
//! bind = SUPER, D, exec, rusty-skews-ctl toggle-bar
//! ```

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use skews_core::ControlCommand;
use thiserror::Error;

/// Socket directory name inside the runtime directory.
const SOCKET_DIR: &str = "rusty-skews";
/// Socket file name.
const SOCKET_FILE: &str = "control.sock";
/// Upper bound on how long reading a command may block the shell.
const READ_TIMEOUT: Duration = Duration::from_millis(200);

/// Errors produced by the control channel.
#[derive(Debug, Error)]
pub enum ControlError {
    /// The runtime directory is not available.
    #[error("XDG_RUNTIME_DIR is not set; cannot locate the control socket")]
    NoRuntimeDir,
    /// The socket call failed.
    #[error("control socket i/o failed: {0}")]
    Io(#[from] std::io::Error),
    /// The line is not a known command.
    #[error("unknown control command `{0}`")]
    Unknown(String),
    /// The shell is not running (no socket).
    #[error("no shell is listening on {0} (is rusty-skews running?)")]
    NotRunning(PathBuf),
}

/// Path of the control socket.
pub fn socket_path() -> Result<PathBuf, ControlError> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").ok_or(ControlError::NoRuntimeDir)?;
    Ok(PathBuf::from(runtime).join(SOCKET_DIR).join(SOCKET_FILE))
}

/// Binds the shell's control socket, replacing a stale one.
pub fn bind() -> Result<UnixListener, ControlError> {
    bind_at(&socket_path()?)
}

/// Removes the control socket at shutdown.
pub fn cleanup() {
    if let Ok(path) = socket_path() {
        let _ = std::fs::remove_file(path);
    }
}

/// Sends one command to the running shell.
pub fn send(command: ControlCommand) -> Result<(), ControlError> {
    send_to(&socket_path()?, command)
}

/// Accepts and reads one pending command, if a client is waiting.
///
/// Returns `Ok(None)` when no client is pending. The read is bounded by a
/// short timeout so a misbehaving client cannot stall the shell's loop.
pub fn poll_command(listener: &UnixListener) -> Result<Option<ControlCommand>, ControlError> {
    let (stream, _) = match listener.accept() {
        Ok(pair) => pair,
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(None),
        Err(error) => return Err(ControlError::Io(error)),
    };

    stream.set_read_timeout(Some(READ_TIMEOUT))?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line)?;

    if line.trim().is_empty() {
        return Ok(None);
    }

    ControlCommand::parse(&line)
        .map(Some)
        .ok_or_else(|| ControlError::Unknown(line.trim().to_owned()))
}

/// Binds a control socket at an explicit path (tests and embedding).
pub fn bind_at(path: &Path) -> Result<UnixListener, ControlError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // A leftover socket from a crashed run would block the bind.
    if path.exists() {
        std::fs::remove_file(path)?;
    }

    let listener = UnixListener::bind(path)?;
    // The runtime directory is already user-private; be explicit anyway.
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

/// Sends a command to a control socket at an explicit path (tests and embedding).
pub fn send_to(path: &Path, command: ControlCommand) -> Result<(), ControlError> {
    let mut stream = UnixStream::connect(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ControlError::NotRunning(path.to_path_buf())
        } else {
            ControlError::Io(error)
        }
    })?;

    writeln!(stream, "{command}")?;
    stream.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ControlCommand, ControlError, bind_at, poll_command, send_to};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_socket() -> PathBuf {
        let mut path = std::env::temp_dir();
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        path.push(format!(
            "skews-ipc-control-test-{}-{id}.sock",
            std::process::id()
        ));
        path
    }

    fn receive(listener: &std::os::unix::net::UnixListener) -> Option<ControlCommand> {
        for _ in 0..100 {
            match poll_command(listener) {
                Ok(Some(command)) => return Some(command),
                Ok(None) => std::thread::sleep(Duration::from_millis(5)),
                Err(error) => panic!("poll failed: {error}"),
            }
        }
        None
    }

    #[test]
    fn round_trips_a_command() {
        let path = temp_socket();
        let listener = bind_at(&path).expect("bind");

        send_to(&path, ControlCommand::ToggleBar).expect("send");
        assert_eq!(receive(&listener), Some(ControlCommand::ToggleBar));

        send_to(&path, ControlCommand::HideBar).expect("send");
        assert_eq!(receive(&listener), Some(ControlCommand::HideBar));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn reports_an_unknown_command() {
        let path = temp_socket();
        let listener = bind_at(&path).expect("bind");

        std::io::Write::write_all(
            &mut std::os::unix::net::UnixStream::connect(&path).expect("connect"),
            b"open-launcher\n",
        )
        .expect("write");

        // The first poll may find nothing yet, then fails with `Unknown`.
        let mut error = None;
        for _ in 0..100 {
            match poll_command(&listener) {
                Ok(Some(command)) => panic!("unexpected command {command:?}"),
                Ok(None) => std::thread::sleep(Duration::from_millis(5)),
                Err(found) => {
                    error = Some(found);
                    break;
                }
            }
        }

        assert!(matches!(error, Some(ControlError::Unknown(_))));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn reports_a_missing_socket() {
        let path = temp_socket();
        let error = send_to(&path, ControlCommand::ToggleBar).expect_err("no listener");
        assert!(matches!(error, ControlError::NotRunning(_)));
    }

    #[test]
    fn replaces_a_stale_socket_file() {
        let path = temp_socket();
        std::fs::write(&path, b"stale").expect("write stale file");

        let listener = bind_at(&path).expect("bind over stale file");
        send_to(&path, ControlCommand::ShowBar).expect("send");
        assert_eq!(receive(&listener), Some(ControlCommand::ShowBar));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn the_socket_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_socket();
        let _listener = bind_at(&path).expect("bind");
        let mode = std::fs::metadata(&path)
            .expect("metadata")
            .permissions()
            .mode();

        assert_eq!(mode & 0o777, 0o600);
        std::fs::remove_file(&path).ok();
    }
}
