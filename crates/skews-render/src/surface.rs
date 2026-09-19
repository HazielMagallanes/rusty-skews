//! Surface creation and configuration.

use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
};
use wgpu::{
    CompositeAlphaMode, Instance, PresentMode, Surface, SurfaceConfiguration, TextureFormat,
    TextureUsages,
};

use crate::error::RenderError;
use crate::renderer::Renderer;

/// A wgpu surface bound to a Wayland `wl_surface`, plus its configuration.
pub struct GpuSurface {
    surface: Surface<'static>,
    config: Option<SurfaceConfiguration>,
}

impl GpuSurface {
    /// Creates a wgpu surface for a Wayland surface.
    ///
    /// # Safety contract
    ///
    /// The handles must reference a live `wl_display` and `wl_surface`, and
    /// both must outlive the returned `GpuSurface`. Callers obtain them from
    /// Wayland proxies they keep alive for the whole program lifetime (see
    /// `skews_wayland::display_handle` / `window_handle`).
    ///
    /// This is the only place in the workspace where `unsafe` is used; it is
    /// audited and reviewed as such.
    #[allow(unsafe_code)]
    pub fn create(
        instance: &Instance,
        display: WaylandDisplayHandle,
        wl_surface: WaylandWindowHandle,
    ) -> Result<Self, RenderError> {
        let raw_display_handle = RawDisplayHandle::Wayland(display);
        let raw_window_handle = RawWindowHandle::Wayland(wl_surface);

        // SAFETY: the caller guarantees both handles reference live objects
        // that outlive this surface (documented above).
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(raw_display_handle),
                raw_window_handle,
            })?
        };

        Ok(Self {
            surface,
            config: None,
        })
    }

    /// Returns the underlying wgpu surface (needed for adapter selection).
    #[must_use]
    pub fn wgpu_surface(&self) -> &Surface<'static> {
        &self.surface
    }

    /// Returns the current configuration, if the surface has been configured.
    #[must_use]
    pub fn configuration(&self) -> Option<&SurfaceConfiguration> {
        self.config.as_ref()
    }

    /// Configures (or reconfigures) the surface for the given logical size.
    ///
    /// The pixel format is chosen from the renderer's preferred format when
    /// available, otherwise from the surface capabilities.
    pub fn configure(
        &mut self,
        renderer: &Renderer,
        width: u32,
        height: u32,
    ) -> Result<(), RenderError> {
        let capabilities = self.surface.get_capabilities(renderer.adapter());

        let format = renderer
            .preferred_format()
            .filter(|format| capabilities.formats.contains(format))
            .or_else(|| {
                capabilities
                    .formats
                    .iter()
                    .copied()
                    .find(TextureFormat::is_srgb)
            })
            .or_else(|| capabilities.formats.first().copied())
            .unwrap_or(TextureFormat::Bgra8UnormSrgb);

        let alpha_mode = if capabilities
            .alpha_modes
            .contains(&CompositeAlphaMode::PreMultiplied)
        {
            CompositeAlphaMode::PreMultiplied
        } else if capabilities
            .alpha_modes
            .contains(&CompositeAlphaMode::PostMultiplied)
        {
            CompositeAlphaMode::PostMultiplied
        } else {
            CompositeAlphaMode::Auto
        };

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: width.max(1),
            height: height.max(1),
            present_mode: PresentMode::Fifo,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        self.surface.configure(renderer.device(), &config);
        self.config = Some(config);

        Ok(())
    }

    /// Re-applies the stored configuration (after `Outdated`/`Lost` errors).
    pub(crate) fn reconfigure(&mut self, renderer: &Renderer) {
        if let Some(config) = self.config.as_ref() {
            self.surface.configure(renderer.device(), config);
        }
    }

    pub(crate) fn surface(&self) -> &Surface<'static> {
        &self.surface
    }

    pub(crate) fn config(&self) -> Option<&SurfaceConfiguration> {
        self.config.as_ref()
    }
}
