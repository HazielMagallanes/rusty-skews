//! Built-in modules for rusty-skews.
//!
//! Every module is registered in the compile-time registry (ADR-0004) by
//! [`register_all`]; adding a module means adding a file and one line here.

#![forbid(unsafe_code)]

pub mod battery;
pub mod clock;
pub mod cpu;
pub mod memory;
pub mod volume;
pub mod workspaces;

pub use battery::Battery;
pub use clock::Clock;
pub use cpu::CpuTemp;
pub use memory::Memory;
pub use volume::Volume;
pub use workspaces::Workspaces;

/// Registers every built-in module in the registry.
pub fn register_all(registry: &mut skews_app::Registry) {
    clock::register(registry);
    cpu::register(registry);
    memory::register(registry);
    battery::register(registry);
    volume::register(registry);
    workspaces::register(registry);
}
