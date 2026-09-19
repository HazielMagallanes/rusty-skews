//! Text shaping and glyph rasterization for rusty-skews.
//!
//! Wraps `cosmic-text` with the shell's needs: measure and shape short labels
//! with font fallback, then rasterize glyphs on demand for the renderer's
//! atlas. Shaping results are cached per `(text, size)` so repeated frames do
//! no work.

#![forbid(unsafe_code)]

use std::collections::HashMap;

use cosmic_text::{
    Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent, SwashImage,
};

pub use cosmic_text::CacheKey;

/// A glyph positioned inside a shaped text run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionedGlyph {
    /// Cache key for rasterization (font, glyph, size, subpixel bins).
    pub key: CacheKey,
    /// Integer pen x relative to the run origin (subpixel offset is baked into
    /// the rasterized bitmap).
    pub x: i32,
    /// Integer baseline y relative to the run origin.
    pub y: i32,
}

/// A shaped text run.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapedText {
    /// Advance width in pixels.
    pub width: f32,
    /// Line height in pixels.
    pub height: f32,
    /// Glyphs with their pen positions.
    pub glyphs: Vec<PositionedGlyph>,
}

/// A rasterized glyph bitmap (8-bit alpha).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphBitmap {
    /// Bitmap width in pixels.
    pub width: u32,
    /// Bitmap height in pixels.
    pub height: u32,
    /// Offset from the pen to the bitmap's left edge.
    pub left: i32,
    /// Offset from the baseline to the bitmap's top edge.
    pub top: i32,
    /// Row-major alpha values (`width * height` bytes).
    pub alpha: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ShapeKey {
    text: String,
    size_bits: u32,
}

/// Shaping and rasterization engine, owned by the renderer thread.
pub struct TextEngine {
    font_system: FontSystem,
    swash_cache: SwashCache,
    family: String,
    line_height_ratio: f32,
    shaped: HashMap<ShapeKey, ShapedText>,
}

impl TextEngine {
    /// Creates an engine for a font family (with system fallback).
    #[must_use]
    pub fn new(family: impl Into<String>) -> Self {
        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
            family: family.into(),
            line_height_ratio: 1.3,
            shaped: HashMap::new(),
        }
    }

    /// Number of fonts known to the font database (0 means no fallback is possible).
    #[must_use]
    pub fn font_count(&self) -> usize {
        self.font_system.db().len()
    }

    /// Shapes (or returns the cached shape of) `text` at `size` pixels.
    pub fn measure(&mut self, text: &str, size: f32) -> ShapedText {
        let key = ShapeKey {
            text: text.to_owned(),
            size_bits: size.to_bits(),
        };
        if let Some(shaped) = self.shaped.get(&key) {
            return shaped.clone();
        }

        let metrics = Metrics {
            font_size: size,
            line_height: size * self.line_height_ratio,
        };
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        let attrs = Attrs {
            family: Family::Name(self.family.as_str()),
            ..Attrs::new().metrics(metrics)
        };
        buffer.set_text(text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut shaped = ShapedText {
            width: 0.0,
            height: metrics.line_height,
            glyphs: Vec::new(),
        };
        for run in buffer.layout_runs() {
            shaped.width = shaped.width.max(run.line_w);
            shaped.height = shaped.height.max(run.line_height);
            for glyph in run.glyphs {
                let pen = (glyph.x + glyph.x_offset, run.line_y + glyph.y_offset);
                let (key, x, y) = CacheKey::new(
                    glyph.font_id,
                    glyph.glyph_id,
                    glyph.font_size,
                    pen,
                    glyph.font_weight,
                    glyph.cache_key_flags,
                );
                shaped.glyphs.push(PositionedGlyph { key, x, y });
            }
        }

        self.shaped.insert(key, shaped.clone());
        shaped
    }

    /// Rasterizes a glyph to an alpha bitmap, if the glyph exists.
    pub fn rasterize(&mut self, key: &CacheKey) -> Option<GlyphBitmap> {
        let image = self
            .swash_cache
            .get_image(&mut self.font_system, *key)
            .as_ref()?;
        let placement = image.placement;

        Some(GlyphBitmap {
            width: placement.width,
            height: placement.height,
            left: placement.left,
            top: placement.top,
            alpha: to_alpha(image),
        })
    }
}

/// Converts a swash image into 8-bit alpha.
///
/// Color glyphs (emoji) are reduced to a silhouette in M1; an RGBA atlas
/// arrives with richer text support.
fn to_alpha(image: &SwashImage) -> Vec<u8> {
    match image.content {
        SwashContent::Mask => image.data.clone(),
        SwashContent::SubpixelMask => image
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| pixel[0].max(pixel[1]).max(pixel[2]))
            .collect(),
        SwashContent::Color => image
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| pixel[3])
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::TextEngine;

    fn engine() -> TextEngine {
        TextEngine::new("Roboto")
    }

    #[test]
    fn measures_and_shapes_text_when_fonts_are_available() {
        let mut engine = engine();
        if engine.font_count() == 0 {
            eprintln!("no system fonts available; skipping shaping assertions");
            return;
        }

        let shaped = engine.measure("12:34", 12.0);

        assert!(shaped.width > 0.0, "expected a positive width");
        assert!(shaped.height >= 12.0, "expected a line height");
        assert_eq!(shaped.glyphs.len(), 5, "expected one glyph per character");
    }

    #[test]
    fn shaping_results_are_cached() {
        let mut engine = engine();

        let first = engine.measure("cpu 61°C", 12.0);
        let second = engine.measure("cpu 61°C", 12.0);

        assert_eq!(first, second);
    }

    #[test]
    fn rasterizes_a_glyph_when_fonts_are_available() {
        let mut engine = engine();
        if engine.font_count() == 0 {
            eprintln!("no system fonts available; skipping rasterization assertion");
            return;
        }

        let shaped = engine.measure("A", 16.0);
        let glyph = shaped.glyphs.first().expect("shaping produced a glyph");

        let bitmap = engine.rasterize(&glyph.key).expect("glyph rasterizes");

        assert!(bitmap.width > 0 && bitmap.height > 0);
        assert_eq!(bitmap.alpha.len(), (bitmap.width * bitmap.height) as usize);
        assert!(bitmap.alpha.iter().any(|value| *value > 0), "expected ink");
    }
}
