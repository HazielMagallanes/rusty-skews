//! Module trait and module outputs.

use crate::Msg;
use skews_core::{Effects, ModuleId};

/// What a module currently wants to display.
///
/// This is intentionally small for M1 slice A; the UI core replaces `Text`
/// with rich elements while keeping the same trait shape.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ModuleOutput {
    /// Nothing to display.
    #[default]
    Empty,
    /// A short text label.
    Text(String),
    /// A percentage level (0-100) with an optional muted state; used by the
    /// bar as text and by panels as a slider value.
    Level {
        /// Level in `0.0..=100.0`.
        percent: f32,
        /// Whether the source is muted.
        muted: bool,
    },
}

/// A row in a module's list panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    /// Primary text (network name, device name, …).
    pub label: String,
    /// Secondary text (signal, state, …).
    pub detail: String,
    /// Whether this row is the active/selected entry.
    pub active: bool,
}

/// Panel content published by a module.
#[derive(Debug, Clone, PartialEq)]
pub enum PanelContent {
    /// Volume slider panel.
    Volume {
        /// Level in `0.0..=100.0`.
        percent: f32,
        /// Whether the sink is muted.
        muted: bool,
    },
    /// A selectable list panel.
    List {
        /// Panel title.
        title: String,
        /// Rows in display order; selection reports the index.
        items: Vec<ListItem>,
    },
}

/// A shell module: bar widget, panel provider or launcher provider.
pub trait Module: Send {
    /// Stable module id, matching the configuration key.
    fn id(&self) -> &ModuleId;

    /// Handles a message, returning effects to execute.
    fn update(&mut self, msg: &Msg) -> Effects;

    /// Returns the current display output.
    fn output(&self) -> ModuleOutput;

    /// Returns the module's panel content, when it has a panel.
    fn panel(&self) -> Option<PanelContent> {
        None
    }

    /// Returns the module's popup content (toast), when it has one.
    ///
    /// Returning `None` hides the popup; the runtime keeps popups in sync with
    /// this method after every message.
    fn popup(&self) -> Option<PanelContent> {
        None
    }
}
