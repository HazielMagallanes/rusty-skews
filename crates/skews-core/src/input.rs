//! User interactions routed from the runtime to modules.

/// A user interaction with a module, produced by the runtime's hit testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionKind {
    /// Primary (left) click.
    Click,
    /// Scroll wheel/touchpad up.
    ScrollUp,
    /// Scroll wheel/touchpad down.
    ScrollDown,
}
