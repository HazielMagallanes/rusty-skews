//! Wayland layer-surface glue for rusty-skews, built on smithay-client-toolkit.

#![forbid(unsafe_code)]

mod error;
mod shell;

pub use error::WaylandError;
pub use shell::{
    BarEvent, BarOptions, BarShell, PANEL_GAP, PanelOptions, PointerKind, display_handle,
    window_handle,
};
/// Re-exported keysym type for key comparisons in the runtime.
pub use smithay_client_toolkit::seat::keyboard::Keysym;

/// Re-exported Wayland client types used by the runtime.
pub use smithay_client_toolkit::reexports::client::{
    Connection, EventQueue, Proxy, QueueHandle,
    backend::ObjectId,
    globals::{GlobalList, registry_queue_init},
};
/// Re-exported layer types used by the runtime.
pub use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
