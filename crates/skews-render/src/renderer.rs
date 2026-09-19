//! The wgpu renderer: pipeline management and frame submission.

use std::collections::HashMap;
use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BlendState, Buffer, BufferBinding,
    BufferBindingType, BufferDescriptor, BufferUsages, ColorTargetState, ColorWrites,
    CommandEncoderDescriptor, Device, DeviceDescriptor, FragmentState, Instance, LoadOp,
    MultisampleState, Operations, PipelineLayoutDescriptor, PrimitiveState, Queue, RenderPass,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor,
    RequestAdapterOptions, ShaderModuleDescriptor, ShaderSource, ShaderStages, StoreOp,
    TextureFormat, TextureViewDescriptor, VertexState,
};

use skews_core::{Rect, Rgba};

use crate::error::RenderError;
use crate::surface::GpuSurface;

/// Description of the single rounded rectangle this renderer draws.
///
/// The M0 renderer intentionally supports one shape; the SDF quad batcher
/// arrives with the UI layer in M1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameParams {
    /// Background color filling the whole surface.
    pub clear: Rgba,
    /// Rectangle in physical pixels, top-left origin.
    pub rect: Rect,
    /// Fill color of the rectangle.
    pub rect_color: Rgba,
    /// Corner radius in physical pixels.
    pub radius: f32,
}

impl FrameParams {
    /// Convenience constructor for a full-surface clear without a shape.
    #[must_use]
    pub fn clear_only(clear: Rgba) -> Self {
        Self {
            clear,
            rect: Rect::ZERO,
            rect_color: Rgba::TRANSPARENT,
            radius: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct FrameUniform {
    clear: [f32; 4],
    rect: [f32; 4],
    color: [f32; 4],
    radius: f32,
    _padding: [f32; 3],
}

impl From<&FrameParams> for FrameUniform {
    fn from(params: &FrameParams) -> Self {
        Self {
            clear: params.clear.to_array(),
            rect: [
                params.rect.x as f32,
                params.rect.y as f32,
                params.rect.width as f32,
                params.rect.height as f32,
            ],
            color: params.rect_color.to_array(),
            radius: params.radius,
            _padding: [0.0; 3],
        }
    }
}

/// A wgpu device with the shell's pipelines, created once per process.
pub struct Renderer {
    adapter: wgpu::Adapter,
    device: Device,
    queue: Queue,
    preferred_format: Option<TextureFormat>,
    pipelines: HashMap<TextureFormat, RenderPipeline>,
    uniform_buffer: Buffer,
    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
}

impl Renderer {
    /// Requests an adapter and device.
    ///
    /// When `compatible_surface` is provided, the adapter is guaranteed to be
    /// able to present to it.
    pub fn new(
        instance: &Instance,
        compatible_surface: Option<&wgpu::Surface<'static>>,
    ) -> Result<Self, RenderError> {
        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface,
            ..Default::default()
        }))
        .map_err(|_| RenderError::NoAdapter)?;

        let (device, queue) =
            pollster::block_on(adapter.request_device(&DeviceDescriptor::default()))?;

        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("skews-frame-uniform"),
            size: size_of::<FrameUniform>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("skews-frame-layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX_FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("skews-frame-bind-group"),
            layout: &bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &uniform_buffer,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let preferred_format = compatible_surface.map(|surface| {
            let capabilities = surface.get_capabilities(&adapter);
            capabilities
                .formats
                .iter()
                .copied()
                .find(TextureFormat::is_srgb)
                .unwrap_or_else(|| capabilities.formats[0])
        });

        tracing::debug!(
            adapter = adapter.get_info().name,
            backend = ?adapter.get_info().backend,
            "renderer initialized"
        );

        Ok(Self {
            adapter,
            device,
            queue,
            preferred_format,
            pipelines: HashMap::new(),
            uniform_buffer,
            bind_group_layout,
            bind_group,
        })
    }

    /// Returns the adapter backing this renderer.
    #[must_use]
    pub fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }

    /// Returns the device backing this renderer.
    #[must_use]
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Returns the format preferred for new surfaces, if known.
    #[must_use]
    pub fn preferred_format(&self) -> Option<TextureFormat> {
        self.preferred_format
    }

    /// Configures a surface using this renderer's device.
    pub fn configure_surface(
        &self,
        target: &mut GpuSurface,
        width: u32,
        height: u32,
    ) -> Result<(), RenderError> {
        target.configure(self, width, height)
    }

    /// Renders one frame into `target`.
    ///
    /// The caller decides when to call this; the renderer never schedules
    /// frames by itself.
    pub fn render(
        &mut self,
        target: &mut GpuSurface,
        params: &FrameParams,
    ) -> Result<(), RenderError> {
        let config = target.config().ok_or(RenderError::NotConfigured)?.clone();

        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&FrameUniform::from(params)),
        );

        let mut attempts = 0;
        let texture = loop {
            match target.surface().get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(texture)
                | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => break texture,
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost
                    if attempts < 1 =>
                {
                    attempts += 1;
                    target.reconfigure(self);
                }
                wgpu::CurrentSurfaceTexture::Timeout => {
                    return Err(RenderError::SurfaceUnavailable(
                        "frame acquisition timed out",
                    ));
                }
                wgpu::CurrentSurfaceTexture::Occluded => {
                    return Err(RenderError::SurfaceUnavailable("surface is occluded"));
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    return Err(RenderError::SurfaceUnavailable(
                        "validation error during frame acquisition",
                    ));
                }
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    return Err(RenderError::SurfaceUnavailable(
                        "surface is outdated or lost",
                    ));
                }
            }
        };

        let view = texture
            .texture
            .create_view(&TextureViewDescriptor::default());
        let device = self.device.clone();
        let queue = self.queue.clone();
        let pipeline = self.pipeline(config.format);
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("skews-frame"),
        });

        {
            let mut pass: RenderPass<'_> = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("skews-frame-pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations {
                        load: LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));
        queue.present(texture);

        Ok(())
    }

    fn pipeline(&mut self, format: TextureFormat) -> &RenderPipeline {
        if !self.pipelines.contains_key(&format) {
            let device = self.device.clone();
            let platform_layout = self.bind_group_layout.clone();
            let module = device.create_shader_module(ShaderModuleDescriptor {
                label: Some("skews-rect-shader"),
                source: ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
            });

            let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("skews-rect-layout"),
                bind_group_layouts: &[Some(&platform_layout)],
                immediate_size: 0,
            });

            let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("skews-rect-pipeline"),
                layout: Some(&layout),
                vertex: VertexState {
                    module: &module,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(FragmentState {
                    module: &module,
                    entry_point: Some("fs_main"),
                    targets: &[Some(ColorTargetState {
                        format,
                        blend: Some(BlendState::ALPHA_BLENDING),
                        write_mask: ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: PrimitiveState::default(),
                depth_stencil: None,
                multisample: MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });

            self.pipelines.insert(format, pipeline);
        }

        &self.pipelines[&format]
    }
}
