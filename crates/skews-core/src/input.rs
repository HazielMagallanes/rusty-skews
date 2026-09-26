//! User interactions routed from the runtime to modules.

/// A user interaction with a module, produced by the runtime's hit testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionKind {
    /// Primary (left) click.
    Click,
    /// Secondary (right) click.
    SecondaryClick,
    /// Scroll wheel/touchpad up.
    ScrollUp,
    /// Scroll wheel/touchpad down.
    ScrollDown,
}

/// Which surface a list selection came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListSource {
    /// The module's dropdown panel.
    Panel,
    /// The module's popup (notification toast).
    Popup,
}
