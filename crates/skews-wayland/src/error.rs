//! Wayland integration error types.

/// Errors produced while connecting to or driving the Wayland compositor.
#[derive(Debug, thiserror::Error)]
pub enum WaylandError {
    /// Establishing the Wayland connection failed.
    #[error("failed to connect to the Wayland compositor: {0}")]
    Connect(String),
    /// A protocol required by the shell is not advertised by the compositor.
    #[error("failed to bind required protocol `{protocol}`: {detail}")]
    Bind {
        /// Name of the missing protocol.
        protocol: &'static str,
        /// Underlying bind error.
        detail: String,
    },
    /// Dispatching Wayland events failed.
    #[error("Wayland event dispatch failed: {0}")]
    Dispatch(String),
}
