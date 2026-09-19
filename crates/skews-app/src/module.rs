//! Module trait and module outputs.

use crate::Msg;
use skews_core::{Effects, ModuleId};

/// What a module currently wants to display.
///
/// This is intentionally small for M1 slice A; the UI core replaces `Text`
/// with rich elements while keeping the same trait shape.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ModuleOutput {
    /// Nothing to display.
    #[default]
    Empty,
    /// A short text label.
    Text(String),
}

/// A shell module: bar widget, panel provider or launcher provider.
pub trait Module: Send {
    /// Stable module id, matching the configuration key.
    fn id(&self) -> &ModuleId;

    /// Handles a message, returning effects to execute.
    fn update(&mut self, msg: &Msg) -> Effects;

    /// Returns the current display output.
    fn output(&self) -> ModuleOutput;
}
