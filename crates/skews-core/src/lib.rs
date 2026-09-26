//! Core domain types shared by every layer of rusty-skews.
//!
//! This crate is intentionally free of I/O, Wayland, GPU and async-runtime
//! dependencies: it must stay usable from any layer and trivially testable.

#![forbid(unsafe_code)]

pub mod color;
pub mod control;
pub mod effect;
pub mod geometry;
pub mod id;
pub mod input;
pub mod notification;

pub use color::Rgba;
pub use control::ControlCommand;
pub use effect::{Action, Effect, Effects, LogLevel};
pub use geometry::{Rect, Size};
pub use id::ModuleId;
pub use input::{InteractionKind, ListSource};
pub use notification::Notification;
