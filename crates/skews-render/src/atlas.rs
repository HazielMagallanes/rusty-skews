//! Glyph atlas: rasterized glyphs packed into an R8 texture.

use std::collections::HashMap;

use skews_text::{CacheKey, TextEngine};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferDescriptor, BufferUsages,
    Device, Extent3d, FilterMode, Origin3d, Queue, SamplerBindingType, SamplerDescriptor,
    ShaderStages, TexelCopyBufferLayout, TexelCopyTextureInfo, Texture, TextureAspect,
    TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
    TextureViewDescriptor, TextureViewDimension,
};

use crate::renderer::TextVertex;

/// Where a glyph lives in the atlas, plus its raster offsets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtlasEntry {
    /// UV rectangle `[u0, v0, u1, v1]` in normalized coordinates.
    pub uv: [f32; 4],
    /// Offset from the pen to the bitmap's left edge.
    pub left: i32,
    /// Offset from the baseline to the bitmap's top edge.
    pub top: i32,
    /// Bitmap width in pixels.
    pub width: u32,
    /// Bitmap height in pixels.
    pub height: u32,
}

/// Shelf packer for glyph rectangles (pure; unit-tested).
#[derive(Debug)]
pub struct ShelfPacker {
    size: u32,
    x: u32,
    y: u32,
    row_height: u32,
}

impl ShelfPacker {
    /// Creates a packer for a square atlas of `size` pixels.
    #[must_use]
    pub const fn new(size: u32) -> Self {
        Self {
            size,
            x: 0,
            y: 0,
            row_height: 0,
        }
    }

    /// Allocates a rectangle, or `None` when the atlas is full.
    pub fn allocate(&mut self, width: u32, height: u32) -> Option<(u32, u32)> {
        if width == 0 || height == 0 || width > self.size || height > self.size {
            return None;
        }

        if self.x + width > self.size {
            self.y += self.row_height + 1;
            self.x = 0;
            self.row_height = 0;
        }
        if self.y + height > self.size {
            return None;
        }

        let position = (self.x, self.y);
        self.x += width + 1;
        self.row_height = self.row_height.max(height);
        Some(position)
    }

    /// Resets the packer to an empty atlas.
    pub fn reset(&mut self) {
        self.x = 0;
        self.y = 0;
        self.row_height = 0;
    }
}

/// The glyph atlas texture plus its GPU state.
pub struct GlyphAtlas {
    size: u32,
    layout: BindGroupLayout,
    texture: Texture,
    bind_group: BindGroup,
    vertices: Buffer,
    capacity: u64,
    entries: HashMap<CacheKey, AtlasEntry>,
    packer: ShelfPacker,
}

impl GlyphAtlas {
    /// Creates an empty atlas and its vertex buffer.
    #[must_use]
    pub fn new(device: &Device, layout: &BindGroupLayout) -> Self {
        const SIZE: u32 = 1024;
        let (texture, bind_group) = create_atlas_texture(device, layout, SIZE);

        let vertices = device.create_buffer(&BufferDescriptor {
            label: Some("skews-text-vertices"),
            size: 64 * 1024,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            size: SIZE,
            layout: layout.clone(),
            texture,
            bind_group,
            vertices,
            capacity: 64 * 1024,
            entries: HashMap::new(),
            packer: ShelfPacker::new(SIZE),
        }
    }

    /// Returns the atlas bind group (atlas texture + sampler).
    #[must_use]
    pub fn bind_group(&self) -> &BindGroup {
        &self.bind_group
    }

    /// Returns the glyph vertex buffer.
    #[must_use]
    pub fn vertices(&self) -> &Buffer {
        &self.vertices
    }

    /// Ensures the vertex buffer can hold `bytes` and uploads `vertices`.
    pub fn upload(&mut self, device: &Device, queue: &Queue, vertices: &[TextVertex]) {
        let bytes = std::mem::size_of_val(vertices) as u64;
        if bytes > self.capacity {
            let capacity = bytes.next_power_of_two();
            self.vertices = device.create_buffer(&BufferDescriptor {
                label: Some("skews-text-vertices"),
                size: capacity,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.capacity = capacity;
        }

        queue.write_buffer(&self.vertices, 0, bytemuck::cast_slice(vertices));
    }

    /// Returns (rasterizing if needed) the atlas entry for a glyph.
    pub fn entry(
        &mut self,
        device: &Device,
        queue: &Queue,
        text: &mut TextEngine,
        key: &CacheKey,
    ) -> Option<AtlasEntry> {
        if let Some(entry) = self.entries.get(key) {
            return Some(*entry);
        }

        let bitmap = text.rasterize(key)?;
        if bitmap.width == 0 || bitmap.height == 0 {
            return None;
        }

        let (x, y) = match self.packer.allocate(bitmap.width, bitmap.height) {
            Some(position) => position,
            None => {
                // Atlas full: rebuild it from scratch. Glyphs re-rasterize on
                // demand, so this is correct, just a rare slow path.
                self.reset(device);
                self.packer.allocate(bitmap.width, bitmap.height)?
            }
        };

        let (data, padded_width) = padded_rows(&bitmap.alpha, bitmap.width, bitmap.height);
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: Origin3d { x, y, z: 0 },
                aspect: TextureAspect::All,
            },
            &data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_width),
                rows_per_image: Some(bitmap.height),
            },
            Extent3d {
                width: bitmap.width,
                height: bitmap.height,
                depth_or_array_layers: 1,
            },
        );

        let size = self.size as f32;
        let entry = AtlasEntry {
            uv: [
                x as f32 / size,
                y as f32 / size,
                (x + bitmap.width) as f32 / size,
                (y + bitmap.height) as f32 / size,
            ],
            left: bitmap.left,
            top: bitmap.top,
            width: bitmap.width,
            height: bitmap.height,
        };
        self.entries.insert(*key, entry);

        Some(entry)
    }

    fn reset(&mut self, device: &Device) {
        let (texture, bind_group) = create_atlas_texture(device, &self.layout, self.size);
        self.texture = texture;
        self.bind_group = bind_group;
        self.entries.clear();
        self.packer.reset();
    }
}

fn create_atlas_texture(
    device: &Device,
    layout: &BindGroupLayout,
    size: u32,
) -> (Texture, BindGroup) {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("skews-glyph-atlas"),
        size: Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::R8Unorm,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    let sampler = device.create_sampler(&SamplerDescriptor {
        label: Some("skews-glyph-sampler"),
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        ..Default::default()
    });
    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("skews-glyph-bind-group"),
        layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(&view),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Sampler(&sampler),
            },
        ],
    });

    (texture, bind_group)
}

/// Creates the bind group layout for the atlas texture and sampler.
#[must_use]
pub fn atlas_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("skews-glyph-layout"),
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// Pads alpha rows to the 256-byte alignment required by texture uploads.
#[must_use]
pub fn padded_rows(alpha: &[u8], width: u32, height: u32) -> (Vec<u8>, u32) {
    let padded_width = width.div_ceil(256) * 256;
    if padded_width == width {
        return (alpha.to_vec(), width);
    }

    let mut data = vec![0_u8; (padded_width * height) as usize];
    for row in 0..height as usize {
        let source = row * width as usize;
        let target = row * padded_width as usize;
        data[target..target + width as usize]
            .copy_from_slice(&alpha[source..source + width as usize]);
    }

    (data, padded_width)
}

#[cfg(test)]
mod tests {
    use super::{ShelfPacker, padded_rows};

    #[test]
    fn packer_fills_rows_then_moves_down() {
        let mut packer = ShelfPacker::new(64);

        assert_eq!(packer.allocate(20, 10), Some((0, 0)));
        assert_eq!(packer.allocate(20, 12), Some((21, 0)));
        assert_eq!(packer.allocate(20, 10), Some((42, 0)));
        // Next allocation wraps to a new row below the tallest glyph.
        assert_eq!(packer.allocate(20, 10), Some((0, 13)));
    }

    #[test]
    fn packer_rejects_overflow_and_oversize() {
        let mut packer = ShelfPacker::new(32);

        assert_eq!(packer.allocate(33, 1), None);
        assert_eq!(packer.allocate(0, 10), None);
        assert_eq!(packer.allocate(32, 32), Some((0, 0)));
        assert_eq!(packer.allocate(1, 1), None);
    }

    #[test]
    fn reset_restarts_packing() {
        let mut packer = ShelfPacker::new(32);
        assert_eq!(packer.allocate(32, 32), Some((0, 0)));

        packer.reset();

        assert_eq!(packer.allocate(8, 8), Some((0, 0)));
    }

    #[test]
    fn rows_are_padded_to_256_bytes() {
        let alpha: Vec<u8> = (0..12).collect();

        let (data, stride) = padded_rows(&alpha, 4, 3);

        assert_eq!(stride, 256);
        assert_eq!(data.len(), 768);
        assert_eq!(&data[0..4], &alpha[0..4]);
        assert_eq!(&data[256..260], &alpha[4..8]);
        assert_eq!(&data[512..516], &alpha[8..12]);
    }

    #[test]
    fn already_aligned_rows_are_untouched() {
        let alpha = vec![7_u8; 256 * 2];

        let (data, stride) = padded_rows(&alpha, 256, 2);

        assert_eq!(stride, 256);
        assert_eq!(data, alpha);
    }
}
