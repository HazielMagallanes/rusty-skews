//! The wgpu renderer: shape and text pipelines, atlas, frame submission.
//!
//! Rendering is demand-driven (ADR-0002): the caller passes a [`Scene`] and
//! the renderer draws exactly that, with no background loops.

use std::collections::HashMap;
use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use skews_core::Rgba;
use skews_text::TextEngine;
use skews_ui::{DrawItem, Scene};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BlendState, Buffer, BufferBinding,
    BufferBindingType, BufferDescriptor, BufferUsages, ColorTargetState, ColorWrites,
    CommandEncoderDescriptor, Device, DeviceDescriptor, FragmentState, Instance, LoadOp,
    MultisampleState, Operations, PipelineLayoutDescriptor, PrimitiveState, Queue,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor,
    RequestAdapterOptions, ShaderModuleDescriptor, ShaderSource, ShaderStages, StoreOp,
    TextureFormat, TextureViewDescriptor, VertexState,
};

use crate::atlas::{GlyphAtlas, atlas_layout};
use crate::error::RenderError;
use crate::surface::GpuSurface;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct ShapeUniform {
    clear: [f32; 4],
    rect: [f32; 4],
    color: [f32; 4],
    radius: f32,
    _padding: [f32; 3],
}

impl ShapeUniform {
    fn from_rect(rect: skews_core::Rect, color: Rgba, radius: f32) -> Self {
        Self {
            clear: Rgba::TRANSPARENT.to_array(),
            rect: [
                rect.x as f32,
                rect.y as f32,
                rect.width as f32,
                rect.height as f32,
            ],
            color: color.to_array(),
            radius,
            _padding: [0.0; 3],
        }
    }
}

/// A glyph quad vertex (clip-space position, atlas UV, straight-alpha color).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct TextVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

/// A wgpu device with the shell's pipelines, created once per process.
pub struct Renderer {
    adapter: wgpu::Adapter,
    device: Device,
    queue: Queue,
    preferred_format: Option<TextureFormat>,
    shape_uniform: Buffer,
    shape_bind_group: BindGroup,
    shape_pipelines: HashMap<TextureFormat, RenderPipeline>,
    text_layout: BindGroupLayout,
    text_pipelines: HashMap<TextureFormat, RenderPipeline>,
    atlas: GlyphAtlas,
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

        let shape_uniform = device.create_buffer(&BufferDescriptor {
            label: Some("skews-shape-uniform"),
            size: size_of::<ShapeUniform>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let shape_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("skews-shape-layout"),
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

        let shape_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("skews-shape-bind-group"),
            layout: &shape_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &shape_uniform,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let text_layout = atlas_layout(&device);
        let atlas = GlyphAtlas::new(&device, &text_layout);

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
            shape_uniform,
            shape_bind_group,
            shape_pipelines: HashMap::new(),
            text_layout,
            text_pipelines: HashMap::new(),
            atlas,
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

    /// Draws a scene into `target`.
    pub fn render_scene(
        &mut self,
        target: &mut GpuSurface,
        scene: &Scene,
        text_engine: &mut TextEngine,
    ) -> Result<(), RenderError> {
        let config = target.config().ok_or(RenderError::NotConfigured)?.clone();
        let (width, height) = (config.width, config.height);

        // 1. Build glyph quads (rasterizing into the atlas as needed).
        let mut vertices: Vec<TextVertex> = Vec::new();
        for item in &scene.items {
            if let DrawItem::Text {
                text,
                x,
                y,
                size,
                color,
            } = item
            {
                let shaped = text_engine.measure(text, *size);
                for glyph in &shaped.glyphs {
                    let Some(entry) =
                        self.atlas
                            .entry(&self.device, &self.queue, text_engine, &glyph.key)
                    else {
                        continue;
                    };
                    let glyph_x = (*x + glyph.x + entry.left) as f32;
                    let glyph_y = (*y + glyph.y - entry.top) as f32;
                    push_quad(
                        &mut vertices,
                        glyph_x,
                        glyph_y,
                        entry.width as f32,
                        entry.height as f32,
                        entry.uv,
                        *color,
                        width,
                        height,
                    );
                }
            }
        }
        if !vertices.is_empty() {
            self.atlas.upload(&self.device, &self.queue, &vertices);
        }

        // 2. Make sure pipelines exist for this surface format.
        self.ensure_pipelines(config.format);

        // 3. Acquire the frame (reconfiguring once on Outdated/Lost).
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
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("skews-frame"),
            });

        // 4. Background clear.
        {
            let _pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("skews-clear"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations {
                        load: LoadOp::Clear(premultiplied(scene.background)),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }

        // 5. Rect items, one pass each (the uniform changes between them).
        let shape_pipeline = self.shape_pipelines[&config.format].clone();
        for item in &scene.items {
            if let DrawItem::Rect {
                rect,
                color,
                radius,
            } = item
            {
                self.queue.write_buffer(
                    &self.shape_uniform,
                    0,
                    bytemuck::bytes_of(&ShapeUniform::from_rect(*rect, *color, *radius)),
                );

                let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("skews-shape"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: Operations {
                            load: LoadOp::Load,
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&shape_pipeline);
                pass.set_bind_group(0, &self.shape_bind_group, &[]);
                pass.draw(0..3, 0..1);
            }
        }

        // 6. Text, one pass and one draw call for the whole scene.
        if !vertices.is_empty() {
            let text_pipeline = self.text_pipelines[&config.format].clone();
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("skews-text"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&text_pipeline);
            pass.set_bind_group(0, self.atlas.bind_group(), &[]);
            pass.set_vertex_buffer(0, self.atlas.vertices().slice(..));
            pass.draw(0..vertices.len() as u32, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));
        self.queue.present(texture);

        Ok(())
    }

    fn ensure_pipelines(&mut self, format: TextureFormat) {
        if !self.shape_pipelines.contains_key(&format) {
            let pipeline =
                build_shape_pipeline(&self.device, &self.shape_bind_group_layout(), format);
            self.shape_pipelines.insert(format, pipeline);
        }
        if !self.text_pipelines.contains_key(&format) {
            let pipeline = build_text_pipeline(&self.device, &self.text_layout, format);
            self.text_pipelines.insert(format, pipeline);
        }
    }

    fn shape_bind_group_layout(&self) -> BindGroupLayout {
        // Recreate the layout from the bind group's descriptor: the pipeline
        // only needs a compatible layout, and this keeps one source of truth.
        self.device
            .create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("skews-shape-layout"),
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
            })
    }
}

fn build_shape_pipeline(
    device: &Device,
    layout: &BindGroupLayout,
    format: TextureFormat,
) -> RenderPipeline {
    let module = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("skews-shape-shader"),
        source: ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("skews-shape-pipeline-layout"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });

    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("skews-shape-pipeline"),
        layout: Some(&pipeline_layout),
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
    })
}

fn build_text_pipeline(
    device: &Device,
    layout: &BindGroupLayout,
    format: TextureFormat,
) -> RenderPipeline {
    let module = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("skews-text-shader"),
        source: ShaderSource::Wgsl(include_str!("text.wgsl").into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("skews-text-pipeline-layout"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });

    const ATTRIBUTES: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x2, 1 => Float32x2, 2 => Float32x4
    ];

    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("skews-text-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: VertexState {
            module: &module,
            entry_point: Some("vs_main"),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<TextVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &ATTRIBUTES,
            })],
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
    })
}

/// Converts a pixel-space quad into two clip-space triangles.
#[allow(clippy::too_many_arguments)]
fn push_quad(
    vertices: &mut Vec<TextVertex>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    uv: [f32; 4],
    color: Rgba,
    surface_width: u32,
    surface_height: u32,
) {
    let surface_width = surface_width.max(1) as f32;
    let surface_height = surface_height.max(1) as f32;

    let x0 = x / surface_width * 2.0 - 1.0;
    let x1 = (x + width) / surface_width * 2.0 - 1.0;
    let y0 = 1.0 - y / surface_height * 2.0;
    let y1 = 1.0 - (y + height) / surface_height * 2.0;
    let [u0, v0, u1, v1] = uv;
    let color = color.to_array();

    let top_left = TextVertex {
        position: [x0, y0],
        uv: [u0, v0],
        color,
    };
    let top_right = TextVertex {
        position: [x1, y0],
        uv: [u1, v0],
        color,
    };
    let bottom_left = TextVertex {
        position: [x0, y1],
        uv: [u0, v1],
        color,
    };
    let bottom_right = TextVertex {
        position: [x1, y1],
        uv: [u1, v1],
        color,
    };

    vertices.extend_from_slice(&[
        top_left,
        top_right,
        bottom_left,
        top_right,
        bottom_right,
        bottom_left,
    ]);
}

/// Converts straight-alpha color into the premultiplied clear value.
fn premultiplied(color: Rgba) -> wgpu::Color {
    wgpu::Color {
        r: f64::from(color.r) * f64::from(color.a),
        g: f64::from(color.g) * f64::from(color.a),
        b: f64::from(color.b) * f64::from(color.a),
        a: f64::from(color.a),
    }
}

#[cfg(test)]
mod tests {
    use super::{TextVertex, premultiplied, push_quad};
    use skews_core::Rgba;

    #[test]
    fn quad_covers_pixel_rect_in_clip_space() {
        let mut vertices: Vec<TextVertex> = Vec::new();

        push_quad(
            &mut vertices,
            0.0,
            0.0,
            10.0,
            10.0,
            [0.0, 0.0, 1.0, 1.0],
            Rgba::WHITE,
            100,
            50,
        );

        assert_eq!(vertices.len(), 6);
        // Top-left pixel maps to clip (-1, 1).
        assert_eq!(vertices[0].position, [-1.0, 1.0]);
        // Bottom-right corner of the quad.
        assert_eq!(vertices[4].position[0], -0.8);
        assert_eq!(vertices[4].position[1], 0.6);
    }

    #[test]
    fn clear_color_is_premultiplied() {
        let color = Rgba::new(1.0, 0.5, 0.0, 0.5);

        let clear = premultiplied(color);

        assert!((clear.r - 0.5).abs() < 1e-6);
        assert!((clear.g - 0.25).abs() < 1e-6);
        assert!((clear.b - 0.0).abs() < 1e-6);
        assert!((clear.a - 0.5).abs() < 1e-6);
    }
}
