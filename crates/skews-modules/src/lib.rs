//! Built-in modules for rusty-skews.
//!
//! Every module is registered in the compile-time registry (ADR-0004) by
//! [`register_all`]; adding a module means adding a file and one line here.

#![forbid(unsafe_code)]

pub mod clock;
pub mod cpu;
pub mod memory;
pub mod stubs;

pub use clock::Clock;
pub use cpu::CpuTemp;
pub use memory::Memory;

/// Registers every built-in module in the registry.
pub fn register_all(registry: &mut skews_app::Registry) {
    clock::register(registry);
    cpu::register(registry);
    memory::register(registry);
    stubs::register_all(registry);
}
