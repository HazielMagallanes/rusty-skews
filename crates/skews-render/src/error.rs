//! Renderer error types.

/// Errors produced by the renderer.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// No GPU adapter compatible with the requirements was found.
    #[error("no compatible GPU adapter found")]
    NoAdapter,
    /// Requesting the logical device failed.
    #[error("failed to request GPU device: {0}")]
    RequestDevice(#[from] wgpu::RequestDeviceError),
    /// Creating the presentation surface failed.
    #[error("failed to create wgpu surface: {0}")]
    CreateSurface(#[from] wgpu::CreateSurfaceError),
    /// The swapchain could not be acquired for this frame.
    #[error("surface unavailable: {0}")]
    SurfaceUnavailable(&'static str),
    /// The surface was rendered before being configured.
    #[error("surface is not configured yet")]
    NotConfigured,
}
