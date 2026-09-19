//! wgpu scene renderer for rusty-skews.
//!
//! The renderer is deliberately small and demand-driven: callers configure a
//! [`GpuSurface`] for each Wayland surface and call [`Renderer::render_scene`]
//! only when the scene is dirty. There are no render loops and no per-frame
//! allocations on the steady path.
//!
//! # Safety
//!
//! `unsafe` is forbidden in every crate of the workspace except here, where it
//! is allowed on a single documented function that bridges Wayland raw handles
//! into wgpu (see [`GpuSurface::create`]).

mod atlas;
mod error;
mod renderer;
mod surface;

pub use error::RenderError;
pub use renderer::Renderer;
pub use surface::GpuSurface;
