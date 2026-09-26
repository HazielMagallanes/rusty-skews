//! System service adapters for rusty-skews.
//!
//! Each adapter is a thin, testable wrapper over a protocol or a documented
//! CLI exception (ADR-0005). Modules consume these through the kernel's
//! action effects; the adapters themselves never touch the UI.

#![forbid(unsafe_code)]

pub mod audio;
